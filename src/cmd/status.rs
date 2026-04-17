use std::path::Path;

use crate::engine::{compute_plan, DeploymentStatus};
use crate::spec::io::{load_lock_or_empty, load_spec};

pub fn exec(root: &Path, rule: Option<String>, verbose: bool) -> anyhow::Result<()> {
    let spec = load_spec(root)?;
    let lock = load_lock_or_empty(root)?;
    let plan = compute_plan(root, &spec, &lock, false)?;

    let items: Vec<_> = plan
        .items
        .iter()
        .filter(|item| match &rule {
            Some(r) => item.rule_id == *r,
            None => true,
        })
        .collect();

    print_plan(&items, verbose);

    Ok(())
}

fn print_plan(items: &[&crate::engine::PlannedDeployment], verbose: bool) {
    let mut creates = 0usize;
    let mut updates = 0usize;
    let mut conflicts = 0usize;
    let mut orphans = 0usize;
    let mut up_to_date = 0usize;

    for item in items {
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
                conflicts += 1;
                println!("  conflict {} ({})", item.target_path.display(), reason);
            }
            DeploymentStatus::Orphan => {
                orphans += 1;
                println!("  orphan   {}", item.target_path.display());
            }
            DeploymentStatus::UpToDate => {
                up_to_date += 1;
                if verbose {
                    println!("  up-to-date {}", item.target_path.display());
                }
            }
        }
    }

    println!(
        "\nSummary: {} create, {} update, {} orphan, {} conflict, {} up-to-date",
        creates, updates, orphans, conflicts, up_to_date
    );
}
