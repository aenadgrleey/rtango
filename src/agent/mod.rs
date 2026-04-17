pub mod detect;
pub mod frontmatter;
mod impls;
mod parse;
pub mod permission;
pub mod write;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub use impls::*;
pub use detect::{DetectedAgent, DetectedSource, Detector, SourceKind};
pub use frontmatter::{FrontMatter, FrontMatterMapper};
pub use parse::*;
pub use permission::Permission;
pub use write::*;

use crate::spec::AgentName;

fn all_parsers() -> Vec<Box<dyn AgentParser>> {
    vec![
        Box::new(CopilotParser),
        Box::new(ClaudeCodeParser),
        Box::new(CodexParser),
        Box::new(PiParser),
        Box::new(OpenCodeParser),
        Box::new(PlainParser),
    ]
}

/// Blanket trait combining all per-agent capabilities.
/// Every agent parser struct implements this via the individual traits.
pub trait AgentParser: SkillsParser + AgentsParser + FrontMatterMapper + FrontMatterWriter + SkillsWriter + AgentsWriter + Detector {}
impl AgentParser for CopilotParser {}
impl AgentParser for ClaudeCodeParser {}
impl AgentParser for CodexParser {}
impl AgentParser for PiParser {}
impl AgentParser for OpenCodeParser {}
impl AgentParser for PlainParser {}

pub fn skills_parser(name: &AgentName) -> Option<Box<dyn SkillsParser>> {
    match name.as_str() {
        "copilot" => Some(Box::new(CopilotParser)),
        "claude-code" => Some(Box::new(ClaudeCodeParser)),
        "codex" => Some(Box::new(CodexParser)),
        "pi" => Some(Box::new(PiParser)),
        "opencode" => Some(Box::new(OpenCodeParser)),
        "plain" => Some(Box::new(PlainParser)),
        _ => None,
    }
}

pub fn agents_parser(name: &AgentName) -> Option<Box<dyn AgentsParser>> {
    match name.as_str() {
        "copilot" => Some(Box::new(CopilotParser)),
        "claude-code" => Some(Box::new(ClaudeCodeParser)),
        "codex" => Some(Box::new(CodexParser)),
        "pi" => Some(Box::new(PiParser)),
        "opencode" => Some(Box::new(OpenCodeParser)),
        "plain" => Some(Box::new(PlainParser)),
        _ => None,
    }
}

pub fn frontmatter_mapper(name: &AgentName) -> Option<Box<dyn FrontMatterMapper>> {
    match name.as_str() {
        "copilot" => Some(Box::new(CopilotParser)),
        "claude-code" => Some(Box::new(ClaudeCodeParser)),
        "codex" => Some(Box::new(CodexParser)),
        "pi" => Some(Box::new(PiParser)),
        "opencode" => Some(Box::new(OpenCodeParser)),
        "plain" => Some(Box::new(PlainParser)),
        _ => None,
    }
}

pub fn frontmatter_writer(name: &AgentName) -> Option<Box<dyn FrontMatterWriter>> {
    match name.as_str() {
        "copilot" => Some(Box::new(CopilotParser)),
        "claude-code" => Some(Box::new(ClaudeCodeParser)),
        "codex" => Some(Box::new(CodexParser)),
        "pi" => Some(Box::new(PiParser)),
        "opencode" => Some(Box::new(OpenCodeParser)),
        "plain" => Some(Box::new(PlainParser)),
        _ => None,
    }
}

pub fn skills_writer(name: &AgentName) -> Option<Box<dyn SkillsWriter>> {
    match name.as_str() {
        "copilot" => Some(Box::new(CopilotParser)),
        "claude-code" => Some(Box::new(ClaudeCodeParser)),
        "codex" => Some(Box::new(CodexParser)),
        "pi" => Some(Box::new(PiParser)),
        "opencode" => Some(Box::new(OpenCodeParser)),
        "plain" => Some(Box::new(PlainParser)),
        _ => None,
    }
}

pub fn agents_writer(name: &AgentName) -> Option<Box<dyn AgentsWriter>> {
    match name.as_str() {
        "copilot" => Some(Box::new(CopilotParser)),
        "claude-code" => Some(Box::new(ClaudeCodeParser)),
        "codex" => Some(Box::new(CodexParser)),
        "pi" => Some(Box::new(PiParser)),
        "opencode" => Some(Box::new(OpenCodeParser)),
        "plain" => Some(Box::new(PlainParser)),
        _ => None,
    }
}

pub fn detector(name: &AgentName) -> Option<Box<dyn Detector>> {
    match name.as_str() {
        "copilot" => Some(Box::new(CopilotParser)),
        "claude-code" => Some(Box::new(ClaudeCodeParser)),
        "codex" => Some(Box::new(CodexParser)),
        "pi" => Some(Box::new(PiParser)),
        "opencode" => Some(Box::new(OpenCodeParser)),
        "plain" => Some(Box::new(PlainParser)),
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
