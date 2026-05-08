use std::fs;
use std::path::Path;

use crate::spec::AgentName;

use super::frontmatter::{FrontMatter, FrontMatterMapper, split_frontmatter};
use super::{Agent, AgentSet, Skill, SkillSet};

pub trait SkillsParser {
    fn name(&self) -> AgentName;
    fn parse_skills(&self, root: &Path) -> anyhow::Result<SkillSet>;
}

pub trait AgentsParser {
    fn name(&self) -> AgentName;
    fn parse_agents(&self, root: &Path) -> anyhow::Result<AgentSet>;
}

/// Parse skills from subdirectories containing `SKILL.md`.
///
/// Layout: `<dir>/<name>/SKILL.md`
pub fn parse_standard_skills(
    dir: &Path,
    mapper: &dyn FrontMatterMapper,
) -> anyhow::Result<SkillSet> {
    let mut skills = Vec::new();
    if !dir.is_dir() {
        return Ok(skills);
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let skill_file = path.join("SKILL.md");
        if skill_file.is_file() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let content = fs::read_to_string(&skill_file)?;
            let (yaml, body) = split_frontmatter(&content);
            let front_matter = match yaml {
                Some(y) => mapper.parse_frontmatter(y)?,
                None => FrontMatter::default(),
            };
            skills.push(Skill {
                name,
                dir: path,
                file: skill_file,
                front_matter,
                body: body.to_string(),
            });
        }
    }
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(skills)
}

/// Parse agents from flat `*.agent.md` files.
///
/// Layout: `<dir>/<name>.agent.md`
pub fn parse_standard_agents(
    dir: &Path,
    mapper: &dyn FrontMatterMapper,
) -> anyhow::Result<AgentSet> {
    let mut agents = Vec::new();
    if !dir.is_dir() {
        return Ok(agents);
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().into_owned();
        if let Some(name) = file_name.strip_suffix(".agent.md") {
            let content = fs::read_to_string(&path)?;
            let (yaml, body) = split_frontmatter(&content);
            let front_matter = match yaml {
                Some(y) => mapper.parse_frontmatter(y)?,
                None => FrontMatter::default(),
            };
            agents.push(Agent {
                name: name.to_owned(),
                file: path,
                front_matter,
                body: body.to_string(),
            });
        }
    }
    agents.sort_by_key(|a| a.name.clone());
    Ok(agents)
}
