use std::path::{Path, PathBuf};

use crate::agent::detect::{self, DetectedAgent, DetectedSource, Detector, SourceKind};
use crate::agent::frontmatter::{self, FrontMatter, FrontMatterMapper};
use crate::agent::parse::{self, SkillsParser};
use crate::agent::permission::Permission;
use crate::agent::write::{self, FrontMatterWriter, SkillsWriter};
use crate::agent::{Skill, SkillSet};
use crate::spec::AgentName;

const NAME: &str = "hermes";
const DIR: &str = ".hermes";

pub struct HermesParser;

impl SkillsParser for HermesParser {
    fn name(&self) -> AgentName {
        AgentName::new(NAME)
    }
    fn parse_skills(&self, root: &Path) -> anyhow::Result<SkillSet> {
        parse::parse_standard_skills(&root.join(format!("{DIR}/skills")), self)
    }
}

impl FrontMatterMapper for HermesParser {
    fn parse_permission(&self, token: &str) -> Permission {
        match token {
            "read" | "read_file" => Permission::Read,
            "write" | "write_file" => Permission::Write,
            "edit" | "patch" => Permission::Edit,
            "shell" | "bash" | "terminal" => Permission::Shell(None),
            "grep" | "search_files" => Permission::Grep,
            "glob" => Permission::Glob,
            "web_fetch" | "web_extract" => Permission::WebFetch,
            "web_search" => Permission::WebSearch,
            other => Permission::Other(other.to_string()),
        }
    }

    fn parse_frontmatter(&self, yaml: &str) -> anyhow::Result<FrontMatter> {
        frontmatter::parse_standard_frontmatter(yaml, self)
    }
}

impl FrontMatterWriter for HermesParser {
    fn format_permission(&self, perm: &Permission) -> Option<String> {
        match perm {
            Permission::Read => Some("read_file".into()),
            Permission::Write => Some("write_file".into()),
            Permission::Edit => Some("patch".into()),
            Permission::Shell(_) => Some("terminal".into()),
            Permission::Grep => Some("search_files".into()),
            Permission::Glob => Some("search_files".into()),
            Permission::WebFetch => Some("web_extract".into()),
            Permission::WebSearch => Some("web_search".into()),
            Permission::Other(s) => Some(s.clone()),
            Permission::NotebookEdit | Permission::TodoWrite => None,
        }
    }

    fn format_frontmatter(&self, fm: &FrontMatter) -> String {
        write::format_standard_frontmatter(fm, self)
    }
}

impl SkillsWriter for HermesParser {
    fn name(&self) -> AgentName {
        AgentName::new(NAME)
    }
    fn write_skill(&self, root: &Path, skill: &Skill) -> anyhow::Result<PathBuf> {
        write::write_standard_skill(&root.join(format!("{DIR}/skills")), skill, self)
    }
}

impl Detector for HermesParser {
    fn name(&self) -> AgentName {
        AgentName::new(NAME)
    }

    fn detect(&self, root: &Path) -> Option<DetectedAgent> {
        let skills_dir = root.join(format!("{DIR}/skills"));
        if !detect::dir_has_standard_skills(&skills_dir) {
            return None;
        }
        Some(DetectedAgent {
            name: Detector::name(self),
            sources: vec![DetectedSource {
                id: format!("{NAME}-skills"),
                path: PathBuf::from(format!("{DIR}/skills/")),
                kind: SourceKind::SkillSet,
            }],
        })
    }
}
