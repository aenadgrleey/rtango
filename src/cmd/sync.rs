use std::io::{self, BufRead, Write};
use std::path::Path;

use crate::engine::{
    AmbiguousPath, DeploymentStatus, Plan, compute_plan_with_fetch_failures, execute_plan,
    find_ambiguities_with_fetch_failures, managed_gitignore_entries,
};
use crate::spec::Ownership;
use crate::spec::io::{
    gitignore_update, load_lock_or_empty, load_spec, save_lock, write_gitignore,
};

/// Strategy for resolving set-vs-set ambiguity during `sync`. `None` from
/// `choose_owner` means the user declined to pick — sync aborts with the
/// normal ambiguity error.
pub trait Prompter {
    fn choose_owner(&mut self, ambiguity: &AmbiguousPath) -> anyhow::Result<Option<String>>;
}

/// Default prompter: reads a rule id from stdin. Returns `None` on EOF or an
/// empty/"abort" answer.
pub struct StdioPrompter;

impl Prompter for StdioPrompter {
    fn choose_owner(&mut self, ambiguity: &AmbiguousPath) -> anyhow::Result<Option<String>> {
        let mut out = io::stdout().lock();
        writeln!(out, "\nmultiple rules claim {}", ambiguity.path.display())?;
        for (i, id) in ambiguity.candidates.iter().enumerate() {
            writeln!(out, "  [{}] {}", i + 1, id)?;
        }
        write!(out, "pick an owner (number or id, empty to abort): ")?;
        out.flush()?;

        let mut line = String::new();
        let n = io::stdin().lock().read_line(&mut line)?;
        if n == 0 {
            return Ok(None);
        }
        let answer = line.trim();
        if answer.is_empty() {
            return Ok(None);
        }
        if let Ok(idx) = answer.parse::<usize>() {
            if idx >= 1 && idx <= ambiguity.candidates.len() {
                return Ok(Some(ambiguity.candidates[idx - 1].clone()));
            }
        }
        if ambiguity.candidates.iter().any(|c| c == answer) {
            return Ok(Some(answer.to_string()));
        }
        anyhow::bail!("'{}' is not one of the candidate rules", answer);
    }
}

pub fn exec(
    root: &Path,
    check: bool,
    force: bool,
    rule: Option<String>,
    adopt: bool,
) -> anyhow::Result<()> {
    exec_with_options(root, check, force, rule, adopt, false)
}

pub fn exec_with_options(
    root: &Path,
    check: bool,
    force: bool,
    rule: Option<String>,
    adopt: bool,
    ignore_fetch_failures: bool,
) -> anyhow::Result<()> {
    let mut prompter = StdioPrompter;
    exec_with_prompter_and_options(
        root,
        check,
        force,
        rule,
        adopt,
        &mut prompter,
        ignore_fetch_failures,
    )
}

pub fn exec_with_prompter(
    root: &Path,
    check: bool,
    force: bool,
    rule: Option<String>,
    adopt: bool,
    prompter: &mut dyn Prompter,
) -> anyhow::Result<()> {
    exec_with_prompter_and_options(root, check, force, rule, adopt, prompter, false)
}

pub fn exec_with_prompter_and_options(
    root: &Path,
    check: bool,
    force: bool,
    rule: Option<String>,
    adopt: bool,
    prompter: &mut dyn Prompter,
    ignore_fetch_failures: bool,
) -> anyhow::Result<()> {
    let spec = load_spec(root)?;
    let mut lock = load_lock_or_empty(root)?;

    // Resolve any set-vs-set ambiguities up front so compute_plan can proceed
    // without bailing. Each answer is persisted to .rtango/lock.yaml so future
    // syncs apply the same decision.
    let mut printed_skipped_fetches = false;
    loop {
        let report =
            find_ambiguities_with_fetch_failures(root, &spec, &lock, ignore_fetch_failures)?;
        if !printed_skipped_fetches {
            super::print_skipped_github_fetches(&report.skipped_fetches);
            printed_skipped_fetches = true;
        }
        if report.ambiguities.is_empty() {
            break;
        }
        for a in &report.ambiguities {
            let choice = prompter.choose_owner(a)?;
            let Some(rule_id) = choice else {
                anyhow::bail!(
                    "ambiguous ownership for {}: rules {:?} all claim this path. \
                     Record a decision in .rtango/lock.yaml under `owners:` or narrow the spec.",
                    a.path.display(),
                    a.candidates
                );
            };
            if !a.candidates.iter().any(|c| c == &rule_id) {
                anyhow::bail!(
                    "rule '{}' does not claim {}; candidates were {:?}",
                    rule_id,
                    a.path.display(),
                    a.candidates
                );
            }
            lock.owners.retain(|o| o.path != a.path);
            lock.owners.push(Ownership {
                path: a.path.clone(),
                rule_id,
            });
        }
        save_lock(root, &lock)?;
    }

    let report = compute_plan_with_fetch_failures(
        root,
        &spec,
        &lock,
        force || adopt,
        true,
        ignore_fetch_failures,
    )?;
    if !printed_skipped_fetches {
        super::print_skipped_github_fetches(&report.skipped_fetches);
    }
    let plan = report.plan;
    let gitignore = if spec.defaults.gitignore_targets && rule.is_none() {
        Some(gitignore_update(
            root,
            &managed_gitignore_entries(&plan, None),
        )?)
    } else {
        None
    };

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

    let mut is_clean = filtered_plan.is_clean();

    if is_clean {
        if gitignore.as_ref().is_some_and(|update| update.changed) {
            is_clean = false;
        } else {
            println!("Already up to date.");
        }
    }

    if !is_clean {
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

        if let Some(update) = &gitignore {
            if update.changed {
                if update.existed {
                    updates += 1;
                    println!("  update   .gitignore");
                } else {
                    creates += 1;
                    println!("  create   .gitignore");
                }
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

    if let Some(update) = &gitignore {
        if update.changed {
            write_gitignore(root, &update.content)?;
        }
    }

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
