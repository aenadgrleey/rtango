use std::fs;
use std::path::{Path, PathBuf};

use crate::agent::detect::{self, DetectedAgent, DetectedSource, Detector, SourceKind};
use crate::agent::frontmatter::{self, FrontMatter, FrontMatterMapper, split_frontmatter};
use crate::agent::parse::{self, AgentsParser, SkillsParser};
use crate::agent::permission::Permission;
use crate::agent::write::{self, AgentsWriter, FrontMatterWriter, SkillsWriter};
use crate::agent::{Agent, AgentSet, Skill, SkillSet};
use crate::spec::AgentName;

const NAME: &str = "pi";
const DIR: &str = ".pi";

pub struct PiParser;

impl SkillsParser for PiParser {
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

impl AgentsParser for PiParser {
    fn name(&self) -> AgentName {
        AgentName::new(NAME)
    }
    fn parse_agents(&self, root: &Path) -> anyhow::Result<AgentSet> {
        let dir = root.join(format!("{DIR}/agents"));
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
            let Some(name) = file_name
                .strip_suffix(".agent.md")
                .or_else(|| file_name.strip_suffix(".md"))
            else {
                continue;
            };
            let content = fs::read_to_string(&path)?;
            let (yaml, body) = split_frontmatter(&content);
            let front_matter = match yaml {
                Some(y) => self.parse_frontmatter(y)?,
                None => FrontMatter::default(),
            };
            agents.push(Agent {
                name: name.to_owned(),
                file: path,
                front_matter,
                body: body.to_string(),
            });
        }
        agents.sort_by_key(|a| a.name.clone());
        Ok(agents)
    }

    fn parse_agents_in(&self, dir: &Path) -> anyhow::Result<AgentSet> {
        // pi agent files use either `<name>.agent.md` or `<name>.md`. The
        // default impl only knows about `.agent.md`, so this duplicates the
        // suffix handling here (without the `.pi/agents` prefix that
        // `parse_agents` uses).
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
            let Some(name) = file_name
                .strip_suffix(".agent.md")
                .or_else(|| file_name.strip_suffix(".md"))
            else {
                continue;
            };
            let content = fs::read_to_string(&path)?;
            let (yaml, body) = split_frontmatter(&content);
            let front_matter = match yaml {
                Some(y) => self.parse_frontmatter(y)?,
                None => FrontMatter::default(),
            };
            agents.push(Agent {
                name: name.to_owned(),
                file: path,
                front_matter,
                body: body.to_string(),
            });
        }
        agents.sort_by_key(|a| a.name.clone());
        Ok(agents)
    }
}

impl FrontMatterMapper for PiParser {
    fn parse_permission(&self, token: &str) -> Permission {
        Permission::Other(token.to_string())
    }

    fn parse_frontmatter(&self, yaml: &str) -> anyhow::Result<FrontMatter> {
        frontmatter::parse_standard_frontmatter(yaml, self)
    }
}

impl FrontMatterWriter for PiParser {
    fn format_permission(&self, _perm: &Permission) -> Option<String> {
        None
    }

    fn format_frontmatter(&self, fm: &FrontMatter) -> String {
        write::format_standard_frontmatter(fm, self)
    }
}

impl SkillsWriter for PiParser {
    fn name(&self) -> AgentName {
        AgentName::new(NAME)
    }
    fn write_skill(&self, root: &Path, skill: &Skill) -> anyhow::Result<PathBuf> {
        write::write_standard_skill(&root.join(format!("{DIR}/skills")), skill, self)
    }
}

impl AgentsWriter for PiParser {
    fn name(&self) -> AgentName {
        AgentName::new(NAME)
    }
    fn write_agent(&self, root: &Path, agent: &Agent) -> anyhow::Result<PathBuf> {
        let dir = root.join(format!("{DIR}/agents"));
        fs::create_dir_all(&dir)?;
        let file = dir.join(format!("{}.md", agent.name));
        let yaml = self.format_frontmatter(&agent.front_matter);
        let content = if yaml.is_empty() {
            agent.body.clone()
        } else {
            frontmatter::join_frontmatter(&yaml, &agent.body)
        };
        fs::write(&file, &content)?;
        Ok(file)
    }
}

impl Detector for PiParser {
    fn name(&self) -> AgentName {
        AgentName::new(NAME)
    }

    fn detect(&self, root: &Path) -> Option<DetectedAgent> {
        let skills_dir = root.join(format!("{DIR}/skills"));
        let agents_dir = root.join(format!("{DIR}/agents"));

        let has_skills = detect::dir_has_standard_skills(&skills_dir);
        let has_agents = agents_dir.is_dir()
            && fs::read_dir(&agents_dir)
                .ok()
                .map(|entries| {
                    entries.filter_map(Result::ok).any(|entry| {
                        let path = entry.path();
                        path.is_file()
                            && path
                                .file_name()
                                .and_then(|name| name.to_str())
                                .is_some_and(|name| name.ends_with(".md") && name != "README.md")
                    })
                })
                .unwrap_or(false);

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
