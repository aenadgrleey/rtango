use std::collections::HashSet;
use std::fs;
use std::path::Path;

use anyhow::Context;

use crate::error::RtangoError;

use super::{Lock, Spec};

const RTANGO_DIR: &str = ".rtango";
const SPEC_FILE: &str = "spec.yaml";
const LOCK_FILE: &str = "lock.yaml";

pub fn rtango_dir(root: &Path) -> std::path::PathBuf {
    root.join(RTANGO_DIR)
}

pub fn spec_path(root: &Path) -> std::path::PathBuf {
    rtango_dir(root).join(SPEC_FILE)
}

pub fn lock_path(root: &Path) -> std::path::PathBuf {
    rtango_dir(root).join(LOCK_FILE)
}

pub fn load_spec(root: &Path) -> anyhow::Result<Spec> {
    let path = spec_path(root);
    let content = fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let spec: Spec = serde_yml::from_str(&content)
        .with_context(|| format!("failed to parse {}", path.display()))?;
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

pub fn load_lock(root: &Path) -> anyhow::Result<Lock> {
    let path = lock_path(root);
    let content = fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
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
