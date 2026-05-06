use std::fs;
use std::path::{Path, PathBuf};

use crate::agent::{
    self,
    frontmatter::{FrontMatter, FrontMatterMapper, split_frontmatter, tokenize_tools},
};
use crate::spec::{Rule, RuleKind, Source};

use super::{
    ExpandedItem, ExpandedKind, SystemFile, fetch_github, hash_content, read_collection_spec,
};

/// Expand a single rule into its constituent items by reading source files.
///
/// - `Skill` / `Agent`: produces one item (single file).
/// - `SkillSet` / `AgentSet`: produces N items (one per source file in the directory).
pub fn expand_rule(root: &Path, rule: &Rule) -> anyhow::Result<Vec<ExpandedItem>> {
    match &rule.kind {
        RuleKind::Collection {
            include,
            exclude,
            schema_override,
        } => expand_collection(root, rule, include, exclude, schema_override),
        _ => expand_local_or_github(root, rule),
    }
}

/// Handle the non-collection kinds (skill, agent, skill-set, agent-set, system).
fn expand_local_or_github(root: &Path, rule: &Rule) -> anyhow::Result<Vec<ExpandedItem>> {
    let (project_root, abs_path) = materialize(root, &rule.source)?;

    match &rule.kind {
        RuleKind::SkillSet { include, exclude } => {
            expand_skill_set(&project_root, rule, &abs_path, include, exclude)
        }
        RuleKind::AgentSet { include, exclude } => {
            expand_agent_set(&project_root, rule, &abs_path, include, exclude)
        }
        RuleKind::Skill {
            name,
            description,
            allowed_tools,
        } => expand_single_skill(
            rule,
            &abs_path,
            name.as_deref(),
            description.as_deref(),
            allowed_tools.as_deref(),
        ),
        RuleKind::Agent {
            name,
            description,
            allowed_tools,
        } => expand_single_agent(
            rule,
            &abs_path,
            name.as_deref(),
            description.as_deref(),
            allowed_tools.as_deref(),
        ),
        RuleKind::System => expand_single_system(rule, &abs_path),
        RuleKind::Collection { .. } => unreachable!("handled in expand_rule"),
    }
}

/// Expand a Collection rule: fetch the remote repo, parse its spec.yaml,
/// and expand each matching rule. Imported rules get their id prefixed with
/// `<collection_rule_id>/` to avoid collisions with local rules.
fn expand_collection(
    root: &Path,
    rule: &Rule,
    include: &[String],
    exclude: &[String],
    schema_override: &Option<crate::spec::AgentName>,
) -> anyhow::Result<Vec<ExpandedItem>> {
    // Resolve the source to an on-disk root, exactly as non-collection rules do.
    let collection_root = match &rule.source {
        Source::Local(rel) => {
            let abs = if rel.is_absolute() {
                rel.clone()
            } else {
                root.join(rel)
            };
            if !abs.is_dir() {
                anyhow::bail!(
                    "collection '{}': source directory not found: {}",
                    rule.id,
                    abs.display()
                );
            }
            abs
        }
        Source::Github(g) => fetch_github(g)?,
    };
    let remote_spec = read_collection_spec(&collection_root)?;

    let mut all_items = Vec::new();
    for remote_rule in &remote_spec.rules {
        if !collection_passes_filter(&remote_rule.id, include, exclude) {
            continue;
        }
        // Build a synthetic rule that uses the remote rule's definition,
        // but with an optional schema_agent override from the local collection rule.
        let effective_schema = schema_override
            .clone()
            .unwrap_or_else(|| remote_rule.schema_agent.clone());

        let synthetic = Rule {
            id: format!("{}/{}", rule.id, remote_rule.id),
            source: remote_rule.source.clone(),
            schema_agent: effective_schema,
            on_target_modified: rule.on_target_modified,
            kind: remote_rule.kind.clone(),
        };

        // Expand the synthetic rule using the cache root as the project root
        let items = expand_local_or_github(&collection_root, &synthetic)?;

        // Re-tag the items so they carry the collection's source for lock tracking
        for mut item in items {
            item.rule_id = synthetic.id.clone();
            all_items.push(item);
        }
    }
    Ok(all_items)
}

/// Check if a remote rule id passes the collection's include/exclude filter.
fn collection_passes_filter(rule_id: &str, include: &[String], exclude: &[String]) -> bool {
    if !include.is_empty() {
        return include.iter().any(|p| p == rule_id);
    }
    !exclude.iter().any(|p| p == rule_id)
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
        } // Collection rules are dispatched before reaching materialize.
          // The Source variants (Local/Github) are handled above for all other kinds.
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
    let name = abs_path
        .file_name()
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

fn expand_single_system(rule: &Rule, abs_path: &Path) -> anyhow::Result<Vec<ExpandedItem>> {
    if !abs_path.is_file() {
        anyhow::bail!("system file not found: {}", abs_path.display());
    }
    let content = fs::read_to_string(abs_path)?;
    let hash = hash_content(&content);
    let system = SystemFile {
        file: abs_path.to_path_buf(),
        body: content.clone(),
    };
    Ok(vec![ExpandedItem {
        rule_id: rule.id.clone(),
        source: rule.source.clone(),
        source_content: content,
        source_hash: hash,
        kind: ExpandedKind::System(system),
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
    let file_name = abs_path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("agent path has no file name"))?
        .to_string_lossy();
    let agent_name = file_name
        .strip_suffix(".agent.md")
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
