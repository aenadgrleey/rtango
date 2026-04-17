use std::fs;
use std::path::{Path, PathBuf};

use crate::spec::{AgentName, RuleKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceKind {
    SkillSet,
    AgentSet,
}

impl From<SourceKind> for RuleKind {
    fn from(kind: SourceKind) -> Self {
        match kind {
            SourceKind::SkillSet => RuleKind::skill_set(),
            SourceKind::AgentSet => RuleKind::agent_set(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DetectedSource {
    pub id: String,
    pub path: PathBuf,
    pub kind: SourceKind,
}

#[derive(Debug, Clone)]
pub struct DetectedAgent {
    pub name: AgentName,
    pub sources: Vec<DetectedSource>,
}

pub trait Detector {
    fn name(&self) -> AgentName;
    fn detect(&self, root: &Path) -> Option<DetectedAgent>;
}

/// Check if a directory contains at least one standard skill (`<name>/SKILL.md`).
pub fn dir_has_standard_skills(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    entries
        .filter_map(|e| e.ok())
        .any(|e| e.path().is_dir() && e.path().join("SKILL.md").is_file())
}

/// Check if a directory contains at least one standard agent (`*.agent.md`).
pub fn dir_has_standard_agents(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    entries
        .filter_map(|e| e.ok())
        .any(|e| {
            e.path().is_file()
                && e.file_name()
                    .to_string_lossy()
                    .ends_with(".agent.md")
        })
}
