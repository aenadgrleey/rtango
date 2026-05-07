use std::collections::HashSet;
use std::fs;
use std::path::Path;

use anyhow::Context;

use crate::error::RtangoError;

use super::{Lock, Spec};

const RTANGO_DIR: &str = ".rtango";
const SPEC_FILE: &str = "spec.yaml";
const LOCK_FILE: &str = "lock.yaml";
const GITIGNORE_FILE: &str = ".gitignore";
const GITIGNORE_START: &str = "# >>> rtango managed targets >>>";
const GITIGNORE_END: &str = "# <<< rtango managed targets <<<";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitignoreUpdate {
    pub existed: bool,
    pub changed: bool,
    pub content: String,
}

pub fn rtango_dir(root: &Path) -> std::path::PathBuf {
    root.join(RTANGO_DIR)
}

pub fn spec_path(root: &Path) -> std::path::PathBuf {
    rtango_dir(root).join(SPEC_FILE)
}

pub fn lock_path(root: &Path) -> std::path::PathBuf {
    rtango_dir(root).join(LOCK_FILE)
}

pub fn gitignore_path(root: &Path) -> std::path::PathBuf {
    root.join(GITIGNORE_FILE)
}

pub fn load_spec(root: &Path) -> anyhow::Result<Spec> {
    let path = spec_path(root);
    let content =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let spec: Spec = serde_yml::from_str(&content)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    validate_spec(&spec)?;
    Ok(spec)
}

/// Parse a spec from a raw YAML string. Used for loading remote collection specs
/// that were fetched from GitHub.
pub fn parse_spec_content(content: &str, source_desc: &str) -> anyhow::Result<Spec> {
    let spec: Spec = serde_yml::from_str(content)
        .with_context(|| format!("failed to parse spec from {source_desc}"))?;
    validate_spec(&spec)?;
    Ok(spec)
}

pub fn validate_spec(spec: &Spec) -> anyhow::Result<()> {
    if spec.version != 1 {
        anyhow::bail!(RtangoError::InvalidSpec(format!(
            "unsupported version {}, expected 1",
            spec.version
        )));
    }
    if spec.agents.is_empty() {
        anyhow::bail!(RtangoError::InvalidSpec(
            "agents list must not be empty".into()
        ));
    }
    let mut seen = HashSet::new();
    for rule in &spec.rules {
        if !seen.insert(&rule.id) {
            anyhow::bail!(RtangoError::InvalidSpec(format!(
                "duplicate rule id '{}'",
                rule.id
            )));
        }
    }
    Ok(())
}

pub fn save_spec(root: &Path, spec: &Spec) -> anyhow::Result<()> {
    let path = spec_path(root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let yaml = serde_yml::to_string(spec)?;
    fs::write(&path, yaml).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

pub fn load_lock(root: &Path) -> anyhow::Result<Lock> {
    let path = lock_path(root);
    let content =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let lock: Lock = serde_yml::from_str(&content)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(lock)
}

pub fn load_lock_or_empty(root: &Path) -> anyhow::Result<Lock> {
    let path = lock_path(root);
    if !path.exists() {
        return Ok(Lock {
            version: 1,
            tracked_agents: vec![],
            owners: vec![],
            deployments: vec![],
        });
    }
    load_lock(root)
}

pub fn save_lock(root: &Path, lock: &Lock) -> anyhow::Result<()> {
    let path = lock_path(root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let yaml = serde_yml::to_string(lock)?;
    fs::write(&path, yaml).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

pub fn gitignore_update(root: &Path, entries: &[String]) -> anyhow::Result<GitignoreUpdate> {
    let path = gitignore_path(root);
    let existed = path.exists();
    let existing = if existed {
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?
    } else {
        String::new()
    };
    let content = render_gitignore(&existing, entries)?;
    let changed = content != existing;
    Ok(GitignoreUpdate {
        existed,
        changed,
        content,
    })
}

pub fn write_gitignore(root: &Path, content: &str) -> anyhow::Result<()> {
    let path = gitignore_path(root);
    fs::write(&path, content).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn render_gitignore(existing: &str, entries: &[String]) -> anyhow::Result<String> {
    let mut preserved = Vec::new();
    let mut in_managed_block = false;
    let mut saw_start = false;
    let mut saw_end = false;

    for line in existing.lines() {
        if line == GITIGNORE_START {
            if saw_start {
                anyhow::bail!("malformed .gitignore: duplicate rtango managed block start marker");
            }
            saw_start = true;
            in_managed_block = true;
            continue;
        }
        if line == GITIGNORE_END {
            if !in_managed_block {
                anyhow::bail!("malformed .gitignore: rtango managed block end marker without start marker");
            }
            saw_end = true;
            in_managed_block = false;
            continue;
        }
        if !in_managed_block {
            preserved.push(line);
        }
    }

    if in_managed_block || saw_start != saw_end {
        anyhow::bail!("malformed .gitignore: unterminated rtango managed block");
    }

    let mut out = preserved.join("\n").trim_end().to_string();
    if !entries.is_empty() {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(GITIGNORE_START);
        out.push('\n');
        out.push_str(&entries.join("\n"));
        out.push('\n');
        out.push_str(GITIGNORE_END);
    }

    if out.is_empty() {
        Ok(String::new())
    } else {
        out.push('\n');
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::render_gitignore;

    #[test]
    fn appends_managed_block() {
        let content = render_gitignore("target/\n", &[".pi/skills/foo/".into()]).unwrap();
        assert_eq!(
            content,
            "target/\n\n# >>> rtango managed targets >>>\n.pi/skills/foo/\n# <<< rtango managed targets <<<\n"
        );
    }

    #[test]
    fn replaces_existing_managed_block() {
        let existing = "target/\n\n# >>> rtango managed targets >>>\n.old/\n# <<< rtango managed targets <<<\n";
        let content = render_gitignore(existing, &[".pi/skills/foo/".into()]).unwrap();
        assert_eq!(
            content,
            "target/\n\n# >>> rtango managed targets >>>\n.pi/skills/foo/\n# <<< rtango managed targets <<<\n"
        );
    }

    #[test]
    fn removes_managed_block_when_no_entries_remain() {
        let existing = "target/\n\n# >>> rtango managed targets >>>\n.old/\n# <<< rtango managed targets <<<\n";
        let content = render_gitignore(existing, &[]).unwrap();
        assert_eq!(content, "target/\n");
    }

    #[test]
    fn replaces_existing_managed_block_in_crlf_file() {
        let existing = "target/\r\n\r\n# >>> rtango managed targets >>>\r\n.old/\r\n# <<< rtango managed targets <<<\r\n";
        let content = render_gitignore(existing, &[".pi/skills/foo/".into()]).unwrap();
        assert_eq!(
            content,
            "target/\n\n# >>> rtango managed targets >>>\n.pi/skills/foo/\n# <<< rtango managed targets <<<\n"
        );
        assert_eq!(content.matches("# >>> rtango managed targets >>>").count(), 1);
    }
}
