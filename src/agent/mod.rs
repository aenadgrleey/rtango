mod copilot;
mod claude_code;
pub mod detect;
pub mod frontmatter;
mod parse;
pub mod permission;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub use copilot::CopilotParser;
pub use claude_code::ClaudeCodeParser;
pub use detect::{DetectedAgent, DetectedSource, Detector, SourceKind};
pub use frontmatter::{FrontMatter, FrontMatterMapper};
pub use parse::*;
pub use permission::Permission;

use crate::spec::AgentName;

fn all_parsers() -> Vec<Box<dyn AgentParser>> {
    vec![
        Box::new(CopilotParser),
        Box::new(ClaudeCodeParser),
    ]
}

/// Blanket trait combining all per-agent capabilities.
/// Every agent parser struct implements this via the individual traits.
pub trait AgentParser: SkillsParser + AgentsParser + FrontMatterMapper + Detector {}
impl AgentParser for CopilotParser {}
impl AgentParser for ClaudeCodeParser {}

pub fn skills_parser(name: &AgentName) -> Option<Box<dyn SkillsParser>> {
    match name.as_str() {
        "copilot" => Some(Box::new(CopilotParser)),
        "claude-code" => Some(Box::new(ClaudeCodeParser)),
        _ => None,
    }
}

pub fn agents_parser(name: &AgentName) -> Option<Box<dyn AgentsParser>> {
    match name.as_str() {
        "copilot" => Some(Box::new(CopilotParser)),
        "claude-code" => Some(Box::new(ClaudeCodeParser)),
        _ => None,
    }
}

pub fn frontmatter_mapper(name: &AgentName) -> Option<Box<dyn FrontMatterMapper>> {
    match name.as_str() {
        "copilot" => Some(Box::new(CopilotParser)),
        "claude-code" => Some(Box::new(ClaudeCodeParser)),
        _ => None,
    }
}

pub fn detector(name: &AgentName) -> Option<Box<dyn Detector>> {
    match name.as_str() {
        "copilot" => Some(Box::new(CopilotParser)),
        "claude-code" => Some(Box::new(ClaudeCodeParser)),
        _ => None,
    }
}

/// Run detection across all known agent parsers.
pub fn detect_agents(root: &Path) -> Vec<DetectedAgent> {
    all_parsers()
        .iter()
        .filter_map(|p| p.detect(root))
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    pub dir: PathBuf,
    pub file: PathBuf,
    #[serde(skip)]
    pub front_matter: FrontMatter,
    #[serde(skip)]
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub name: String,
    pub file: PathBuf,
    #[serde(skip)]
    pub front_matter: FrontMatter,
    #[serde(skip)]
    pub body: String,
}

pub type SkillSet = Vec<Skill>;
pub type AgentSet = Vec<Agent>;
