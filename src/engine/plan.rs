use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::agent::{self, frontmatter::join_frontmatter};
use crate::spec::{AgentName, Deployment, Lock, OnTargetModified, Spec};

use super::{
    DeploymentStatus, ExpandedItem, ExpandedKind, Plan, PlannedDeployment,
    RenderedTarget, effective_policy, expand_rule, hash_content,
};

/// Compute the target path for a rendered item based on the target agent.
fn target_path_for(agent: &AgentName, kind: &ExpandedKind) -> anyhow::Result<PathBuf> {
    let dir = match agent.as_str() {
        "copilot" => ".github",
        "claude-code" => ".claude",
        "codex" => ".codex",
        "pi" => ".pi",
        "opencode" => ".opencode",
        other => anyhow::bail!("unknown target agent: {}", other),
    };
    match kind {
        ExpandedKind::Skill(s) => Ok(PathBuf::from(format!("{dir}/skills/{}/SKILL.md", s.name))),
        ExpandedKind::Agent(a) => Ok(PathBuf::from(format!("{dir}/agents/{}.agent.md", a.name))),
    }
}

/// Render an expanded item for a specific target agent.
pub fn render_for_agent(
    _root: &Path,
    item: &ExpandedItem,
    _schema_agent: &AgentName,
    target_agent: &AgentName,
) -> anyhow::Result<RenderedTarget> {
    let writer = agent::frontmatter_writer(target_agent)
        .ok_or_else(|| anyhow::anyhow!("unknown target agent: {}", target_agent))?;

    let (fm, body) = match &item.kind {
        ExpandedKind::Skill(s) => (&s.front_matter, &s.body),
        ExpandedKind::Agent(a) => (&a.front_matter, &a.body),
    };

    let yaml = writer.format_frontmatter(fm);
    let content = if yaml.is_empty() {
        body.clone()
    } else {
        join_frontmatter(&yaml, body)
    };

    let target_path = target_path_for(target_agent, &item.kind)?;
    let content_hash = hash_content(&content);

    Ok(RenderedTarget {
        rule_id: item.rule_id.clone(),
        agent: target_agent.clone(),
        source: item.source.clone(),
        source_hash: item.source_hash.clone(),
        target_path,
        content,
        content_hash,
    })
}

/// Find a matching lock deployment by rule_id, agent, and target_path.
fn find_lock_entry<'a>(
    lock: &'a Lock,
    rule_id: &str,
    agent: &AgentName,
    target_path: &Path,
) -> Option<&'a Deployment> {
    lock.deployments.iter().find(|d| {
        d.rule_id == rule_id && d.agent == *agent && d.content == target_path
    })
}

/// Compute the full sync plan.
pub fn compute_plan(
    root: &Path,
    spec: &Spec,
    lock: &Lock,
    force: bool,
) -> anyhow::Result<Plan> {
    let default_policy = spec.defaults.on_target_modified;
    let mut items = Vec::new();

    // Track which (rule_id, agent, target_path) combos we produce, for orphan detection
    let mut seen: HashSet<(String, String, PathBuf)> = HashSet::new();

    for rule in &spec.rules {
        let expanded = expand_rule(root, rule)?;
        let policy = effective_policy(rule.on_target_modified, default_policy);

        for exp_item in &expanded {
            for target_agent in &spec.agents {
                let rendered = render_for_agent(root, exp_item, &rule.schema_agent, target_agent)?;
                seen.insert((
                    rendered.rule_id.clone(),
                    rendered.agent.0.clone(),
                    rendered.target_path.clone(),
                ));

                let lock_entry = find_lock_entry(lock, &rendered.rule_id, &rendered.agent, &rendered.target_path);
                let disk_path = root.join(&rendered.target_path);
                let disk_content = fs::read_to_string(&disk_path).ok();
                let disk_hash = disk_content.as_deref().map(hash_content);

                let status = compute_status(
                    lock_entry,
                    &rendered,
                    disk_hash.as_deref(),
                    disk_content.is_some(),
                    policy,
                    force,
                );

                items.push(PlannedDeployment {
                    rule_id: rendered.rule_id,
                    agent: rendered.agent,
                    source: rendered.source,
                    source_hash: rendered.source_hash,
                    target_path: rendered.target_path,
                    rendered_content: rendered.content,
                    status,
                });
            }
        }
    }

    // Orphan detection: lock entries not present in seen
    for dep in &lock.deployments {
        let key = (dep.rule_id.clone(), dep.agent.0.clone(), dep.content.clone());
        if !seen.contains(&key) {
            items.push(PlannedDeployment {
                rule_id: dep.rule_id.clone(),
                agent: dep.agent.clone(),
                source: dep.source.clone(),
                source_hash: dep.source_hash.clone(),
                target_path: dep.content.clone(),
                rendered_content: String::new(),
                status: DeploymentStatus::Orphan,
            });
        }
    }

    Ok(Plan { items })
}

fn compute_status(
    lock_entry: Option<&Deployment>,
    rendered: &RenderedTarget,
    disk_hash: Option<&str>,
    disk_exists: bool,
    policy: OnTargetModified,
    force: bool,
) -> DeploymentStatus {
    match lock_entry {
        None => {
            // No lock entry
            if !disk_exists {
                DeploymentStatus::Create
            } else if force {
                DeploymentStatus::Update
            } else {
                DeploymentStatus::Conflict {
                    reason: "target file exists but is not tracked in lock".into(),
                }
            }
        }
        Some(dep) => {
            if dep.source_hash == rendered.source_hash {
                // Source unchanged — check if target was modified externally
                match disk_hash {
                    Some(dh) if dh == dep.content_hash => DeploymentStatus::UpToDate,
                    Some(_) => {
                        // Target was modified externally
                        apply_policy(policy, force, "target was modified externally")
                    }
                    None => {
                        // Target file was deleted
                        DeploymentStatus::Create
                    }
                }
            } else {
                // Source changed
                let target_modified = match disk_hash {
                    Some(dh) => dh != dep.content_hash,
                    None => false, // file deleted, no conflict
                };
                if target_modified {
                    // Both source and target changed
                    if force {
                        DeploymentStatus::Update
                    } else {
                        apply_policy(policy, false, "both source and target were modified")
                    }
                } else {
                    DeploymentStatus::Update
                }
            }
        }
    }
}

fn apply_policy(policy: OnTargetModified, force: bool, reason: &str) -> DeploymentStatus {
    if force {
        return DeploymentStatus::Update;
    }
    match policy {
        OnTargetModified::Fail => DeploymentStatus::Conflict {
            reason: reason.to_string(),
        },
        OnTargetModified::Overwrite => DeploymentStatus::Update,
        OnTargetModified::Skip => DeploymentStatus::UpToDate,
    }
}
