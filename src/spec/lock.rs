use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::source::Source;
use super::spec::AgentName;

#[derive(Debug, Serialize, Deserialize)]
pub struct Lock {
    pub version: u32,
    pub tracked_agents: Vec<AgentName>,
    #[serde(default)]
    pub deployments: Vec<Deployment>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Deployment {
    pub rule_id: String,
    pub agent: AgentName,
    pub source: Source,
    pub source_hash: String,
    pub content: PathBuf,
    pub content_hash: String,
}
