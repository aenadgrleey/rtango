use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::source::Source;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spec {
    pub version: u32,
    #[serde(default)]
    pub agents: Vec<AgentName>,
    #[serde(default)]
    pub defaults: Defaults,
    #[serde(default)]
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentName(pub String);

impl AgentName {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A target registry for one rule. `home` is an optional agent-specific
/// profile root, primarily useful for running several isolated Codex
/// instances from one global spec.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Target {
    pub agent: AgentName,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub home: Option<PathBuf>,
}

impl Target {
    pub fn agent(agent: impl Into<String>) -> Self {
        Self {
            agent: AgentName::new(agent),
            home: None,
        }
    }
}

impl std::fmt::Display for AgentName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Defaults {
    #[serde(default)]
    pub on_target_modified: OnTargetModified,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub gitignore_targets: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OnTargetModified {
    #[default]
    Fail,
    Overwrite,
    Skip,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub id: String,
    pub source: Source,
    pub schema_agent: AgentName,
    /// Optional per-rule target override. When absent, `Spec.agents` applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub targets: Option<Vec<Target>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_target_modified: Option<OnTargetModified>,
    #[serde(flatten)]
    pub kind: RuleKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RuleKind {
    Skill {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        allowed_tools: Option<String>,
    },
    SkillSet {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        include: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        exclude: Vec<String>,
    },
    Agent {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        allowed_tools: Option<String>,
    },
    AgentSet {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        include: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        exclude: Vec<String>,
    },
    /// A root-level instruction file (CLAUDE.md, AGENTS.md, etc.).
    /// Source is one markdown file, written verbatim to the per-agent
    /// convention path with no frontmatter rewriting.
    System,

    /// A remote rtango collection: imports all (or filtered) rules from
    /// another repo's `.rtango/spec.yaml`. The `source` must be
    /// `Source::Collection`.
    Collection {
        /// Only import rules whose id matches one of these names.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        include: Vec<String>,
        /// Exclude rules whose id matches one of these names.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        exclude: Vec<String>,
        /// Override the schema_agent for all imported rules. If absent,
        /// each imported rule uses its own schema_agent from the remote spec.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        schema_override: Option<AgentName>,
    },
}

impl RuleKind {
    pub fn skill() -> Self {
        RuleKind::Skill {
            name: None,
            description: None,
            allowed_tools: None,
        }
    }
    pub fn skill_set() -> Self {
        RuleKind::SkillSet {
            include: Vec::new(),
            exclude: Vec::new(),
        }
    }
    pub fn agent() -> Self {
        RuleKind::Agent {
            name: None,
            description: None,
            allowed_tools: None,
        }
    }
    pub fn agent_set() -> Self {
        RuleKind::AgentSet {
            include: Vec::new(),
            exclude: Vec::new(),
        }
    }
    pub fn system() -> Self {
        RuleKind::System
    }
    pub fn collection() -> Self {
        RuleKind::Collection {
            include: Vec::new(),
            exclude: Vec::new(),
            schema_override: None,
        }
    }
}
