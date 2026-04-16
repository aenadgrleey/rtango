mod copilot;
mod claude_code;
pub mod frontmatter;
mod parse;
pub mod permission;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub use copilot::CopilotParser;
pub use claude_code::ClaudeCodeParser;
pub use frontmatter::{FrontMatter, FrontMatterMapper};
pub use parse::*;
pub use permission::Permission;

use crate::spec::AgentName;

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
