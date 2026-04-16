use std::collections::BTreeMap;
use std::path::Path;

use crate::spec::AgentName;

use super::frontmatter::{FrontMatter, FrontMatterMapper, tokenize_tools};
use super::parse::{self, AgentsParser, SkillsParser};
use super::permission::Permission;
use super::{AgentSet, SkillSet};

pub struct ClaudeCodeParser;

impl SkillsParser for ClaudeCodeParser {
    fn name(&self) -> AgentName {
        AgentName::new("claude-code")
    }

    fn parse_skills(&self, root: &Path) -> anyhow::Result<SkillSet> {
        parse::parse_standard_skills(&root.join(".claude/skills"), self)
    }
}

impl AgentsParser for ClaudeCodeParser {
    fn name(&self) -> AgentName {
        AgentName::new("claude-code")
    }

    fn parse_agents(&self, root: &Path) -> anyhow::Result<AgentSet> {
        parse::parse_standard_agents(&root.join(".claude/agents"), self)
    }
}

impl FrontMatterMapper for ClaudeCodeParser {
    fn parse_permission(&self, token: &str) -> Permission {
        if let Some(inner) = token.strip_prefix("Bash(").and_then(|s| s.strip_suffix(')')) {
            return Permission::Shell(Some(inner.to_string()));
        }
        match token {
            "Read" => Permission::Read,
            "Write" => Permission::Write,
            "Edit" | "MultiEdit" => Permission::Edit,
            "Bash" => Permission::Shell(None),
            "Grep" => Permission::Grep,
            "Glob" => Permission::Glob,
            "WebFetch" => Permission::WebFetch,
            "WebSearch" => Permission::WebSearch,
            "NotebookRead" => Permission::NotebookRead,
            "NotebookEdit" => Permission::NotebookEdit,
            "TodoRead" => Permission::TodoRead,
            "TodoWrite" => Permission::TodoWrite,
            "LS" => Permission::ListDir,
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
