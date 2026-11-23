use std::path::{Path, PathBuf};

use crate::agent::detect::{self, DetectedAgent, DetectedSource, Detector, SourceKind};
use crate::agent::frontmatter::{self, FrontMatter, FrontMatterMapper};
use crate::agent::parse::{self, SkillsParser};
use crate::agent::permission::Permission;
use crate::agent::write::{self, FrontMatterWriter, SkillsWriter};
use crate::agent::{Skill, SkillSet};
use crate::spec::AgentName;

const NAME: &str = "cursor";
const DIR: &str = ".cursor";

pub struct CursorParser;

impl SkillsParser for CursorParser {
    fn name(&self) -> AgentName {
        AgentName::new(NAME)
    }

    fn parse_skills(&self, root: &Path) -> anyhow::Result<SkillSet> {
        parse::parse_standard_skills(&root.join(format!("{DIR}/skills")), self)
    }
}

impl FrontMatterMapper for CursorParser {
    fn parse_permission(&self, token: &str) -> Permission {
        // Cursor's skill frontmatter does not define an `allowed-tools` field.
        // Keep unexpected tokens lossless in the canonical representation, but
        // do not emit them for Cursor.
        Permission::Other(token.to_string())
    }

    fn parse_frontmatter(&self, yaml: &str) -> anyhow::Result<FrontMatter> {
        frontmatter::parse_standard_frontmatter(yaml, self)
    }
}

impl FrontMatterWriter for CursorParser {
    fn format_permission(&self, _perm: &Permission) -> Option<String> {
        None
    }

    fn format_frontmatter(&self, fm: &FrontMatter) -> String {
        write::format_standard_frontmatter(fm, self)
    }
}

impl SkillsWriter for CursorParser {
    fn name(&self) -> AgentName {
        AgentName::new(NAME)
    }

    fn write_skill(&self, root: &Path, skill: &Skill) -> anyhow::Result<PathBuf> {
        write::write_standard_skill(&root.join(format!("{DIR}/skills")), skill, self)
    }
}

impl Detector for CursorParser {
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
