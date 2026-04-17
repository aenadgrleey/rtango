use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::agent::{self, frontmatter::join_frontmatter};
use crate::spec::{AgentName, Deployment, Lock, OnTargetModified, Ownership, RuleKind, Spec};

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

    // Phase 1: expand every rule once and collect, for each absolute path that
    // would be touched (source file of an expanded item, or target path of a
    // rendered item), the set of rules that claim it.
    let expansions: Vec<Vec<ExpandedItem>> = spec
        .rules
        .iter()
        .map(|r| expand_rule(root, r))
        .collect::<anyhow::Result<_>>()?;

    let mut candidates: HashMap<PathBuf, HashSet<String>> = HashMap::new();
    for (rule, expanded) in spec.rules.iter().zip(expansions.iter()) {
        for item in expanded {
            let source_file = match &item.kind {
                ExpandedKind::Skill(s) => s.file.clone(),
                ExpandedKind::Agent(a) => a.file.clone(),
            };
            candidates.entry(source_file).or_default().insert(rule.id.clone());
            for target_agent in &spec.agents {
                let tp = root.join(target_path_for(target_agent, &item.kind)?);
                candidates.entry(tp).or_default().insert(rule.id.clone());
            }
        }
    }

    // Phase 2: resolve ownership for every touched path.
    let owners = resolve_owners(spec, &candidates, lock)?;

    // Keep only contested resolutions in the lock (uncontested ones are
    // derivable from the spec).
    let contested_owners: Vec<Ownership> = candidates
        .iter()
        .filter(|(_, cs)| cs.len() > 1)
        .filter_map(|(path, _)| {
            owners.get(path).map(|rule_id| Ownership {
                path: path.clone(),
                rule_id: rule_id.clone(),
            })
        })
        .collect();

    // Track which (rule_id, agent, target_path) combos we produce, for orphan detection.
    let mut seen: HashSet<(String, String, PathBuf)> = HashSet::new();
    // Track every absolute target path produced, to distinguish real orphans
    // (lock entries for paths nobody writes) from reparented paths (now owned
    // by a different rule and still live).
    let mut produced: HashSet<PathBuf> = HashSet::new();

    for (rule, expanded) in spec.rules.iter().zip(expansions.iter()) {
        let policy = effective_policy(rule.on_target_modified, default_policy);

        for exp_item in expanded {
            let source_file = match &exp_item.kind {
                ExpandedKind::Skill(s) => &s.file,
                ExpandedKind::Agent(a) => &a.file,
            };
            // Only process items whose source path this rule owns.
            if owners.get(source_file).map(String::as_str) != Some(rule.id.as_str()) {
                continue;
            }

            for target_agent in &spec.agents {
                let rendered = render_for_agent(root, exp_item, &rule.schema_agent, target_agent)?;
                let abs_target = root.join(&rendered.target_path);

                // Skip self-writes: writer round-tripping its own source file.
                if source_file == &abs_target {
                    continue;
                }
                // Only render targets this rule owns.
                if owners.get(&abs_target).map(String::as_str) != Some(rule.id.as_str()) {
                    continue;
                }

                seen.insert((
                    rendered.rule_id.clone(),
                    rendered.agent.0.clone(),
                    rendered.target_path.clone(),
                ));
                produced.insert(abs_target.clone());

                let lock_entry = find_lock_entry(lock, &rendered.rule_id, &rendered.agent, &rendered.target_path);
                let disk_content = fs::read_to_string(&abs_target).ok();
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

    // Orphan detection. A lock entry is truly orphaned only if the current
    // spec no longer produces it AND no other rule now owns the path. The
    // second check keeps us from deleting a file that was simply reparented
    // to a different rule (e.g. a set rule's old entry for a file that a
    // single-file rule now owns).
    for dep in &lock.deployments {
        let key = (dep.rule_id.clone(), dep.agent.0.clone(), dep.content.clone());
        if seen.contains(&key) {
            continue;
        }
        let abs = root.join(&dep.content);
        if produced.contains(&abs) || owners.contains_key(&abs) {
            continue;
        }
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

    Ok(Plan {
        items,
        owners: contested_owners,
    })
}

/// Pick one rule to own each path. Single claimant → auto. Multiple claimants
/// → use lock-recorded decision if valid; otherwise heuristic (single-file
/// rules beat set rules). Set-vs-set with no lock entry errors out so the
/// user can record an explicit decision.
fn resolve_owners(
    spec: &Spec,
    candidates: &HashMap<PathBuf, HashSet<String>>,
    lock: &Lock,
) -> anyhow::Result<HashMap<PathBuf, String>> {
    let rule_kinds: HashMap<&str, &RuleKind> = spec
        .rules
        .iter()
        .map(|r| (r.id.as_str(), &r.kind))
        .collect();
    let lock_owners: HashMap<&Path, &str> = lock
        .owners
        .iter()
        .map(|o| (o.path.as_path(), o.rule_id.as_str()))
        .collect();

    let mut resolved: HashMap<PathBuf, String> = HashMap::new();
    for (path, claimants) in candidates {
        if claimants.len() == 1 {
            let rule_id = claimants.iter().next().unwrap().clone();
            resolved.insert(path.clone(), rule_id);
            continue;
        }

        // Multiple claimants. First, honor any user decision from the lock.
        if let Some(&locked) = lock_owners.get(path.as_path()) {
            if claimants.contains(locked) {
                resolved.insert(path.clone(), locked.to_string());
                continue;
            }
            // Lock points at a rule that no longer claims this path; fall
            // through to heuristic.
        }

        // Heuristic: a single-file rule is strictly more specific than a set.
        let singles: Vec<&String> = claimants
            .iter()
            .filter(|r| matches!(
                rule_kinds.get(r.as_str()),
                Some(RuleKind::Skill {}) | Some(RuleKind::Agent {})
            ))
            .collect();
        match singles.len() {
            0 => {}
            1 => {
                resolved.insert(path.clone(), singles[0].clone());
                continue;
            }
            _ => {
                let mut names: Vec<&str> = singles.iter().map(|s| s.as_str()).collect();
                names.sort();
                anyhow::bail!(
                    "ambiguous ownership for {}: single-file rules {:?} both claim this path",
                    path.display(),
                    names
                );
            }
        }

        let mut names: Vec<&str> = claimants.iter().map(String::as_str).collect();
        names.sort();
        anyhow::bail!(
            "ambiguous ownership for {}: rules {:?} all claim this path. \
             Record a decision in .rtango/lock.yaml under `owners:` or narrow the spec.",
            path.display(),
            names
        );
    }
    Ok(resolved)
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
