use std::path::{Path, PathBuf};

use crate::agent::detect::{self, DetectedAgent, DetectedSource, Detector, SourceKind};
use crate::agent::frontmatter::{self, FrontMatter, FrontMatterMapper};
use crate::agent::parse::{self, SkillsParser};
use crate::agent::permission::Permission;
use crate::agent::write::{self, FrontMatterWriter, SkillsWriter};
use crate::agent::{Skill, SkillSet};
use crate::spec::AgentName;

pub struct ClaudeCodeParser;

impl SkillsParser for ClaudeCodeParser {
    fn name(&self) -> AgentName {
        AgentName::new("claude-code")
    }
    fn parse_skills(&self, root: &Path) -> anyhow::Result<SkillSet> {
        parse::parse_standard_skills(&root.join(".claude/skills"), self)
    }
}

impl FrontMatterMapper for ClaudeCodeParser {
    fn parse_permission(&self, token: &str) -> Permission {
        if let Some(inner) = token
            .strip_prefix("Bash(")
            .and_then(|s| s.strip_suffix(')'))
        {
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
            "NotebookEdit" => Permission::NotebookEdit,
            "TodoWrite" => Permission::TodoWrite,
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
            Permission::NotebookEdit => "NotebookEdit".into(),
            Permission::TodoWrite => "TodoWrite".into(),
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

impl Detector for ClaudeCodeParser {
    fn name(&self) -> AgentName {
        AgentName::new("claude-code")
    }

    fn detect(&self, root: &Path) -> Option<DetectedAgent> {
        let skills_dir = root.join(".claude/skills");
        if !detect::dir_has_standard_skills(&skills_dir) {
            return None;
        }
        Some(DetectedAgent {
            name: Detector::name(self),
            sources: vec![DetectedSource {
                id: "claude-code-skills".into(),
                path: PathBuf::from(".claude/skills/"),
                kind: SourceKind::SkillSet,
            }],
        })
    }
}
