use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::source::Source;
use super::spec::AgentName;

#[derive(Debug, Serialize, Deserialize)]
pub struct Lock {
    pub version: u32,
    pub tracked_agents: Vec<AgentName>,
    /// Explicit ownership decisions for paths that were claimed by multiple
    /// rules. Only contested paths appear here; uncontested ones are derivable
    /// from the spec.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub owners: Vec<Ownership>,
    #[serde(default)]
    pub deployments: Vec<Deployment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Ownership {
    pub path: PathBuf,
    pub rule_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deployment {
    pub rule_id: String,
    pub agent: AgentName,
    pub source: Source,
    pub source_hash: String,
    pub content: PathBuf,
    pub content_hash: String,
}
