use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::spec::AgentName;

use super::frontmatter::{FrontMatter, join_frontmatter};
use super::permission::Permission;
use super::{Agent, Skill};

/// Inverse of `FrontMatterMapper` — serialises canonical models back to
/// an agent-specific frontmatter string.
pub trait FrontMatterWriter {
    /// Convert a canonical `Permission` to the agent-native token.
    /// Returns `None` if the permission has no representation in this agent
    /// (e.g. `NotebookEdit` for Copilot).
    fn format_permission(&self, perm: &Permission) -> Option<String>;

    /// Serialise a `FrontMatter` into a YAML string (without the `---` fences).
    fn format_frontmatter(&self, fm: &FrontMatter) -> String;
}

pub trait SkillsWriter {
    fn name(&self) -> AgentName;

    /// Write a single skill to `<root>/<agent-dir>/skills/<skill.name>/SKILL.md`.
    /// Returns the path of the written file.
    fn write_skill(&self, root: &Path, skill: &Skill) -> anyhow::Result<PathBuf>;
}

pub trait AgentsWriter {
    fn name(&self) -> AgentName;

    /// Write a single agent to `<root>/<agent-dir>/agents/<agent.name>.agent.md`.
    /// Returns the path of the written file.
    fn write_agent(&self, root: &Path, agent: &Agent) -> anyhow::Result<PathBuf>;
}

// ── Frontmatter formatting ────────────────────────────────────────

/// Standard frontmatter serialisation shared by all agents.
/// Produces YAML with `name`, `description`, `allowed-tools` (if non-empty),
/// and any extra keys. Returns empty string if the frontmatter is entirely empty.
pub fn format_standard_frontmatter(fm: &FrontMatter, writer: &dyn FrontMatterWriter) -> String {
    let mut map: BTreeMap<String, serde_yml::Value> = BTreeMap::new();

    if let Some(name) = &fm.name {
        map.insert("name".into(), serde_yml::Value::String(name.clone()));
    }
    if let Some(desc) = &fm.description {
        map.insert("description".into(), serde_yml::Value::String(desc.clone()));
    }

    let tokens: Vec<String> = fm
        .allowed_tools
        .iter()
        .filter_map(|p| writer.format_permission(p))
        .collect();
    if !tokens.is_empty() {
        map.insert(
            "allowed-tools".into(),
            serde_yml::Value::String(tokens.join(" ")),
        );
    }

    for (k, v) in &fm.extra {
        map.insert(k.clone(), v.clone());
    }

    if map.is_empty() {
        return String::new();
    }

    serde_yml::to_string(&map).unwrap_or_default()
}

// ── Standard helpers (parallel to parse.rs) ───────────────────────

/// Write a skill as `<dir>/<name>/SKILL.md` using the given formatter.
pub fn write_standard_skill(
    dir: &Path,
    skill: &Skill,
    writer: &dyn FrontMatterWriter,
) -> anyhow::Result<PathBuf> {
    let skill_dir = dir.join(&skill.name);
    fs::create_dir_all(&skill_dir)?;
    let file = skill_dir.join("SKILL.md");
    let yaml = writer.format_frontmatter(&skill.front_matter);
    let content = if yaml.is_empty() {
        skill.body.clone()
    } else {
        join_frontmatter(&yaml, &skill.body)
    };
    fs::write(&file, &content)?;
    Ok(file)
}

/// Write an agent as `<dir>/<name>.agent.md` using the given formatter.
pub fn write_standard_agent(
    dir: &Path,
    agent: &Agent,
    writer: &dyn FrontMatterWriter,
) -> anyhow::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let file = dir.join(format!("{}.agent.md", agent.name));
    let yaml = writer.format_frontmatter(&agent.front_matter);
    let content = if yaml.is_empty() {
        agent.body.clone()
    } else {
        join_frontmatter(&yaml, &agent.body)
    };
    fs::write(&file, &content)?;
    Ok(file)
}
