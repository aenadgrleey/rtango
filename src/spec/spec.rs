use serde::{Deserialize, Serialize};

use super::source::Source;

#[derive(Debug, Serialize, Deserialize)]
pub struct Spec {
    pub version: u32,
    pub agents: Vec<AgentName>,
    #[serde(default)]
    pub defaults: Defaults,
    #[serde(default)]
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentName {
    Copilot,
    ClaudeCode,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Defaults {
    #[serde(default)]
    pub mode: DeployMode,
    #[serde(default)]
    pub on_target_modified: OnTargetModified,
}

impl Default for Defaults {
    fn default() -> Self {
        Self {
            mode: DeployMode::Copy,
            on_target_modified: OnTargetModified::default(),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeployMode {
    #[default]
    Copy,
    Symlink,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OnTargetModified {
    #[default]
    Fail,
    Overwrite,
    Skip,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Rule {
    pub id: String,
    pub source: Source,
    pub schema_agent: AgentName,
    pub mode: Option<DeployMode>,
    pub on_target_modified: Option<OnTargetModified>,
    #[serde(flatten)]
    pub kind: RuleKind,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RuleKind {
    Skill {},
    SkillSet {},
    Agent {},
    AgentSet {},
}
