use std::path::{Path, PathBuf};

use crate::agent::detect::{self, DetectedAgent, DetectedSource, Detector, SourceKind};
use crate::agent::frontmatter::{self, FrontMatter, FrontMatterMapper};
use crate::agent::parse::{self, AgentsParser, SkillsParser};
use crate::agent::permission::Permission;
use crate::agent::write::{self, AgentsWriter, FrontMatterWriter, SkillsWriter};
use crate::agent::{Agent, AgentSet, Skill, SkillSet};
use crate::spec::AgentName;

pub struct ClaudeCodeParser;

impl SkillsParser for ClaudeCodeParser {
    fn name(&self) -> AgentName { AgentName::new("claude-code") }
    fn parse_skills(&self, root: &Path) -> anyhow::Result<SkillSet> {
        parse::parse_standard_skills(&root.join(".claude/skills"), self)
    }
}

impl AgentsParser for ClaudeCodeParser {
    fn name(&self) -> AgentName { AgentName::new("claude-code") }
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
        frontmatter::parse_standard_frontmatter(yaml, self)
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
    fn name(&self) -> AgentName { AgentName::new("claude-code") }
    fn write_skill(&self, root: &Path, skill: &Skill) -> anyhow::Result<PathBuf> {
        write::write_standard_skill(&root.join(".claude/skills"), skill, self)
    }
}

impl AgentsWriter for ClaudeCodeParser {
    fn name(&self) -> AgentName { AgentName::new("claude-code") }
    fn write_agent(&self, root: &Path, agent: &Agent) -> anyhow::Result<PathBuf> {
        write::write_standard_agent(&root.join(".claude/agents"), agent, self)
    }
}

impl Detector for ClaudeCodeParser {
    fn name(&self) -> AgentName { AgentName::new("claude-code") }

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
