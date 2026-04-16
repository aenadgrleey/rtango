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

    fn format_permission(&self, perm: &Permission) -> Option<String> {
        Some(match perm {
            Permission::Read => "read".into(),
            Permission::Write => "write".into(),
            Permission::Edit => "edit".into(),
            Permission::Shell(_) => "shell".into(),
            Permission::Grep => "grep".into(),
            Permission::Glob => "glob".into(),
            Permission::WebFetch => "web_fetch".into(),
            Permission::WebSearch => "web_search".into(),
            Permission::Other(s) => s.clone(),
            _ => return None,
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
