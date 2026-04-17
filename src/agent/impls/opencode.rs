use std::path::{Path, PathBuf};

use crate::agent::detect::{self, DetectedAgent, DetectedSource, Detector, SourceKind};
use crate::agent::frontmatter::{self, FrontMatter, FrontMatterMapper};
use crate::agent::parse::{self, AgentsParser, SkillsParser};
use crate::agent::permission::Permission;
use crate::agent::write::{self, AgentsWriter, FrontMatterWriter, SkillsWriter};
use crate::agent::{Agent, AgentSet, Skill, SkillSet};
use crate::spec::AgentName;

const NAME: &str = "opencode";
const DIR: &str = ".opencode";

pub struct OpenCodeParser;

impl SkillsParser for OpenCodeParser {
    fn name(&self) -> AgentName {
        AgentName::new(NAME)
    }
    fn parse_skills(&self, root: &Path) -> anyhow::Result<SkillSet> {
        parse::parse_standard_skills(&root.join(format!("{DIR}/skills")), self)
    }
}

impl AgentsParser for OpenCodeParser {
    fn name(&self) -> AgentName {
        AgentName::new(NAME)
    }
    fn parse_agents(&self, root: &Path) -> anyhow::Result<AgentSet> {
        parse::parse_standard_agents(&root.join(format!("{DIR}/agents")), self)
    }
}

impl FrontMatterMapper for OpenCodeParser {
    fn parse_permission(&self, token: &str) -> Permission {
        match token {
            "read" => Permission::Read,
            "write" => Permission::Write,
            "edit" => Permission::Edit,
            "shell" | "bash" => Permission::Shell(None),
            "grep" => Permission::Grep,
            "glob" => Permission::Glob,
            "webfetch" => Permission::WebFetch,
            "websearch" => Permission::WebSearch,
            other => Permission::Other(other.to_string()),
        }
    }

    fn parse_frontmatter(&self, yaml: &str) -> anyhow::Result<FrontMatter> {
        frontmatter::parse_standard_frontmatter(yaml, self)
    }
}

impl FrontMatterWriter for OpenCodeParser {
    fn format_permission(&self, perm: &Permission) -> Option<String> {
        match perm {
            Permission::Read => Some("read".into()),
            Permission::Write => Some("write".into()),
            Permission::Edit => Some("edit".into()),
            Permission::Shell(_) => Some("bash".into()),
            Permission::Grep => Some("grep".into()),
            Permission::Glob => Some("glob".into()),
            Permission::WebFetch => Some("webfetch".into()),
            Permission::WebSearch => Some("websearch".into()),
            Permission::Other(s) => Some(s.clone()),
            Permission::NotebookRead
            | Permission::NotebookEdit
            | Permission::TodoRead
            | Permission::TodoWrite
            | Permission::ListDir => None,
        }
    }

    fn format_frontmatter(&self, fm: &FrontMatter) -> String {
        write::format_standard_frontmatter(fm, self)
    }
}

impl SkillsWriter for OpenCodeParser {
    fn name(&self) -> AgentName {
        AgentName::new(NAME)
    }
    fn write_skill(&self, root: &Path, skill: &Skill) -> anyhow::Result<PathBuf> {
        write::write_standard_skill(&root.join(format!("{DIR}/skills")), skill, self)
    }
}

impl AgentsWriter for OpenCodeParser {
    fn name(&self) -> AgentName {
        AgentName::new(NAME)
    }
    fn write_agent(&self, root: &Path, agent: &Agent) -> anyhow::Result<PathBuf> {
        write::write_standard_agent(&root.join(format!("{DIR}/agents")), agent, self)
    }
}

impl Detector for OpenCodeParser {
    fn name(&self) -> AgentName {
        AgentName::new(NAME)
    }

    fn detect(&self, root: &Path) -> Option<DetectedAgent> {
        let skills_dir = root.join(format!("{DIR}/skills"));
        let agents_dir = root.join(format!("{DIR}/agents"));

        let has_skills = detect::dir_has_standard_skills(&skills_dir);
        let has_agents = detect::dir_has_standard_agents(&agents_dir);

        if !has_skills && !has_agents {
            return None;
        }

        let mut sources = Vec::new();
        if has_skills {
            sources.push(DetectedSource {
                id: format!("{NAME}-skills"),
                path: PathBuf::from(format!("{DIR}/skills/")),
                kind: SourceKind::SkillSet,
            });
        }
        if has_agents {
            sources.push(DetectedSource {
                id: format!("{NAME}-agents"),
                path: PathBuf::from(format!("{DIR}/agents/")),
                kind: SourceKind::AgentSet,
            });
        }

        Some(DetectedAgent {
            name: Detector::name(self),
            sources,
        })
    }
}
