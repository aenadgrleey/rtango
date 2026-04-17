mod expand;
mod fetch;
mod plan;
mod execute;

pub use expand::*;
pub use fetch::*;
pub use plan::*;
pub use execute::*;

use std::path::PathBuf;

use crate::spec::{AgentName, OnTargetModified, Source};

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
}

impl Plan {
    pub fn is_clean(&self) -> bool {
        self.items.iter().all(|d| d.status == DeploymentStatus::UpToDate)
    }

    pub fn has_conflicts(&self) -> bool {
        self.items.iter().any(|d| matches!(d.status, DeploymentStatus::Conflict { .. }))
    }

    pub fn has_orphans(&self) -> bool {
        self.items.iter().any(|d| d.status == DeploymentStatus::Orphan)
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
