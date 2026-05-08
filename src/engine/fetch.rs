use std::fs;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, anyhow};
use flate2::read::GzDecoder;
use serde::Deserialize;
use tar::Archive;
use thiserror::Error;

use crate::spec::GithubSource;

const USER_AGENT: &str = concat!("rtango/", env!("CARGO_PKG_VERSION"));

/// Where the rtango spec lives inside a fetched or local collection root.
pub const COLLECTION_SPEC_PATH: &str = ".rtango/spec.yaml";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GithubFetchErrorClass {
    InvalidSource,
    Auth,
    RateLimited,
    Http,
    Network,
    Response,
    Archive,
    PromptAborted,
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct GithubFetchError {
    class: GithubFetchErrorClass,
    message: String,
}

impl GithubFetchError {
    pub fn is_ignorable_fetch_failure(&self) -> bool {
        self.class != GithubFetchErrorClass::InvalidSource
    }

    fn should_offer_login(&self) -> bool {
        matches!(
            self.class,
            GithubFetchErrorClass::Auth | GithubFetchErrorClass::RateLimited
        )
    }

    fn invalid_source(slug: &str) -> Self {
        Self {
            class: GithubFetchErrorClass::InvalidSource,
            message: format!("invalid github slug '{slug}', expected 'owner/repo'"),
        }
    }

    fn prompt_aborted(source: &GithubSource) -> Self {
        Self {
            class: GithubFetchErrorClass::PromptAborted,
            message: format!(
                "aborted GitHub CLI authentication for {}; fetch was not retried",
                describe_github_source(source)
            ),
        }
    }

    fn prompt_io(err: io::Error) -> Self {
        Self {
            class: GithubFetchErrorClass::PromptAborted,
            message: format!("failed to read GitHub CLI auth input: {err}"),
        }
    }

    fn gh_cli(action: &str, err: impl std::fmt::Display) -> Self {
        Self {
            class: GithubFetchErrorClass::PromptAborted,
            message: format!("failed to {action} via GitHub CLI: {err}"),
        }
    }

    fn gh_cli_login_failed(source: &GithubSource) -> Self {
        Self {
            class: GithubFetchErrorClass::PromptAborted,
            message: format!(
                "`gh auth login` did not complete successfully for {}; fetch was not retried",
                describe_github_source(source)
            ),
        }
    }

    fn gh_cli_missing_token(source: &GithubSource) -> Self {
        Self {
            class: GithubFetchErrorClass::PromptAborted,
            message: format!(
                "GitHub CLI authentication finished, but `gh auth token` did not return a token for {}; fetch was not retried",
                describe_github_source(source)
            ),
        }
    }

    fn network(action: &str, target: &str, err: impl std::fmt::Display) -> Self {
        Self {
            class: GithubFetchErrorClass::Network,
            message: format!("failed to reach GitHub while {action} {target}: {err}"),
        }
    }

    fn response(action: &str, target: &str, err: impl std::fmt::Display) -> Self {
        Self {
            class: GithubFetchErrorClass::Response,
            message: format!("GitHub returned an invalid response while {action} {target}: {err}"),
        }
    }

    fn archive(action: &str, target: &str, err: impl std::fmt::Display) -> Self {
        Self {
            class: GithubFetchErrorClass::Archive,
            message: format!("failed while {action} {target}: {err}"),
        }
    }

    fn status(action: &str, target: &str, code: u16, response: ureq::Response) -> Self {
        let remaining = response.header("X-RateLimit-Remaining").map(str::to_string);
        let body = response.into_string().unwrap_or_default();
        let body_lower = body.to_ascii_lowercase();

        let class = if code == 401 {
            GithubFetchErrorClass::Auth
        } else if code == 403
            && (remaining.as_deref() == Some("0") || body_lower.contains("rate limit"))
        {
            GithubFetchErrorClass::RateLimited
        } else if code == 403 || body_lower.contains("requires authentication") {
            GithubFetchErrorClass::Auth
        } else {
            GithubFetchErrorClass::Http
        };

        let mut message = match class {
            GithubFetchErrorClass::Auth => {
                format!("GitHub requires authentication while {action} {target} (HTTP {code})")
            }
            GithubFetchErrorClass::RateLimited => {
                format!("GitHub rate limit blocked {action} {target} (HTTP {code})")
            }
            GithubFetchErrorClass::Http => {
                format!("GitHub failed while {action} {target} (HTTP {code})")
            }
            _ => unreachable!("status() only produces HTTP-like classes"),
        };

        if let Some(detail) = response_detail(&body) {
            message.push_str(&format!(". GitHub said: {detail}"));
        }
        if matches!(
            class,
            GithubFetchErrorClass::Auth | GithubFetchErrorClass::RateLimited
        ) {
            message.push_str(
                ". rtango can retry after `gh auth login`, or you can provide RTANGO_GITHUB_TOKEN/GITHUB_TOKEN.",
            );
        }

        Self { class, message }
    }
}

/// Materialize a GitHub source on disk and return the path that should be
/// treated as a "project root" by the expansion pipeline.
///
/// Resolves the ref to an immutable commit SHA, downloads the tarball into
/// a content-addressed cache (idempotent), and returns the cache directory.
pub fn fetch_github(source: &GithubSource) -> anyhow::Result<PathBuf> {
    let (owner, repo) = parse_owner_repo(&source.github)?;
    let mut token = github_token();
    let mut prompted = false;

    loop {
        match fetch_github_with_token(owner, repo, source, token.as_deref()) {
            Ok(path) => return Ok(path),
            Err(err)
                if err.should_offer_login() && !prompted && can_prompt_for_github_cli_login() =>
            {
                if !prompt_for_github_cli_login(source, &err)? {
                    return Err(GithubFetchError::prompt_aborted(source).into());
                }
                token = github_token_from_gh_cli();
                if token.is_none() {
                    return Err(GithubFetchError::gh_cli_missing_token(source).into());
                }
                prompted = true;
            }
            Err(err) => return Err(err.into()),
        }
    }
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

pub(crate) fn describe_github_source(source: &GithubSource) -> String {
    if source.path.is_empty() {
        format!("{}@{}", source.github, source.r#ref)
    } else {
        format!("{}@{}:{}", source.github, source.r#ref, source.path)
    }
}

fn fetch_github_with_token(
    owner: &str,
    repo: &str,
    source: &GithubSource,
    token: Option<&str>,
) -> Result<PathBuf, GithubFetchError> {
    let sha = resolve_ref(owner, repo, source.r#ref.as_str(), token)?;
    let cache_dir = cache_root()?
        .join("github")
        .join(owner)
        .join(repo)
        .join(&sha);

    if !cache_dir.exists() {
        download_and_extract(owner, repo, &sha, token, &cache_dir)?;
    }

    Ok(cache_dir)
}

fn parse_owner_repo(slug: &str) -> Result<(&str, &str), GithubFetchError> {
    slug.split_once('/')
        .filter(|(o, r)| !o.is_empty() && !r.is_empty() && !r.contains('/'))
        .ok_or_else(|| GithubFetchError::invalid_source(slug))
}

fn cache_root() -> Result<PathBuf, GithubFetchError> {
    let base = dirs::cache_dir().ok_or_else(|| GithubFetchError {
        class: GithubFetchErrorClass::Archive,
        message: "could not determine user cache directory".to_string(),
    })?;
    Ok(base.join("rtango"))
}

#[derive(Deserialize)]
struct CommitResponse {
    sha: String,
}

fn resolve_ref(
    owner: &str,
    repo: &str,
    git_ref: &str,
    token: Option<&str>,
) -> Result<String, GithubFetchError> {
    let target = format!("{owner}/{repo}@{git_ref}");
    let url = format!("https://api.github.com/repos/{owner}/{repo}/commits/{git_ref}");
    let mut req = ureq::get(&url)
        .set("User-Agent", USER_AGENT)
        .set("Accept", "application/vnd.github+json")
        .set("X-GitHub-Api-Version", "2022-11-28");
    if let Some(token) = token {
        req = req.set("Authorization", &format!("Bearer {token}"));
    }

    let resp = match req.call() {
        Ok(resp) => resp,
        Err(ureq::Error::Status(code, resp)) => {
            return Err(GithubFetchError::status("resolving", &target, code, resp));
        }
        Err(ureq::Error::Transport(err)) => {
            return Err(GithubFetchError::network("resolving", &target, err));
        }
    };

    let body: CommitResponse = resp
        .into_json()
        .map_err(|err| GithubFetchError::response("resolving", &target, err))?;
    Ok(body.sha)
}

fn github_token() -> Option<String> {
    github_token_from_env().or_else(github_token_from_gh_cli)
}

fn github_token_from_env() -> Option<String> {
    std::env::var("RTANGO_GITHUB_TOKEN")
        .or_else(|_| std::env::var("GITHUB_TOKEN"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn github_token_from_gh_cli() -> Option<String> {
    let output = Command::new("gh")
        .args(["auth", "token"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let token = String::from_utf8(output.stdout).ok()?;
    let token = token.trim();
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

fn gh_cli_available() -> bool {
    Command::new("gh")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn can_prompt_for_github_cli_login() -> bool {
    io::stdin().is_terminal()
        && io::stdout().is_terminal()
        && io::stderr().is_terminal()
        && gh_cli_available()
}

fn prompt_for_github_cli_login(
    source: &GithubSource,
    err: &GithubFetchError,
) -> Result<bool, GithubFetchError> {
    let mut stderr = io::stderr().lock();
    writeln!(stderr, "\n{err}").map_err(GithubFetchError::prompt_io)?;
    writeln!(
        stderr,
        "rtango can retry {} after authenticating with GitHub CLI.",
        describe_github_source(source)
    )
    .map_err(GithubFetchError::prompt_io)?;
    write!(
        stderr,
        "Press Enter to run `gh auth login`, or type 'abort' to stop: "
    )
    .map_err(GithubFetchError::prompt_io)?;
    stderr.flush().map_err(GithubFetchError::prompt_io)?;
    drop(stderr);

    let mut line = String::new();
    let n = io::stdin()
        .lock()
        .read_line(&mut line)
        .map_err(GithubFetchError::prompt_io)?;
    if n == 0 {
        return Ok(false);
    }

    let answer = line.trim().to_ascii_lowercase();
    if matches!(answer.as_str(), "" | "y" | "yes" | "run" | "login") {
        let status = Command::new("gh")
            .args(["auth", "login"])
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|err| GithubFetchError::gh_cli("start `gh auth login`", err))?;
        if status.success() {
            return Ok(true);
        }
        return Err(GithubFetchError::gh_cli_login_failed(source));
    }

    Ok(false)
}

fn download_and_extract(
    owner: &str,
    repo: &str,
    sha: &str,
    token: Option<&str>,
    dest: &Path,
) -> Result<(), GithubFetchError> {
    let target = format!("{owner}/{repo}@{sha}");
    let url = format!("https://codeload.github.com/{owner}/{repo}/tar.gz/{sha}");
    let mut req = ureq::get(&url).set("User-Agent", USER_AGENT);
    if let Some(token) = token {
        req = req.set("Authorization", &format!("Bearer {token}"));
    }

    let resp = match req.call() {
        Ok(resp) => resp,
        Err(ureq::Error::Status(code, resp)) => {
            return Err(GithubFetchError::status("downloading", &target, code, resp));
        }
        Err(ureq::Error::Transport(err)) => {
            return Err(GithubFetchError::network("downloading", &target, err));
        }
    };

    let parent = dest.parent().ok_or_else(|| {
        GithubFetchError::archive(
            "preparing the cache directory for",
            &target,
            format!("cache destination has no parent: {}", dest.display()),
        )
    })?;
    fs::create_dir_all(parent).map_err(|err| {
        GithubFetchError::archive(
            "creating the cache directory for",
            &target,
            format!("{} ({})", parent.display(), err),
        )
    })?;

    let staging = parent.join(format!(".staging-{sha}"));
    if staging.exists() {
        fs::remove_dir_all(&staging).ok();
    }
    fs::create_dir_all(&staging).map_err(|err| {
        GithubFetchError::archive(
            "creating the staging directory for",
            &target,
            format!("{} ({})", staging.display(), err),
        )
    })?;

    let decoder = GzDecoder::new(resp.into_reader());
    let mut archive = Archive::new(decoder);
    archive
        .unpack(&staging)
        .map_err(|err| GithubFetchError::archive("extracting", &target, err))?;

    let inner = single_top_level_dir(&staging).map_err(|err| {
        GithubFetchError::archive("inspecting the downloaded archive for", &target, err)
    })?;

    if dest.exists() {
        fs::remove_dir_all(&staging).ok();
        return Ok(());
    }
    fs::rename(&inner, dest).map_err(|err| {
        GithubFetchError::archive(
            "installing the cache for",
            &target,
            format!("{} ({})", dest.display(), err),
        )
    })?;
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

fn response_detail(body: &str) -> Option<String> {
    let compact = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.is_empty() {
        return None;
    }

    let needs_ellipsis = compact.chars().count() > 200;
    let mut truncated = compact.chars().take(200).collect::<String>();
    if needs_ellipsis {
        truncated.push('…');
    }
    Some(truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_source_errors_are_not_ignorable() {
        let err = GithubFetchError::invalid_source("owner/repo/extra");
        assert!(!err.is_ignorable_fetch_failure());
        assert!(!err.should_offer_login());
    }

    #[test]
    fn rate_limit_errors_offer_login_and_are_ignorable() {
        let err = GithubFetchError {
            class: GithubFetchErrorClass::RateLimited,
            message: "rate limited".into(),
        };
        assert!(err.is_ignorable_fetch_failure());
        assert!(err.should_offer_login());
    }

    #[test]
    fn describe_github_source_includes_optional_path() {
        assert_eq!(
            describe_github_source(&GithubSource {
                github: "owner/repo".into(),
                r#ref: "main".into(),
                path: "skills/demo".into(),
            }),
            "owner/repo@main:skills/demo"
        );
    }
}
