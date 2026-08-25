use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::agent;
use crate::engine::{
    DeploymentStatus, GlobalEnvironment, TargetScope, compute_plan_with_fetch_failures_in_scope,
    execute_plan, hash_content, resolve_target_path,
};
use crate::spec::io::{load_lock_or_empty_file, load_spec_file, save_lock_file};
use crate::spec::{AgentName, Lock, Source, Target};

/// Synchronise a standalone spec into user-level agent registries.
///
/// This is deliberately independent from the current working directory and
/// project `.rtango/` state. Relative local sources are resolved beside the
/// explicit spec file; rendered targets are resolved below the current user's
/// home directory by the global target layout.
pub fn exec(
    spec_path: Option<&Path>,
    agents: Vec<String>,
    lock_path: Option<PathBuf>,
    check: bool,
    force: bool,
    prune: bool,
) -> anyhow::Result<()> {
    let environment = GlobalEnvironment::from_process()?;
    let spec_path = resolve_spec_path(&environment.home, spec_path);
    if !spec_path.exists() {
        let message = format!(
            "Global spec is empty: {} does not exist. Add rules with `rtango add --global ...`.",
            spec_path.display()
        );
        if check {
            anyhow::bail!("{message}");
        }
        println!("{message}");
        return Ok(());
    }
    exec_at_with_environment(
        &spec_path,
        environment,
        agents,
        lock_path,
        check,
        force,
        prune,
    )
}

pub fn default_spec_path(home: &Path) -> PathBuf {
    home.join(".rtango/spec.yaml")
}

fn resolve_spec_path(home: &Path, requested: Option<&Path>) -> PathBuf {
    if let Some(path) = requested {
        return path.to_path_buf();
    }
    let canonical = default_spec_path(home);
    if canonical.exists() {
        return canonical;
    }
    // Read the previous name once for a painless migration, but always write
    // new global specs to ~/.rtango/spec.yaml.
    let legacy = home.join(".config/rtango/global.yaml");
    if legacy.exists() {
        eprintln!(
            "warning: using legacy global spec {}; migrate it to {}",
            legacy.display(),
            canonical.display()
        );
        legacy
    } else {
        canonical
    }
}

/// Testable application entry point with an injected global home directory.
pub fn exec_at(
    spec_path: &Path,
    home: &Path,
    agents: Vec<String>,
    lock_path: Option<PathBuf>,
    check: bool,
    force: bool,
    prune: bool,
) -> anyhow::Result<()> {
    exec_at_with_environment(
        spec_path,
        GlobalEnvironment::for_home(home),
        agents,
        lock_path,
        check,
        force,
        prune,
    )
}

/// Testable application entry point with an injected global environment.
/// This is also the integration boundary for tools that maintain multiple
/// Codex profiles with different `CODEX_HOME` directories.
pub fn exec_at_with_environment(
    spec_path: &Path,
    environment: GlobalEnvironment,
    agents: Vec<String>,
    lock_path: Option<PathBuf>,
    check: bool,
    force: bool,
    prune: bool,
) -> anyhow::Result<()> {
    let home = &environment.home;
    let spec_path = fs::canonicalize(spec_path)
        .map_err(|err| anyhow::anyhow!("failed to resolve spec {}: {err}", spec_path.display()))?;
    let source_root = spec_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("spec path has no parent: {}", spec_path.display()))?;
    let mut spec = load_spec_file(&spec_path)?;
    normalize_local_sources(&mut spec, source_root, home);
    normalize_target_homes(&mut spec, home);

    let cli_targets = dedupe_agents(agents);
    if !cli_targets.is_empty() {
        // CLI targets remain a compatibility override. Without them, the
        // spec's agents and per-rule targets are authoritative.
        spec.agents = cli_targets;
    }
    if spec.rules.is_empty() {
        println!("Global spec is empty: {}", spec_path.display());
        if !prune {
            return Ok(());
        }
    } else {
        validate_targets(&spec)?;
    }

    // Only repository-specific concerns have no meaning in registry mode.
    let ignored = [
        "defaults.gitignore_targets (.gitignore is never changed)",
        "spec.local.yaml (the selected spec is self-contained)",
        "project target paths, ownership, and built-in skills",
    ];
    println!("Global sync: ignoring {}", ignored.join(", "));

    let lock_path = lock_path.unwrap_or_else(|| default_lock_path(&spec_path));
    let lock = load_lock_or_empty_file(&lock_path)?;
    // Ownership decisions belong to project sync. Global registry mode has no
    // `own` command and must not silently reuse project lock decisions.
    let planning_lock = Lock {
        version: lock.version,
        tracked_agents: lock.tracked_agents.clone(),
        owners: Vec::new(),
        deployments: lock.deployments.clone(),
    };
    let report = compute_plan_with_fetch_failures_in_scope(
        home,
        source_root,
        &spec,
        &planning_lock,
        force,
        false,
        false,
        TargetScope::Global(environment.clone()),
    )?;
    let mut plan = report.plan;
    print_skipped_fetches(&report.skipped_fetches);

    let orphan_deployments: Vec<_> = lock
        .deployments
        .iter()
        .filter(|deployment| {
            plan.items.iter().any(|item| {
                item.status == DeploymentStatus::Orphan
                    && item.rule_id == deployment.rule_id
                    && item.agent == deployment.agent
                    && item.target_path == deployment.content
            })
        })
        .cloned()
        .collect();

    if !prune {
        plan.items
            .retain(|item| item.status != DeploymentStatus::Orphan);
        if !orphan_deployments.is_empty() {
            println!(
                "  orphan   {} (kept; pass --prune to remove)",
                orphan_deployments.len()
            );
        }
    }

    if prune {
        validate_prunable_orphans(&environment, &orphan_deployments)?;
    }

    for item in &plan.items {
        match &item.status {
            DeploymentStatus::Create => println!("  create   {}", item.target_path.display()),
            DeploymentStatus::Update => println!("  update   {}", item.target_path.display()),
            DeploymentStatus::Conflict { reason } => {
                println!("  conflict {} ({reason})", item.target_path.display())
            }
            DeploymentStatus::Orphan => println!("  orphan   {}", item.target_path.display()),
            DeploymentStatus::UpToDate => {}
        }
    }

    if check {
        if !plan.is_clean() || (prune && !orphan_deployments.is_empty()) {
            anyhow::bail!("global registries are not in sync");
        }
        println!("Already up to date.");
        return Ok(());
    }

    let mut new_lock = execute_plan(home, &plan, &lock, false)?;
    if !prune {
        new_lock.deployments.extend(orphan_deployments);
    }
    new_lock.tracked_agents = spec.agents.clone();
    save_lock_file(&lock_path, &new_lock)?;

    let creates = plan
        .items
        .iter()
        .filter(|item| item.status == DeploymentStatus::Create)
        .count();
    let updates = plan
        .items
        .iter()
        .filter(|item| item.status == DeploymentStatus::Update)
        .count();
    println!(
        "Global sync complete: {} created, {} updated{}",
        creates,
        updates,
        if prune { ", orphans pruned" } else { "" }
    );
    Ok(())
}

fn default_lock_path(spec_path: &Path) -> PathBuf {
    let mut path = spec_path.to_path_buf();
    path.set_extension("lock.yaml");
    path
}

fn normalize_local_sources(spec: &mut crate::spec::Spec, source_root: &Path, home: &Path) {
    for rule in &mut spec.rules {
        if let Source::Local(path) = &mut rule.source {
            let resolved = if path == Path::new("~") {
                home.to_path_buf()
            } else if let Ok(relative) = path.strip_prefix("~/") {
                home.join(relative)
            } else if path.is_absolute() {
                path.clone()
            } else {
                source_root.join(&*path)
            };
            *path = resolved;
        }
    }
}

fn normalize_target_homes(spec: &mut crate::spec::Spec, home: &Path) {
    for rule in &mut spec.rules {
        if let Some(targets) = &mut rule.targets {
            for target in targets {
                if let Some(path) = &mut target.home {
                    if *path == Path::new("~") {
                        *path = home.to_path_buf();
                    } else if let Ok(relative) = path.strip_prefix("~/") {
                        *path = home.join(relative);
                    }
                }
            }
        }
    }
}

fn dedupe_agents(agents: Vec<String>) -> Vec<AgentName> {
    let mut seen = HashSet::new();
    agents
        .into_iter()
        .filter(|agent| seen.insert(agent.clone()))
        .map(AgentName::new)
        .collect()
}

fn validate_targets(spec: &crate::spec::Spec) -> anyhow::Result<()> {
    let mut targets = spec
        .agents
        .iter()
        .cloned()
        .map(|agent| Target { agent, home: None })
        .collect::<Vec<_>>();
    for rule in &spec.rules {
        if let Some(rule_targets) = &rule.targets {
            if rule_targets.is_empty() {
                anyhow::bail!("rule '{}' has an empty targets list", rule.id);
            }
            targets.extend(rule_targets.iter().cloned());
        }
    }
    if targets.is_empty() {
        anyhow::bail!("global spec has no targets: add agents or rule-level targets");
    }
    for target in targets {
        if target.agent.as_str() == "plain"
            || !matches!(
                target.agent.as_str(),
                "claude-code" | "codex" | "copilot" | "cursor" | "opencode" | "pi"
            )
            || agent::frontmatter_writer(&target.agent).is_none()
        {
            anyhow::bail!("unknown global target agent: {}", target.agent);
        }
    }
    Ok(())
}

fn print_skipped_fetches(fetches: &[crate::engine::SkippedGithubFetch]) {
    for skipped in fetches {
        eprintln!(
            "warning: skipped GitHub rule '{}' ({}): {}",
            skipped.rule_id, skipped.source, skipped.message
        );
    }
}

fn validate_prunable_orphans(
    environment: &GlobalEnvironment,
    orphan_deployments: &[crate::spec::Deployment],
) -> anyhow::Result<()> {
    for deployment in orphan_deployments {
        let target = resolve_target_path(&environment.home, &deployment.content);
        if !global_registry_roots(environment)
            .iter()
            .any(|root| target.starts_with(root))
        {
            anyhow::bail!(
                "refusing to prune global target outside current home: {}",
                target.display()
            );
        }
        if let Ok(content) = fs::read_to_string(&target) {
            if hash_content(&content) != deployment.content_hash {
                anyhow::bail!(
                    "refusing to prune manually modified global target: {}",
                    target.display()
                );
            }
        }
    }
    Ok(())
}

fn global_registry_roots(environment: &GlobalEnvironment) -> Vec<PathBuf> {
    vec![
        environment.home.clone(),
        environment.codex_home.clone(),
        environment.copilot_home.clone(),
        environment.xdg_config_home.join("opencode"),
    ]
}
