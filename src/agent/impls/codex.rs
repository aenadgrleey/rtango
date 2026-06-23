use std::path::{Path, PathBuf};

use crate::agent::detect::{self, DetectedAgent, DetectedSource, Detector, SourceKind};
use crate::agent::frontmatter::{self, FrontMatter, FrontMatterMapper};
use crate::agent::parse::{self, AgentsParser, SkillsParser};
use crate::agent::permission::Permission;
use crate::agent::write::{self, AgentsWriter, FrontMatterWriter, SkillsWriter};
use crate::agent::{Agent, AgentSet, Skill, SkillSet};
use crate::spec::AgentName;

const NAME: &str = "codex";
const DIR: &str = ".codex";

pub struct CodexParser;

impl SkillsParser for CodexParser {
    fn name(&self) -> AgentName {
        AgentName::new(NAME)
    }
    fn parse_skills(&self, root: &Path) -> anyhow::Result<SkillSet> {
        parse::parse_standard_skills(&root.join(format!("{DIR}/skills")), self)
    }
    fn parse_skills_in(&self, dir: &Path) -> anyhow::Result<SkillSet> {
        parse::parse_standard_skills(dir, self)
    }
}

impl AgentsParser for CodexParser {
    fn name(&self) -> AgentName {
        AgentName::new(NAME)
    }
    fn parse_agents(&self, root: &Path) -> anyhow::Result<AgentSet> {
        parse::parse_standard_agents(&root.join(format!("{DIR}/agents")), self)
    }
    fn parse_agents_in(&self, dir: &Path) -> anyhow::Result<AgentSet> {
        parse::parse_standard_agents(dir, self)
    }
}

impl FrontMatterMapper for CodexParser {
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
        frontmatter::parse_standard_frontmatter(yaml, self)
    }
}

impl FrontMatterWriter for CodexParser {
    fn format_permission(&self, perm: &Permission) -> Option<String> {
        match perm {
            Permission::Read => Some("read".into()),
            Permission::Write => Some("write".into()),
            Permission::Edit => Some("edit".into()),
            Permission::Shell(_) => Some("bash".into()),
            Permission::Grep => Some("grep".into()),
            Permission::Glob => Some("glob".into()),
            Permission::WebFetch => Some("web_fetch".into()),
            Permission::WebSearch => Some("web_search".into()),
            Permission::Other(s) => Some(s.clone()),
            Permission::NotebookEdit | Permission::TodoWrite => None,
        }
    }

    fn format_frontmatter(&self, fm: &FrontMatter) -> String {
        write::format_standard_frontmatter(fm, self)
    }
}

impl SkillsWriter for CodexParser {
    fn name(&self) -> AgentName {
        AgentName::new(NAME)
    }
    fn write_skill(&self, root: &Path, skill: &Skill) -> anyhow::Result<PathBuf> {
        write::write_standard_skill(&root.join(format!("{DIR}/skills")), skill, self)
    }
}

impl AgentsWriter for CodexParser {
    fn name(&self) -> AgentName {
        AgentName::new(NAME)
    }
    fn write_agent(&self, root: &Path, agent: &Agent) -> anyhow::Result<PathBuf> {
        write::write_standard_agent(&root.join(format!("{DIR}/agents")), agent, self)
    }
}

impl Detector for CodexParser {
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
