use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, anyhow};
use flate2::read::GzDecoder;
use serde::Deserialize;
use tar::Archive;

use crate::spec::GithubSource;

const USER_AGENT: &str = concat!("rtango/", env!("CARGO_PKG_VERSION"));

/// Where the rtango spec lives inside a fetched or local collection root.
pub const COLLECTION_SPEC_PATH: &str = ".rtango/spec.yaml";

/// Materialize a GitHub source on disk and return the path that should be
/// treated as a "project root" by the expansion pipeline.
///
/// Resolves the ref to an immutable commit SHA, downloads the tarball into
/// a content-addressed cache (idempotent), and returns the cache directory.
pub fn fetch_github(source: &GithubSource) -> anyhow::Result<PathBuf> {
    let (owner, repo) = parse_owner_repo(&source.github)?;
    let sha = resolve_ref(owner, repo, source.r#ref.as_str())?;
    let cache_dir = cache_root()?
        .join("github")
        .join(owner)
        .join(repo)
        .join(&sha);

    if !cache_dir.exists() {
        download_and_extract(owner, repo, &sha, &cache_dir)?;
    }

    Ok(cache_dir)
}

/// Read the rtango spec from an already-materialised collection root directory.
///
/// Used by `expand_collection` after resolving the source (local path or
/// GitHub cache) to its on-disk location.
pub fn read_collection_spec(collection_root: &Path) -> anyhow::Result<crate::spec::Spec> {
    let spec_file = collection_root.join(COLLECTION_SPEC_PATH);
    if !spec_file.is_file() {
        anyhow::bail!(
            "collection directory {} does not contain {}",
            collection_root.display(),
            COLLECTION_SPEC_PATH,
        );
    }
    let content = fs::read_to_string(&spec_file).with_context(|| {
        format!(
            "failed to read {} from {}",
            COLLECTION_SPEC_PATH,
            collection_root.display()
        )
    })?;
    crate::spec::io::parse_spec_content(&content, &collection_root.display().to_string())
}

fn parse_owner_repo(slug: &str) -> anyhow::Result<(&str, &str)> {
    slug.split_once('/')
        .filter(|(o, r)| !o.is_empty() && !r.is_empty() && !r.contains('/'))
        .ok_or_else(|| anyhow!("invalid github slug '{}', expected 'owner/repo'", slug))
}

fn cache_root() -> anyhow::Result<PathBuf> {
    let base =
        dirs::cache_dir().ok_or_else(|| anyhow!("could not determine user cache directory"))?;
    Ok(base.join("rtango"))
}

#[derive(Deserialize)]
struct CommitResponse {
    sha: String,
}

fn resolve_ref(owner: &str, repo: &str, git_ref: &str) -> anyhow::Result<String> {
    let url = format!("https://api.github.com/repos/{owner}/{repo}/commits/{git_ref}");
    let mut req = ureq::get(&url)
        .set("User-Agent", USER_AGENT)
        .set("Accept", "application/vnd.github+json")
        .set("X-GitHub-Api-Version", "2022-11-28");
    if let Some(token) = github_token() {
        req = req.set("Authorization", &format!("Bearer {token}"));
    }
    let resp = req
        .call()
        .with_context(|| format!("failed to resolve {owner}/{repo}@{git_ref}"))?;
    let body: CommitResponse = resp
        .into_json()
        .with_context(|| format!("invalid commit response for {owner}/{repo}@{git_ref}"))?;
    Ok(body.sha)
}

fn github_token() -> Option<String> {
    std::env::var("RTANGO_GITHUB_TOKEN")
        .or_else(|_| std::env::var("GITHUB_TOKEN"))
        .ok()
        .filter(|s| !s.is_empty())
}

fn download_and_extract(owner: &str, repo: &str, sha: &str, dest: &Path) -> anyhow::Result<()> {
    let url = format!("https://codeload.github.com/{owner}/{repo}/tar.gz/{sha}");
    let mut req = ureq::get(&url).set("User-Agent", USER_AGENT);
    if let Some(token) = github_token() {
        req = req.set("Authorization", &format!("Bearer {token}"));
    }
    let resp = req
        .call()
        .with_context(|| format!("failed to download {owner}/{repo}@{sha}"))?;

    let parent = dest
        .parent()
        .ok_or_else(|| anyhow!("cache dest has no parent: {}", dest.display()))?;
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create cache dir {}", parent.display()))?;

    let staging = parent.join(format!(".staging-{sha}"));
    if staging.exists() {
        fs::remove_dir_all(&staging).ok();
    }
    fs::create_dir_all(&staging)?;

    let reader: Box<dyn Read + Send + Sync> = resp.into_reader();
    let decoder = GzDecoder::new(reader);
    let mut archive = Archive::new(decoder);
    archive
        .unpack(&staging)
        .with_context(|| format!("failed to extract {owner}/{repo}@{sha}"))?;

    let inner = single_top_level_dir(&staging)
        .with_context(|| format!("unexpected archive layout for {owner}/{repo}@{sha}"))?;

    if dest.exists() {
        // Another process won the race — drop our staging copy.
        fs::remove_dir_all(&staging).ok();
        return Ok(());
    }
    fs::rename(&inner, dest)
        .with_context(|| format!("failed to install cache at {}", dest.display()))?;
    fs::remove_dir_all(&staging).ok();
    Ok(())
}

fn single_top_level_dir(staging: &Path) -> anyhow::Result<PathBuf> {
    let mut iter = fs::read_dir(staging)?;
    let first = iter
        .next()
        .ok_or_else(|| anyhow!("archive is empty"))??
        .path();
    if iter.next().is_some() {
        anyhow::bail!("archive has multiple top-level entries");
    }
    if !first.is_dir() {
        anyhow::bail!("archive top-level entry is not a directory");
    }
    Ok(first)
}
