use std::path::Path;

use crate::engine::{compute_plan, execute_plan, DeploymentStatus, Plan};
use crate::spec::io::{load_lock_or_empty, load_spec, save_lock};

pub fn exec(
    root: &Path,
    check: bool,
    force: bool,
    rule: Option<String>,
    adopt: bool,
) -> anyhow::Result<()> {
    let spec = load_spec(root)?;
    let lock = load_lock_or_empty(root)?;
    let plan = compute_plan(root, &spec, &lock, force || adopt)?;

    // If filtering by rule, partition the plan items. Owners are always
    // carried through whole-spec so a per-rule sync doesn't drop unrelated
    // ownership decisions.
    let (filtered_plan, is_rule_filtered) = match &rule {
        Some(r) => {
            let filtered_items: Vec<_> = plan
                .items
                .into_iter()
                .filter(|item| item.rule_id == *r)
                .collect();
            (
                Plan {
                    items: filtered_items,
                    owners: plan.owners,
                },
                true,
            )
        }
        None => (plan, false),
    };

    // Print what will happen
    let mut creates = 0usize;
    let mut updates = 0usize;
    let mut orphans = 0usize;

    let is_clean = filtered_plan.is_clean();

    if is_clean {
        println!("Already up to date.");
    } else {
        for item in &filtered_plan.items {
            match &item.status {
                DeploymentStatus::Create => {
                    creates += 1;
                    println!("  create   {}", item.target_path.display());
                }
                DeploymentStatus::Update => {
                    updates += 1;
                    println!("  update   {}", item.target_path.display());
                }
                DeploymentStatus::Conflict { reason } => {
                    println!("  conflict {} ({})", item.target_path.display(), reason);
                }
                DeploymentStatus::Orphan => {
                    orphans += 1;
                    println!("  orphan   {}", item.target_path.display());
                }
                DeploymentStatus::UpToDate => {}
            }
        }
    }

    if check {
        if !is_clean {
            anyhow::bail!("not in sync");
        }
        return Ok(());
    }

    // Execute the plan
    let mut new_lock = execute_plan(root, &filtered_plan, &lock, false)?;

    // If we filtered by rule, merge back lock entries for unaffected rules
    if is_rule_filtered {
        let rule_filter = rule.as_deref().unwrap();
        let preserved: Vec<_> = lock
            .deployments
            .iter()
            .filter(|d| d.rule_id != rule_filter)
            .cloned()
            .collect();
        new_lock.deployments.extend(preserved);
    }

    new_lock.tracked_agents = spec.agents.clone();
    save_lock(root, &new_lock)?;

    println!(
        "Synced: {} created, {} updated, {} orphans removed",
        creates, updates, orphans
    );

    Ok(())
}
