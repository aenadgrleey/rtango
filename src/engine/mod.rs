pub mod builtin;
mod execute;
mod expand;
mod fetch;
mod plan;

pub use execute::*;
pub use expand::*;
pub use fetch::{fetch_github, read_collection_spec};
pub use plan::*;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::spec::{AgentName, OnTargetModified, Ownership, Source};

/// What the engine intends to do with a single target file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeploymentStatus {
    /// Target does not exist yet — will be created.
    Create,
    /// Source changed since last sync — target will be overwritten.
    Update,
    /// Target was modified externally and policy is `Fail`.
    Conflict { reason: String },
    /// Target exists in lock but its rule/source no longer exists in spec.
    Orphan,
    /// Source and target hashes match the lock — nothing to do.
    UpToDate,
}

/// A single item in the sync plan.
#[derive(Debug, Clone)]
pub struct PlannedDeployment {
    pub rule_id: String,
    pub agent: AgentName,
    pub source: Source,
    /// Hash of the source file content.
    pub source_hash: String,
    /// Where the target file will be written (relative to root).
    pub target_path: PathBuf,
    /// The rendered content that would be written.
    pub rendered_content: String,
    pub status: DeploymentStatus,
}

/// The full sync plan: what the engine computed after expansion + diffing.
#[derive(Debug)]
pub struct Plan {
    pub items: Vec<PlannedDeployment>,
    /// Ownership decisions for contested paths, carried forward into the lock
    /// so the same resolution applies on the next sync.
    pub owners: Vec<Ownership>,
}

impl Plan {
    pub fn is_clean(&self) -> bool {
        self.items
            .iter()
            .all(|d| d.status == DeploymentStatus::UpToDate)
    }

    pub fn has_conflicts(&self) -> bool {
        self.items
            .iter()
            .any(|d| matches!(d.status, DeploymentStatus::Conflict { .. }))
    }

    pub fn has_orphans(&self) -> bool {
        self.items
            .iter()
            .any(|d| d.status == DeploymentStatus::Orphan)
    }
}

/// A single expanded item produced from a rule — a source file that should be
/// deployed to each target agent.
#[derive(Debug, Clone)]
pub struct ExpandedItem {
    pub rule_id: String,
    pub source: Source,
    /// Raw content of the source file.
    pub source_content: String,
    /// blake3 hash of source_content.
    pub source_hash: String,
    /// The kind of item (skill or agent) — determines which writer to use.
    pub kind: ExpandedKind,
}

#[derive(Debug, Clone)]
pub enum ExpandedKind {
    Skill(crate::agent::Skill),
    Agent(crate::agent::Agent),
    System(SystemFile),
}

/// A single root-level instruction file (CLAUDE.md / AGENTS.md / etc.).
/// Source content is written verbatim — no frontmatter parsing or rewriting.
#[derive(Debug, Clone)]
pub struct SystemFile {
    pub file: PathBuf,
    pub body: String,
}

/// Produced by rendering an ExpandedItem for a specific target agent.
#[derive(Debug, Clone)]
pub struct RenderedTarget {
    pub rule_id: String,
    pub agent: AgentName,
    pub source: Source,
    pub source_hash: String,
    /// Relative path where the file should be written.
    pub target_path: PathBuf,
    /// The full file content to write.
    pub content: String,
    /// blake3 hash of content.
    pub content_hash: String,
}

/// Hash file content with blake3, returning the hex digest.
pub fn hash_content(content: &str) -> String {
    blake3::hash(content.as_bytes()).to_hex().to_string()
}

/// Determine the effective `OnTargetModified` policy for a rule,
/// falling back to the spec default.
pub fn effective_policy(
    rule_policy: Option<OnTargetModified>,
    default: OnTargetModified,
) -> OnTargetModified {
    rule_policy.unwrap_or(default)
}

/// Compute the precise .gitignore entries for user-managed projected targets
/// in the current plan.
///
/// - Skills are ignored as leaf directories (`.pi/skills/foo/`), not whole
///   agent roots like `.pi/` or `.pi/skills/`.
/// - Agents and system files are ignored as individual files.
/// - Orphans are excluded because the current spec no longer manages them.
/// - Built-ins are excluded: the managed block reflects only user rule
///   projections.
/// - When `rule_filter` is set, only projections owned by that rule are
///   included. This keeps `status --rule ...` and `sync --rule ...` scoped to
///   the selected rule.
pub fn managed_gitignore_entries(plan: &Plan, rule_filter: Option<&str>) -> Vec<String> {
    let mut entries = BTreeSet::new();

    for item in &plan.items {
        if item.status == DeploymentStatus::Orphan || item.rule_id == builtin::BUILTIN_RTANGO_RULE_ID
        {
            continue;
        }
        if rule_filter.is_some_and(|rule_id| item.rule_id != rule_id) {
            continue;
        }
        entries.insert(gitignore_entry_for_target(&item.target_path));
    }

    entries.into_iter().collect()
}

fn gitignore_entry_for_target(target_path: &Path) -> String {
    let normalized = target_path.to_string_lossy().replace('\\', "/");
    if let Some(parent) = normalized.strip_suffix("/SKILL.md") {
        return format!("{parent}/");
    }
    normalized
}
