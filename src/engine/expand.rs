use std::fs;
use std::path::{Path, PathBuf};

use crate::agent::{self, frontmatter::{FrontMatter, FrontMatterMapper, split_frontmatter, tokenize_tools}};
use crate::spec::{Rule, RuleKind, Source};

use super::{ExpandedItem, ExpandedKind, fetch_github, hash_content};

/// Expand a single rule into its constituent items by reading source files.
///
/// - `Skill` / `Agent`: produces one item (single file).
/// - `SkillSet` / `AgentSet`: produces N items (one per source file in the directory).
pub fn expand_rule(root: &Path, rule: &Rule) -> anyhow::Result<Vec<ExpandedItem>> {
    let (project_root, abs_path) = materialize(root, &rule.source)?;

    match &rule.kind {
        RuleKind::SkillSet { include, exclude } => {
            expand_skill_set(&project_root, rule, &abs_path, include, exclude)
        }
        RuleKind::AgentSet { include, exclude } => {
            expand_agent_set(&project_root, rule, &abs_path, include, exclude)
        }
        RuleKind::Skill { name, description, allowed_tools } => {
            expand_single_skill(rule, &abs_path, name.as_deref(), description.as_deref(), allowed_tools.as_deref())
        }
        RuleKind::Agent { name, description, allowed_tools } => {
            expand_single_agent(rule, &abs_path, name.as_deref(), description.as_deref(), allowed_tools.as_deref())
        }
    }
}

/// Decide whether an entry named `name` passes the include/exclude filter.
/// Include wins if set (whitelist); otherwise exclude drops matches.
/// Include/exclude are mutually exclusive — validated at the CLI layer.
fn passes_filter(name: &str, include: &[String], exclude: &[String]) -> bool {
    if !include.is_empty() {
        return include.iter().any(|p| p == name);
    }
    !exclude.iter().any(|p| p == name)
}

/// Apply CLI-level overrides from the rule to a parsed `FrontMatter`.
fn apply_overrides(
    fm: &mut FrontMatter,
    mapper: &dyn FrontMatterMapper,
    name: Option<&str>,
    description: Option<&str>,
    allowed_tools: Option<&str>,
) {
    if let Some(n) = name {
        fm.name = Some(n.to_string());
    }
    if let Some(d) = description {
        fm.description = Some(d.to_string());
    }
    if let Some(t) = allowed_tools {
        fm.allowed_tools = tokenize_tools(t)
            .into_iter()
            .map(|tok| mapper.parse_permission(&tok))
            .collect();
    }
}

/// Resolve a source to (project_root, filter_path) on disk.
///
/// `project_root` is what gets handed to agent parsers (which internally append
/// `.claude/skills`, `.agent/skills`, etc.). `filter_path` is what the
/// `expand_*_set` helpers use to narrow the parser's results to a subtree.
fn materialize(root: &Path, source: &Source) -> anyhow::Result<(PathBuf, PathBuf)> {
    match source {
        Source::Local(rel) => Ok((root.to_path_buf(), root.join(rel))),
        Source::Github(g) => {
            let cache_root = fetch_github(g)?;
            let filter = if g.path.is_empty() {
                cache_root.clone()
            } else {
                cache_root.join(&g.path)
            };
            Ok((cache_root, filter))
        }
    }
}

fn expand_skill_set(
    root: &Path,
    rule: &Rule,
    abs_path: &Path,
    include: &[String],
    exclude: &[String],
) -> anyhow::Result<Vec<ExpandedItem>> {
    let parser = agent::skills_parser(&rule.schema_agent)
        .ok_or_else(|| anyhow::anyhow!("unknown agent: {}", rule.schema_agent))?;
    let skills = parser.parse_skills(root)?;
    let mut items = Vec::new();
    for skill in &skills {
        if !skill.dir.starts_with(abs_path) {
            continue;
        }
        if !passes_filter(&skill.name, include, exclude) {
            continue;
        }
        let content = fs::read_to_string(&skill.file)?;
        let hash = hash_content(&content);
        items.push(ExpandedItem {
            rule_id: rule.id.clone(),
            source: rule.source.clone(),
            source_content: content,
            source_hash: hash,
            kind: ExpandedKind::Skill(skill.clone()),
        });
    }
    Ok(items)
}

fn expand_agent_set(
    root: &Path,
    rule: &Rule,
    abs_path: &Path,
    include: &[String],
    exclude: &[String],
) -> anyhow::Result<Vec<ExpandedItem>> {
    let parser = agent::agents_parser(&rule.schema_agent)
        .ok_or_else(|| anyhow::anyhow!("unknown agent: {}", rule.schema_agent))?;
    let agents = parser.parse_agents(root)?;
    let mut items = Vec::new();
    for ag in &agents {
        if !ag.file.starts_with(abs_path) {
            continue;
        }
        if !passes_filter(&ag.name, include, exclude) {
            continue;
        }
        let content = fs::read_to_string(&ag.file)?;
        let hash = hash_content(&content);
        items.push(ExpandedItem {
            rule_id: rule.id.clone(),
            source: rule.source.clone(),
            source_content: content,
            source_hash: hash,
            kind: ExpandedKind::Agent(ag.clone()),
        });
    }
    Ok(items)
}

fn expand_single_skill(
    rule: &Rule,
    abs_path: &Path,
    override_name: Option<&str>,
    override_description: Option<&str>,
    override_allowed_tools: Option<&str>,
) -> anyhow::Result<Vec<ExpandedItem>> {
    let mapper = agent::frontmatter_mapper(&rule.schema_agent)
        .ok_or_else(|| anyhow::anyhow!("unknown agent: {}", rule.schema_agent))?;

    let skill_file = abs_path.join("SKILL.md");
    if !skill_file.is_file() {
        anyhow::bail!("skill file not found: {}", skill_file.display());
    }
    let name = abs_path.file_name()
        .ok_or_else(|| anyhow::anyhow!("skill path has no name"))?
        .to_string_lossy()
        .into_owned();
    let content = fs::read_to_string(&skill_file)?;
    let (yaml, body) = split_frontmatter(&content);
    let mut front_matter = match yaml {
        Some(y) => mapper.parse_frontmatter(y)?,
        None => FrontMatter::default(),
    };
    apply_overrides(
        &mut front_matter,
        mapper.as_ref(),
        override_name,
        override_description,
        override_allowed_tools,
    );
    let hash = hash_content(&content);
    let skill = crate::agent::Skill {
        name,
        dir: abs_path.to_path_buf(),
        file: skill_file,
        front_matter,
        body: body.to_string(),
    };
    Ok(vec![ExpandedItem {
        rule_id: rule.id.clone(),
        source: rule.source.clone(),
        source_content: content,
        source_hash: hash,
        kind: ExpandedKind::Skill(skill),
    }])
}

fn expand_single_agent(
    rule: &Rule,
    abs_path: &Path,
    override_name: Option<&str>,
    override_description: Option<&str>,
    override_allowed_tools: Option<&str>,
) -> anyhow::Result<Vec<ExpandedItem>> {
    let mapper = agent::frontmatter_mapper(&rule.schema_agent)
        .ok_or_else(|| anyhow::anyhow!("unknown agent: {}", rule.schema_agent))?;

    if !abs_path.is_file() {
        anyhow::bail!("agent file not found: {}", abs_path.display());
    }
    let file_name = abs_path.file_name()
        .ok_or_else(|| anyhow::anyhow!("agent path has no file name"))?
        .to_string_lossy();
    let agent_name = file_name.strip_suffix(".agent.md")
        .ok_or_else(|| anyhow::anyhow!("agent file must end with .agent.md: {}", file_name))?
        .to_owned();
    let content = fs::read_to_string(abs_path)?;
    let (yaml, body) = split_frontmatter(&content);
    let mut front_matter = match yaml {
        Some(y) => mapper.parse_frontmatter(y)?,
        None => FrontMatter::default(),
    };
    apply_overrides(
        &mut front_matter,
        mapper.as_ref(),
        override_name,
        override_description,
        override_allowed_tools,
    );
    let hash = hash_content(&content);
    let agent = crate::agent::Agent {
        name: agent_name,
        file: abs_path.to_path_buf(),
        front_matter,
        body: body.to_string(),
    };
    Ok(vec![ExpandedItem {
        rule_id: rule.id.clone(),
        source: rule.source.clone(),
        source_content: content,
        source_hash: hash,
        kind: ExpandedKind::Agent(agent),
    }])
}
