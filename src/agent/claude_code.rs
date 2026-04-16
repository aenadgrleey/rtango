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

    fn format_permission(&self, perm: &Permission) -> Option<String> {
        Some(match perm {
            Permission::Read => "Read".into(),
            Permission::Write => "Write".into(),
            Permission::Edit => "Edit".into(),
            Permission::Shell(None) => "Bash".into(),
            Permission::Shell(Some(p)) => format!("Bash({p})"),
            Permission::Grep => "Grep".into(),
            Permission::Glob => "Glob".into(),
            Permission::WebFetch => "WebFetch".into(),
            Permission::WebSearch => "WebSearch".into(),
            Permission::NotebookRead => "NotebookRead".into(),
            Permission::NotebookEdit => "NotebookEdit".into(),
            Permission::TodoRead => "TodoRead".into(),
            Permission::TodoWrite => "TodoWrite".into(),
            Permission::ListDir => "LS".into(),
            Permission::Other(s) => s.clone(),
        })
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

    fn serialize_frontmatter(&self, fm: &FrontMatter) -> anyhow::Result<String> {
        let mut map = BTreeMap::<String, serde_yml::Value>::new();

        if let Some(name) = &fm.name {
            map.insert("name".into(), serde_yml::Value::String(name.clone()));
        }
        if let Some(desc) = &fm.description {
            map.insert("description".into(), serde_yml::Value::String(desc.clone()));
        }
        if !fm.allowed_tools.is_empty() {
            let tools: Vec<String> = fm.allowed_tools.iter()
                .filter_map(|p| self.format_permission(p))
                .collect();
            if !tools.is_empty() {
                map.insert("allowed-tools".into(), serde_yml::Value::String(tools.join(" ")));
            }
        }

        for (k, v) in &fm.extra {
            map.entry(k.clone()).or_insert_with(|| v.clone());
        }

        Ok(serde_yml::to_string(&map)?)
    }
}

fn extract_string(map: &BTreeMap<String, serde_yml::Value>, key: &str) -> Option<String> {
    map.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
}
