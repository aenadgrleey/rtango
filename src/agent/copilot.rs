use std::collections::BTreeMap;
use std::path::Path;

use crate::spec::AgentName;

use super::frontmatter::{FrontMatter, FrontMatterMapper, tokenize_tools};
use super::parse::{self, AgentsParser, SkillsParser};
use super::permission::Permission;
use super::{AgentSet, SkillSet};

pub struct CopilotParser;

impl SkillsParser for CopilotParser {
    fn name(&self) -> AgentName {
        AgentName::new("copilot")
    }

    fn parse_skills(&self, root: &Path) -> anyhow::Result<SkillSet> {
        parse::parse_standard_skills(&root.join(".github/skills"), self)
    }
}

impl AgentsParser for CopilotParser {
    fn name(&self) -> AgentName {
        AgentName::new("copilot")
    }

    fn parse_agents(&self, root: &Path) -> anyhow::Result<AgentSet> {
        parse::parse_standard_agents(&root.join(".github/agents"), self)
    }
}

impl FrontMatterMapper for CopilotParser {
    fn parse_permission(&self, token: &str) -> Permission {
        match token {
            "read" => Permission::Read,
            "write" => Permission::Write,
            "edit" => Permission::Edit,
            "shell" | "bash" => Permission::Shell(None),
            "grep" => Permission::Grep,
            "glob" => Permission::Glob,
            "web_fetch" => Permission::WebFetch,
            "web_search" => Permission::WebSearch,
            other => Permission::Other(other.to_string()),
        }
    }

    fn parse_frontmatter(&self, yaml: &str) -> anyhow::Result<FrontMatter> {
        let raw: BTreeMap<String, serde_yml::Value> = serde_yml::from_str(yaml)?;
        let mut fm = FrontMatter::default();

        fm.name = extract_string(&raw, "name");
        fm.description = extract_string(&raw, "description");

        if let Some(tools_str) = extract_string(&raw, "allowed-tools") {
            fm.allowed_tools = tokenize_tools(&tools_str)
                .into_iter()
                .map(|t| self.parse_permission(&t))
                .collect();
        }

        fm.extra = raw
            .into_iter()
            .filter(|(k, _)| !matches!(k.as_str(), "name" | "description" | "allowed-tools"))
            .collect();

        Ok(fm)
    }

}

fn extract_string(map: &BTreeMap<String, serde_yml::Value>, key: &str) -> Option<String> {
    map.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
}
