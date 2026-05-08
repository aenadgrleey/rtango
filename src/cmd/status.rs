use std::path::Path;

use crate::engine::{
    DeploymentStatus, compute_plan_with_fetch_failures, managed_gitignore_entries,
};
use crate::spec::io::{gitignore_update, load_lock_or_empty, load_spec};

pub fn exec(root: &Path, rule: Option<String>, verbose: bool) -> anyhow::Result<()> {
    exec_with_options(root, rule, verbose, false)
}

pub fn exec_with_options(
    root: &Path,
    rule: Option<String>,
    verbose: bool,
    ignore_fetch_failures: bool,
) -> anyhow::Result<()> {
    let spec = load_spec(root)?;
    let lock = load_lock_or_empty(root)?;
    let report =
        compute_plan_with_fetch_failures(root, &spec, &lock, false, true, ignore_fetch_failures)?;
    super::print_skipped_github_fetches(&report.skipped_fetches);
    let plan = report.plan;

    let items: Vec<_> = plan
        .items
        .iter()
        .filter(|item| match &rule {
            Some(r) => item.rule_id == *r,
            None => true,
        })
        .collect();

    let gitignore = if spec.defaults.gitignore_targets && rule.is_none() {
        Some(gitignore_update(
            root,
            &managed_gitignore_entries(&plan, None),
        )?)
    } else {
        None
    };

    print_plan(&items, verbose, gitignore.as_ref());

    Ok(())
}

fn print_plan(
    items: &[&crate::engine::PlannedDeployment],
    verbose: bool,
    gitignore: Option<&crate::spec::io::GitignoreUpdate>,
) {
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

    if let Some(update) = gitignore {
        if update.changed {
            if update.existed {
                updates += 1;
                println!("  update   .gitignore");
            } else {
                creates += 1;
                println!("  create   .gitignore");
            }
        } else if verbose {
            up_to_date += 1;
            println!("  up-to-date .gitignore");
        }
    }

    println!(
        "\nSummary: {} create, {} update, {} orphan, {} conflict, {} up-to-date",
        creates, updates, orphans, conflicts, up_to_date
    );
}
