use std::path::Path;

use crate::spec::AgentName;

use super::parse::{self, AgentsParser, SkillsParser};
use super::{AgentSet, SkillSet};

pub struct CopilotParser;

impl SkillsParser for CopilotParser {
    fn name(&self) -> AgentName {
        AgentName::new("copilot")
    }

    fn parse_skills(&self, root: &Path) -> anyhow::Result<SkillSet> {
        parse::parse_standard_skills(&root.join(".github/skills"))
    }
}

impl AgentsParser for CopilotParser {
    fn name(&self) -> AgentName {
        AgentName::new("copilot")
    }

    fn parse_agents(&self, root: &Path) -> anyhow::Result<AgentSet> {
        parse::parse_standard_agents(&root.join(".github/agents"))
    }
}
