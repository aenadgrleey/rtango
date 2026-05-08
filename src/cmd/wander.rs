use std::collections::HashSet;
use std::path::Path;

use anyhow::bail;

use crate::agent::{self, SourceKind};
use crate::engine::{DeploymentStatus, compute_plan_with_fetch_failures, execute_plan};
use crate::error::RtangoError;
use crate::spec::{AgentName, Defaults, Lock, Rule, RuleKind, Source, Spec};

/// Run init + sync in-memory: auto-detect sources, render for the detected
/// agents plus any `--target` agents, write target files, and leave no
/// `.rtango/spec.yaml` or `.rtango/lock.yaml` behind.
///
/// Agents named via `--target` are treated as outputs, not sources: their
/// folders are excluded from detection so the previous run's rendered files
/// don't become inputs on a later invocation.
///
/// `force` is always on — without a lock there's no prior state to reason
/// about, so pre-existing target files are overwritten.
pub fn exec(root: &Path, targets: Vec<String>) -> anyhow::Result<()> {
    exec_with_options(root, targets, false)
}

pub fn exec_with_options(
    root: &Path,
    targets: Vec<String>,
    ignore_fetch_failures: bool,
) -> anyhow::Result<()> {
    let target_agents: Vec<AgentName> = targets.into_iter().map(AgentName::new).collect();
    let target_set: HashSet<&AgentName> = target_agents.iter().collect();

    let detected: Vec<_> = agent::detect_agents(root)
        .into_iter()
        .filter(|d| !target_set.contains(&d.name))
        .collect();

    if detected.is_empty() {
        bail!(RtangoError::NoAgentsDetected);
    }

    let detected_agent_names: Vec<AgentName> = detected.iter().map(|d| d.name.clone()).collect();
    let mut spec_agents = detected_agent_names.clone();
    for t in &target_agents {
        if !spec_agents.contains(t) {
            spec_agents.push(t.clone());
        }
    }

    let mut rules = Vec::new();
    for a in &detected {
        for source in &a.sources {
            rules.push(Rule {
                id: source.id.clone(),
                source: Source::Local(source.path.clone()),
                schema_agent: a.name.clone(),
                on_target_modified: None,
                kind: match source.kind {
                    SourceKind::SkillSet => RuleKind::skill_set(),
                    SourceKind::AgentSet => RuleKind::agent_set(),
                },
            });
        }
    }

    let spec = Spec {
        version: 1,
        agents: spec_agents,
        defaults: Defaults::default(),
        rules,
    };
    let lock = Lock {
        version: 1,
        tracked_agents: vec![],
        owners: vec![],
        deployments: vec![],
    };

    let report =
        compute_plan_with_fetch_failures(root, &spec, &lock, true, true, ignore_fetch_failures)?;
    super::print_skipped_github_fetches(&report.skipped_fetches);
    let plan = report.plan;
    let _new_lock = execute_plan(root, &plan, &lock, false)?;

    let mut creates = 0usize;
    let mut updates = 0usize;
    for item in &plan.items {
        match item.status {
            DeploymentStatus::Create => {
                creates += 1;
                println!("  create   {}", item.target_path.display());
            }
            DeploymentStatus::Update => {
                updates += 1;
                println!("  update   {}", item.target_path.display());
            }
            _ => {}
        }
    }

    if creates + updates == 0 {
        println!(
            "Nothing to render. Pass `--target AGENT` to render the detected sources \
             for additional agents."
        );
    } else {
        println!(
            "Wandered: {} created, {} updated (no .rtango/ written)",
            creates, updates
        );
    }

    Ok(())
}
