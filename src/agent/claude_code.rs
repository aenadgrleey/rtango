use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::spec::AgentName;

use super::detect::{self, DetectedAgent, DetectedSource, Detector, SourceKind};
use super::frontmatter::{FrontMatter, FrontMatterMapper, tokenize_tools};
use super::parse::{self, AgentsParser, SkillsParser};
use super::permission::Permission;
use super::write::{self, AgentsWriter, FrontMatterWriter, SkillsWriter};
use super::{Agent, AgentSet, Skill, SkillSet};

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

impl FrontMatterWriter for ClaudeCodeParser {
    fn format_permission(&self, perm: &Permission) -> Option<String> {
        let token = match perm {
            Permission::Read => "Read".into(),
            Permission::Write => "Write".into(),
            Permission::Edit => "Edit".into(),
            Permission::Shell(None) => "Bash".into(),
            Permission::Shell(Some(pattern)) => format!("Bash({pattern})"),
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
        };
        Some(token)
    }

    fn format_frontmatter(&self, fm: &FrontMatter) -> String {
        write::format_standard_frontmatter(fm, self)
    }
}

impl SkillsWriter for ClaudeCodeParser {
    fn name(&self) -> AgentName {
        AgentName::new("claude-code")
    }

    fn write_skill(&self, root: &Path, skill: &Skill) -> anyhow::Result<PathBuf> {
        write::write_standard_skill(&root.join(".claude/skills"), skill, self)
    }
}

impl AgentsWriter for ClaudeCodeParser {
    fn name(&self) -> AgentName {
        AgentName::new("claude-code")
    }

    fn write_agent(&self, root: &Path, agent: &Agent) -> anyhow::Result<PathBuf> {
        write::write_standard_agent(&root.join(".claude/agents"), agent, self)
    }
}

impl Detector for ClaudeCodeParser {
    fn name(&self) -> AgentName {
        AgentName::new("claude-code")
    }

    fn detect(&self, root: &Path) -> Option<DetectedAgent> {
        let skills_dir = root.join(".claude/skills");
        let agents_dir = root.join(".claude/agents");

        let has_skills = detect::dir_has_standard_skills(&skills_dir);
        let has_agents = detect::dir_has_standard_agents(&agents_dir);

        if !has_skills && !has_agents {
            return None;
        }

        let mut sources = Vec::new();
        if has_skills {
            sources.push(DetectedSource {
                id: "claude-code-skills".into(),
                path: PathBuf::from(".claude/skills/"),
                kind: SourceKind::SkillSet,
            });
        }
        if has_agents {
            sources.push(DetectedSource {
                id: "claude-code-agents".into(),
                path: PathBuf::from(".claude/agents/"),
                kind: SourceKind::AgentSet,
            });
        }

        Some(DetectedAgent {
            name: Detector::name(self),
            sources,
        })
    }
}

fn extract_string(map: &BTreeMap<String, serde_yml::Value>, key: &str) -> Option<String> {
    map.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
}
