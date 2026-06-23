use std::path::{Path, PathBuf};

use crate::agent::detect::{self, DetectedAgent, DetectedSource, Detector, SourceKind};
use crate::agent::frontmatter::{self, FrontMatter, FrontMatterMapper};
use crate::agent::parse::{self, AgentsParser, SkillsParser};
use crate::agent::permission::Permission;
use crate::agent::write::{self, AgentsWriter, FrontMatterWriter, SkillsWriter};
use crate::agent::{Agent, AgentSet, Skill, SkillSet};
use crate::spec::AgentName;

const NAME: &str = "plain";

pub struct PlainParser;

impl SkillsParser for PlainParser {
    fn name(&self) -> AgentName {
        AgentName::new(NAME)
    }
    fn parse_skills(&self, root: &Path) -> anyhow::Result<SkillSet> {
        parse::parse_standard_skills(&root.join("skills"), self)
    }
    fn parse_skills_in(&self, dir: &Path) -> anyhow::Result<SkillSet> {
        parse::parse_standard_skills(dir, self)
    }
}

impl AgentsParser for PlainParser {
    fn name(&self) -> AgentName {
        AgentName::new(NAME)
    }
    fn parse_agents(&self, root: &Path) -> anyhow::Result<AgentSet> {
        parse::parse_standard_agents(&root.join("agents"), self)
    }
    fn parse_agents_in(&self, dir: &Path) -> anyhow::Result<AgentSet> {
        parse::parse_standard_agents(dir, self)
    }
}

impl FrontMatterMapper for PlainParser {
    fn parse_permission(&self, token: &str) -> Permission {
        if let Some(inner) = token
            .strip_prefix("Shell(")
            .and_then(|s| s.strip_suffix(')'))
        {
            return Permission::Shell(Some(inner.to_string()));
        }
        match token {
            "Read" => Permission::Read,
            "Write" => Permission::Write,
            "Edit" => Permission::Edit,
            "Shell" => Permission::Shell(None),
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

impl FrontMatterWriter for PlainParser {
    fn format_permission(&self, perm: &Permission) -> Option<String> {
        let token = match perm {
            Permission::Read => "Read".into(),
            Permission::Write => "Write".into(),
            Permission::Edit => "Edit".into(),
            Permission::Shell(None) => "Shell".into(),
            Permission::Shell(Some(pattern)) => format!("Shell({pattern})"),
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

impl SkillsWriter for PlainParser {
    fn name(&self) -> AgentName {
        AgentName::new(NAME)
    }
    fn write_skill(&self, root: &Path, skill: &Skill) -> anyhow::Result<PathBuf> {
        write::write_standard_skill(&root.join("skills"), skill, self)
    }
}

impl AgentsWriter for PlainParser {
    fn name(&self) -> AgentName {
        AgentName::new(NAME)
    }
    fn write_agent(&self, root: &Path, agent: &Agent) -> anyhow::Result<PathBuf> {
        write::write_standard_agent(&root.join("agents"), agent, self)
    }
}

impl Detector for PlainParser {
    fn name(&self) -> AgentName {
        AgentName::new(NAME)
    }

    fn detect(&self, root: &Path) -> Option<DetectedAgent> {
        let skills_dir = root.join("skills");
        let agents_dir = root.join("agents");

        let has_skills = detect::dir_has_standard_skills(&skills_dir);
        let has_agents = detect::dir_has_standard_agents(&agents_dir);

        if !has_skills && !has_agents {
            return None;
        }

        let mut sources = Vec::new();
        if has_skills {
            sources.push(DetectedSource {
                id: format!("{NAME}-skills"),
                path: PathBuf::from("skills/"),
                kind: SourceKind::SkillSet,
            });
        }
        if has_agents {
            sources.push(DetectedSource {
                id: format!("{NAME}-agents"),
                path: PathBuf::from("agents/"),
                kind: SourceKind::AgentSet,
            });
        }

        Some(DetectedAgent {
            name: Detector::name(self),
            sources,
        })
    }
}
