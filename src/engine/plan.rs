use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::agent::{self, frontmatter::join_frontmatter};
use crate::spec::{
    AgentName, Deployment, Lock, OnTargetModified, Ownership, Rule, RuleKind, Source, Spec, Target,
};

use super::fetch::{GithubFetchError, describe_github_source};
use super::{
    AmbiguityReport, DeploymentStatus, ExpandedItem, ExpandedKind, Plan, PlanReport,
    PlannedDeployment, RenderedTarget, SkippedGithubFetch, builtin, effective_policy, expand_rule,
    hash_content,
};

/// User-level directories used by global agent registries.
///
/// Keeping these paths in a value object makes global planning independent of
/// the process environment. The CLI constructs it from the real environment;
/// callers embedding rtango (and tests) can inject a different profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalEnvironment {
    pub home: PathBuf,
    pub codex_home: PathBuf,
    pub copilot_home: PathBuf,
    pub xdg_config_home: PathBuf,
}

impl GlobalEnvironment {
    pub fn from_process() -> anyhow::Result<Self> {
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("could not determine home directory"))?;
        Ok(Self {
            codex_home: std::env::var_os("CODEX_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".codex")),
            copilot_home: std::env::var_os("COPILOT_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".copilot")),
            xdg_config_home: std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".config")),
            home,
        })
    }

    /// Construct a hermetic profile rooted at `home`, with no process-env
    /// lookups. This is useful for isolated Codex instances and tests.
    pub fn for_home(home: &Path) -> Self {
        Self {
            codex_home: home.join(".codex"),
            copilot_home: home.join(".copilot"),
            xdg_config_home: home.join(".config"),
            home: home.to_path_buf(),
        }
    }
}

/// `Project` preserves rtango's existing repository layout. `Global` uses
/// each agent's documented user-level configuration directory and is used by
/// the standalone `global-sync` command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetScope {
    Project,
    Global(GlobalEnvironment),
}

/// Compute the target path for a rendered item based on the target agent.
fn target_path_for(
    root: &Path,
    target: &Target,
    kind: &ExpandedKind,
    scope: &TargetScope,
) -> anyhow::Result<PathBuf> {
    let agent = &target.agent;
    if let ExpandedKind::System(_) = kind {
        return system_file_path_for(root, target, scope);
    }
    if let TargetScope::Global(environment) = scope {
        let base = if agent.as_str() == "codex"
            && matches!(kind, ExpandedKind::Skill(_) | ExpandedKind::SkillAsset(_))
        {
            global_shared_skills_root(environment, target.home.as_deref())
        } else {
            global_agent_root(root, agent, environment, target.home.as_deref())?
        };
        return match kind {
            ExpandedKind::Skill(skill) => {
                Ok(base.join("skills").join(&skill.name).join("SKILL.md"))
            }
            ExpandedKind::SkillAsset(asset) => Ok(base
                .join("skills")
                .join(&asset.skill_name)
                .join(&asset.relative_path)),
            ExpandedKind::Agent(agent_file) => {
                let suffix = if matches!(agent.as_str(), "pi" | "cursor") {
                    format!("{}.md", agent_file.name)
                } else {
                    format!("{}.agent.md", agent_file.name)
                };
                Ok(base.join("agents").join(suffix))
            }
            ExpandedKind::System(_) => unreachable!("handled above"),
        };
    }
    let dir = match agent.as_str() {
        "copilot" => ".github",
        "cursor" => ".cursor",
        "claude-code" => ".claude",
        "codex" => ".codex",
        "pi" => ".pi",
        "opencode" => ".opencode",
        "plain" => "",
        other => anyhow::bail!("unknown target agent: {}", other),
    };
    let prefix = if dir.is_empty() {
        String::new()
    } else {
        format!("{dir}/")
    };
    match kind {
        ExpandedKind::Skill(s) => Ok(PathBuf::from(format!("{prefix}skills/{}/SKILL.md", s.name))),
        ExpandedKind::SkillAsset(asset) => {
            let mut path = if prefix.is_empty() {
                PathBuf::new()
            } else {
                PathBuf::from(&prefix)
            };
            path.push("skills");
            path.push(&asset.skill_name);
            path.push(&asset.relative_path);
            Ok(path)
        }
        ExpandedKind::Agent(a) => {
            let file_name = if matches!(agent.as_str(), "pi" | "cursor") {
                format!("{}.md", a.name)
            } else {
                format!("{}.agent.md", a.name)
            };
            Ok(PathBuf::from(format!("{prefix}agents/{file_name}")))
        }
        ExpandedKind::System(_) => unreachable!("handled above"),
    }
}

/// Convention path for the per-agent root instruction file.
fn system_file_path_for(
    root: &Path,
    target: &Target,
    scope: &TargetScope,
) -> anyhow::Result<PathBuf> {
    let agent = &target.agent;
    match scope {
        TargetScope::Project => match agent.as_str() {
            "copilot" => Ok(PathBuf::from(".github/copilot-instructions.md")),
            "cursor" => Ok(PathBuf::from("AGENTS.md")),
            "claude-code" => Ok(PathBuf::from("CLAUDE.md")),
            "codex" | "pi" | "opencode" => Ok(PathBuf::from("AGENTS.md")),
            "plain" => Ok(PathBuf::from("system/AGENTS.md")),
            other => anyhow::bail!("unknown target agent: {other}"),
        },
        TargetScope::Global(environment) => match agent.as_str() {
            "copilot" => Ok(
                global_agent_root(root, agent, environment, target.home.as_deref())?
                    .join("copilot-instructions.md"),
            ),
            "claude-code" => {
                Ok(
                    global_agent_root(root, agent, environment, target.home.as_deref())?
                        .join("CLAUDE.md"),
                )
            }
            "codex" => {
                let base = global_agent_root(root, agent, environment, target.home.as_deref())?;
                let override_file = base.join("AGENTS.override.md");
                if override_file.is_file() {
                    Ok(override_file)
                } else {
                    Ok(base.join("AGENTS.md"))
                }
            }
            "pi" | "opencode" => {
                Ok(
                    global_agent_root(root, agent, environment, target.home.as_deref())?
                        .join("AGENTS.md"),
                )
            }
            "cursor" => anyhow::bail!(
                "global system instructions are not file-backed for cursor; use Cursor User Rules in settings"
            ),
            "plain" => Ok(PathBuf::from(".rtango/AGENTS.md")),
            other => anyhow::bail!("unknown target agent: {other}"),
        },
    }
}

fn global_agent_root(
    root: &Path,
    agent: &AgentName,
    environment: &GlobalEnvironment,
    custom_home: Option<&Path>,
) -> anyhow::Result<PathBuf> {
    if let Some(home) = custom_home {
        return Ok(match agent.as_str() {
            "codex" | "copilot" => home.to_path_buf(),
            "opencode" => home.join("opencode"),
            "cursor" => home.join(".cursor"),
            "claude-code" => home.join(".claude"),
            "pi" => home.join(".pi/agent"),
            "plain" => home.join(".rtango"),
            other => anyhow::bail!("unknown target agent: {other}"),
        });
    }
    match agent.as_str() {
        "codex" => Ok(environment.codex_home.clone()),
        "copilot" => Ok(environment.copilot_home.clone()),
        "opencode" => Ok(environment.xdg_config_home.join("opencode")),
        "cursor" => Ok(root.join(".cursor")),
        "claude-code" => Ok(root.join(".claude")),
        "pi" => Ok(root.join(".pi/agent")),
        "plain" => Ok(root.join(".rtango")),
        other => anyhow::bail!("unknown target agent: {other}"),
    }
}

fn global_shared_skills_root(
    environment: &GlobalEnvironment,
    custom_home: Option<&Path>,
) -> PathBuf {
    custom_home
        .map(Path::to_path_buf)
        .unwrap_or_else(|| environment.home.join(".agents"))
}

/// Resolve a plan path, which is relative for project targets and absolute
/// for global targets redirected outside `$HOME` by an agent environment
/// variable.
pub fn resolve_target_path(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

/// Render an expanded item for a specific target agent.
pub fn render_for_agent(
    root: &Path,
    item: &ExpandedItem,
    schema_agent: &AgentName,
    target_agent: &AgentName,
) -> anyhow::Result<RenderedTarget> {
    render_for_agent_in_scope(
        root,
        item,
        schema_agent,
        &Target::agent(target_agent.0.clone()),
        &TargetScope::Project,
    )
}

pub fn render_for_agent_in_scope(
    _root: &Path,
    item: &ExpandedItem,
    _schema_agent: &AgentName,
    target: &Target,
    scope: &TargetScope,
) -> anyhow::Result<RenderedTarget> {
    let content = match &item.kind {
        ExpandedKind::System(s) => s.body.clone(),
        ExpandedKind::SkillAsset(asset) => asset.content.clone(),
        ExpandedKind::Skill(_) | ExpandedKind::Agent(_) => {
            let writer = agent::frontmatter_writer(&target.agent)
                .ok_or_else(|| anyhow::anyhow!("unknown target agent: {}", target.agent))?;
            let (fm, body) = match &item.kind {
                ExpandedKind::Skill(s) => (&s.front_matter, &s.body),
                ExpandedKind::Agent(a) => (&a.front_matter, &a.body),
                ExpandedKind::SkillAsset(_) | ExpandedKind::System(_) => {
                    unreachable!("handled above")
                }
            };
            let yaml = writer.format_frontmatter(fm);
            if yaml.is_empty() {
                body.clone()
            } else {
                join_frontmatter(&yaml, body)
            }
        }
    };

    let target_path = target_path_for(_root, target, &item.kind, scope)?;
    let content_hash = hash_content(&content);

    Ok(RenderedTarget {
        rule_id: item.rule_id.clone(),
        agent: target.agent.clone(),
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
    lock.deployments
        .iter()
        .find(|d| d.rule_id == rule_id && d.agent == *agent && d.content == target_path)
}

/// Find any lock deployment for (agent, target_path), regardless of rule_id.
/// Used to adopt an existing on-disk file when a path is reparented to a new
/// owning rule — we forward the prior content_hash so a matching disk file is
/// treated as already-in-sync instead of emitting a "not tracked in lock"
/// conflict.
fn find_reparent_candidate<'a>(
    lock: &'a Lock,
    agent: &AgentName,
    target_path: &Path,
) -> Option<&'a Deployment> {
    lock.deployments
        .iter()
        .find(|d| d.agent == *agent && d.content == target_path)
}

fn rule_targets(spec: &Spec, rule: &Rule) -> Vec<Target> {
    rule.targets.clone().unwrap_or_else(|| {
        spec.agents
            .iter()
            .cloned()
            .map(|agent| Target { agent, home: None })
            .collect()
    })
}

struct ExpandedRule {
    rule_index: usize,
    items: Vec<ExpandedItem>,
}

struct ExpansionOutcome {
    expanded_rules: Vec<ExpandedRule>,
    skipped_fetches: Vec<SkippedGithubFetch>,
}

/// Compute the full sync plan.
///
/// When `inject_builtins` is true, built-in skills (like the rtango usage
/// skill) are automatically added to the plan. They are written to disk but
/// not tracked in the lock.
pub fn compute_plan(
    root: &Path,
    spec: &Spec,
    lock: &Lock,
    force: bool,
    inject_builtins: bool,
) -> anyhow::Result<Plan> {
    Ok(compute_plan_with_fetch_failures(root, spec, lock, force, inject_builtins, false)?.plan)
}

/// Compute a plan with separate source and target roots.
///
/// This is the domain entry point for stateless/global projections: sources
/// are expanded relative to `source_root`, while rendered files are diffed
/// and written relative to `target_root`.
pub fn compute_plan_in_scope(
    target_root: &Path,
    source_root: &Path,
    spec: &Spec,
    lock: &Lock,
    force: bool,
    inject_builtins: bool,
    scope: TargetScope,
) -> anyhow::Result<Plan> {
    Ok(compute_plan_with_fetch_failures_in_scope(
        target_root,
        source_root,
        spec,
        lock,
        force,
        inject_builtins,
        false,
        scope,
    )?
    .plan)
}

pub fn compute_plan_with_fetch_failures(
    root: &Path,
    spec: &Spec,
    lock: &Lock,
    force: bool,
    inject_builtins: bool,
    ignore_fetch_failures: bool,
) -> anyhow::Result<PlanReport> {
    compute_plan_with_fetch_failures_in_scope(
        root,
        root,
        spec,
        lock,
        force,
        inject_builtins,
        ignore_fetch_failures,
        TargetScope::Project,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn compute_plan_with_fetch_failures_in_scope(
    target_root: &Path,
    source_root: &Path,
    spec: &Spec,
    lock: &Lock,
    force: bool,
    inject_builtins: bool,
    ignore_fetch_failures: bool,
    scope: TargetScope,
) -> anyhow::Result<PlanReport> {
    let expansion = expand_rules(source_root, spec, ignore_fetch_failures)?;
    let plan = compute_plan_from_expanded_rules(
        target_root,
        source_root,
        spec,
        lock,
        force,
        inject_builtins,
        &scope,
        &expansion.expanded_rules,
    )?;
    Ok(PlanReport {
        plan,
        skipped_fetches: expansion.skipped_fetches,
    })
}

#[allow(clippy::too_many_arguments)]
fn compute_plan_from_expanded_rules(
    target_root: &Path,
    source_root: &Path,
    spec: &Spec,
    lock: &Lock,
    force: bool,
    inject_builtins: bool,
    scope: &TargetScope,
    expanded_rules: &[ExpandedRule],
) -> anyhow::Result<Plan> {
    let default_policy = spec.defaults.on_target_modified;
    let mut items = Vec::new();
    let candidates =
        collect_candidates_from_expanded_rules(target_root, spec, scope, expanded_rules)?;

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

    for expanded_rule in expanded_rules {
        let rule = &spec.rules[expanded_rule.rule_index];
        let policy = effective_policy(rule.on_target_modified, default_policy);

        for exp_item in &expanded_rule.items {
            let source_file = match &exp_item.kind {
                ExpandedKind::Skill(s) => &s.file,
                ExpandedKind::SkillAsset(asset) => &asset.source_file,
                ExpandedKind::Agent(a) => &a.file,
                ExpandedKind::System(s) => &s.file,
            };
            if owners.get(source_file).map(String::as_str) != Some(rule.id.as_str()) {
                continue;
            }

            for target in rule_targets(spec, rule) {
                let rendered = render_for_agent_in_scope(
                    target_root,
                    exp_item,
                    &rule.schema_agent,
                    &target,
                    scope,
                )?;
                let abs_target = resolve_target_path(target_root, &rendered.target_path);

                if source_file == &abs_target {
                    continue;
                }
                if owners.get(&abs_target).map(String::as_str) != Some(rule.id.as_str()) {
                    continue;
                }

                if !seen.insert((
                    rendered.rule_id.clone(),
                    rendered.agent.0.clone(),
                    rendered.target_path.clone(),
                )) {
                    continue;
                }
                produced.insert(abs_target.clone());

                let disk_content = fs::read_to_string(&abs_target).ok();
                let disk_hash = disk_content.as_deref().map(hash_content);

                let direct = find_lock_entry(
                    lock,
                    &rendered.rule_id,
                    &rendered.agent,
                    &rendered.target_path,
                );
                let adopted: Option<Deployment> = match (direct, disk_hash.as_deref()) {
                    (None, Some(dh)) => {
                        find_reparent_candidate(lock, &rendered.agent, &rendered.target_path)
                            .filter(|cand| cand.content_hash == dh)
                            .map(|cand| Deployment {
                                rule_id: rendered.rule_id.clone(),
                                agent: cand.agent.clone(),
                                source: cand.source.clone(),
                                source_hash: cand.source_hash.clone(),
                                content: cand.content.clone(),
                                content_hash: cand.content_hash.clone(),
                            })
                    }
                    _ => None,
                };
                let lock_entry = direct.or(adopted.as_ref());

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

    for dep in &lock.deployments {
        let key = (
            dep.rule_id.clone(),
            dep.agent.0.clone(),
            dep.content.clone(),
        );
        if seen.contains(&key) {
            continue;
        }
        let abs = resolve_target_path(target_root, &dep.content);
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

    if inject_builtins && matches!(scope, TargetScope::Project) {
        let user_source_dirs: Vec<PathBuf> = spec
            .rules
            .iter()
            .filter_map(|r| match &r.source {
                Source::Local(p) => Some(source_root.join(p)),
                _ => None,
            })
            .filter(|p| p.is_dir())
            .collect();

        for rendered in builtin::builtin_rendered_targets(target_root, &spec.agents) {
            let abs_target = resolve_target_path(target_root, &rendered.target_path);
            if produced.contains(&abs_target) {
                continue;
            }
            if user_source_dirs
                .iter()
                .any(|dir| abs_target.starts_with(dir))
            {
                continue;
            }
            let disk_content = fs::read_to_string(&abs_target).ok();
            let status = match disk_content {
                Some(dc) if hash_content(&dc) == hash_content(&rendered.content) => {
                    DeploymentStatus::UpToDate
                }
                Some(_) => DeploymentStatus::Update,
                None => DeploymentStatus::Create,
            };
            produced.insert(abs_target.clone());
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

    Ok(Plan {
        items,
        owners: contested_owners,
    })
}

fn expand_rules(
    root: &Path,
    spec: &Spec,
    ignore_fetch_failures: bool,
) -> anyhow::Result<ExpansionOutcome> {
    let mut expanded_rules = Vec::new();
    let mut skipped_fetches = Vec::new();

    for (rule_index, rule) in spec.rules.iter().enumerate() {
        match expand_rule(root, rule) {
            Ok(items) => expanded_rules.push(ExpandedRule { rule_index, items }),
            Err(err) => {
                if let Some(skipped) = skipped_github_fetch(rule, &err, ignore_fetch_failures) {
                    skipped_fetches.push(skipped);
                    continue;
                }
                return Err(err);
            }
        }
    }

    Ok(ExpansionOutcome {
        expanded_rules,
        skipped_fetches,
    })
}

fn skipped_github_fetch(
    rule: &Rule,
    err: &anyhow::Error,
    ignore_fetch_failures: bool,
) -> Option<SkippedGithubFetch> {
    if !ignore_fetch_failures {
        return None;
    }
    let fetch_err = err.downcast_ref::<GithubFetchError>()?;
    if !fetch_err.is_ignorable_fetch_failure() {
        return None;
    }

    let source = match &rule.source {
        Source::Github(g) => describe_github_source(g),
        Source::Local(path) => path.display().to_string(),
    };

    Some(SkippedGithubFetch {
        rule_id: rule.id.clone(),
        source,
        message: fetch_err.to_string(),
    })
}

/// A path that multiple rules claim and which cannot be resolved without a
/// user decision (recorded in `.rtango/lock.yaml` under `owners:` or via
/// `rtango own`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmbiguousPath {
    pub path: PathBuf,
    /// All rule ids that claim this path, sorted.
    pub candidates: Vec<String>,
}

/// One outcome of trying to resolve a single path: either we know the owner
/// or we need a user decision.
enum Resolution {
    Owner(String),
    Ambiguous(AmbiguousPath),
}

fn resolve_one(
    path: &Path,
    claimants: &HashSet<String>,
    rule_kinds: &HashMap<&str, &RuleKind>,
    lock_owners: &HashMap<&Path, &str>,
) -> anyhow::Result<Resolution> {
    if claimants.len() == 1 {
        return Ok(Resolution::Owner(claimants.iter().next().unwrap().clone()));
    }

    // Multiple claimants. First, honor any user decision from the lock.
    if let Some(&locked) = lock_owners.get(path) {
        if claimants.contains(locked) {
            return Ok(Resolution::Owner(locked.to_string()));
        }
        // Lock points at a rule that no longer claims this path; fall
        // through to heuristic.
    }

    // Heuristic: a single-file rule is strictly more specific than a set.
    let singles: Vec<&String> = claimants
        .iter()
        .filter(|r| {
            matches!(
                rule_kinds.get(r.as_str()),
                Some(RuleKind::Skill { .. })
                    | Some(RuleKind::Agent { .. })
                    | Some(RuleKind::System)
            )
        })
        .collect();
    match singles.len() {
        0 => {}
        1 => return Ok(Resolution::Owner(singles[0].clone())),
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

    let mut names: Vec<String> = claimants.iter().cloned().collect();
    names.sort();
    Ok(Resolution::Ambiguous(AmbiguousPath {
        path: path.to_path_buf(),
        candidates: names,
    }))
}

/// Return every path the spec touches that currently has no unambiguous
/// owner. Callers (e.g. `rtango sync`) can prompt the user for a decision
/// and re-run with an updated lock.
pub fn find_ambiguities(
    root: &Path,
    spec: &Spec,
    lock: &Lock,
) -> anyhow::Result<Vec<AmbiguousPath>> {
    Ok(find_ambiguities_with_fetch_failures(root, spec, lock, false)?.ambiguities)
}

pub fn find_ambiguities_with_fetch_failures(
    root: &Path,
    spec: &Spec,
    lock: &Lock,
    ignore_fetch_failures: bool,
) -> anyhow::Result<AmbiguityReport> {
    find_ambiguities_with_fetch_failures_in_scope(
        root,
        root,
        spec,
        lock,
        ignore_fetch_failures,
        TargetScope::Project,
    )
}

pub fn find_ambiguities_with_fetch_failures_in_scope(
    target_root: &Path,
    source_root: &Path,
    spec: &Spec,
    lock: &Lock,
    ignore_fetch_failures: bool,
    scope: TargetScope,
) -> anyhow::Result<AmbiguityReport> {
    let expansion = expand_rules(source_root, spec, ignore_fetch_failures)?;
    let candidates = collect_candidates_from_expanded_rules(
        target_root,
        spec,
        &scope,
        &expansion.expanded_rules,
    )?;
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

    let mut out = Vec::new();
    for (path, claimants) in &candidates {
        if let Resolution::Ambiguous(a) = resolve_one(path, claimants, &rule_kinds, &lock_owners)? {
            out.push(a);
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(AmbiguityReport {
        ambiguities: out,
        skipped_fetches: expansion.skipped_fetches,
    })
}

fn collect_candidates_from_expanded_rules(
    target_root: &Path,
    spec: &Spec,
    scope: &TargetScope,
    expanded_rules: &[ExpandedRule],
) -> anyhow::Result<HashMap<PathBuf, HashSet<String>>> {
    let mut candidates: HashMap<PathBuf, HashSet<String>> = HashMap::new();
    for expanded_rule in expanded_rules {
        let rule = &spec.rules[expanded_rule.rule_index];
        for item in &expanded_rule.items {
            let source_file = match &item.kind {
                ExpandedKind::Skill(s) => s.file.clone(),
                ExpandedKind::SkillAsset(asset) => asset.source_file.clone(),
                ExpandedKind::Agent(a) => a.file.clone(),
                ExpandedKind::System(s) => s.file.clone(),
            };
            candidates
                .entry(source_file)
                .or_default()
                .insert(rule.id.clone());
            for target in rule_targets(spec, rule) {
                let tp = resolve_target_path(
                    target_root,
                    &target_path_for(target_root, &target, &item.kind, scope)?,
                );
                candidates.entry(tp).or_default().insert(rule.id.clone());
            }
        }
    }
    Ok(candidates)
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
        match resolve_one(path, claimants, &rule_kinds, &lock_owners)? {
            Resolution::Owner(id) => {
                resolved.insert(path.clone(), id);
            }
            Resolution::Ambiguous(a) => {
                anyhow::bail!(
                    "ambiguous ownership for {}: rules {:?} all claim this path. \
                     Record a decision in .rtango/lock.yaml under `owners:` or narrow the spec.",
                    a.path.display(),
                    a.candidates
                );
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};

    use tempfile::TempDir;

    fn setup_copilot_skill(root: &Path, name: &str, body: &str) {
        let dir = root.join(format!(".github/skills/{name}"));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("SKILL.md"), body).unwrap();
    }

    fn empty_lock() -> Lock {
        Lock {
            version: 1,
            tracked_agents: vec![],
            owners: vec![],
            deployments: vec![],
        }
    }

    #[test]
    fn skipped_rules_do_not_contribute_to_plan_items() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        setup_copilot_skill(root, "local", "Local body");

        let local_rule = Rule {
            id: "local".into(),
            source: Source::Local(PathBuf::from(".github/skills")),
            schema_agent: AgentName::new("copilot"),
            targets: None,
            on_target_modified: None,
            kind: RuleKind::skill_set(),
        };
        let skipped_rule = Rule {
            id: "remote".into(),
            source: Source::Github(crate::spec::GithubSource {
                github: "owner/repo".into(),
                r#ref: "main".into(),
                path: "skills".into(),
            }),
            schema_agent: AgentName::new("copilot"),
            targets: None,
            on_target_modified: None,
            kind: RuleKind::skill_set(),
        };
        let local_items = expand_rule(root, &local_rule).unwrap();
        let spec = Spec {
            version: 1,
            agents: vec![AgentName::new("claude-code")],
            defaults: crate::spec::Defaults::default(),
            rules: vec![local_rule, skipped_rule],
        };
        let expanded_rules = vec![ExpandedRule {
            rule_index: 0,
            items: local_items,
        }];

        let plan = compute_plan_from_expanded_rules(
            root,
            root,
            &spec,
            &empty_lock(),
            false,
            false,
            &TargetScope::Project,
            &expanded_rules,
        )
        .unwrap();

        assert_eq!(plan.items.len(), 1);
        assert_eq!(plan.items[0].rule_id, "local");
        assert_eq!(plan.items[0].status, DeploymentStatus::Create);
    }
}
