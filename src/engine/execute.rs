use std::fs;
use std::path::Path;

use crate::spec::{Deployment, Lock};

use super::{DeploymentStatus, Plan, builtin};

/// Execute a sync plan: write target files and return the updated lock.
///
/// - `Create` / `Update`: writes the rendered content to disk.
/// - `Orphan`: deletes the target file.
/// - `Conflict`: returns error (caller must resolve conflicts first).
/// - `UpToDate`: skips.
///
/// If `check` is true, performs a dry-run — returns the new lock without
/// writing anything. Returns `Err` if there are unresolved conflicts.
pub fn execute_plan(
    root: &Path,
    plan: &Plan,
    _old_lock: &Lock,
    check: bool,
) -> anyhow::Result<Lock> {
    // Check for unresolved conflicts
    if plan.has_conflicts() {
        let reasons: Vec<&str> = plan
            .items
            .iter()
            .filter_map(|d| match &d.status {
                DeploymentStatus::Conflict { reason } => Some(reason.as_str()),
                _ => None,
            })
            .collect();
        anyhow::bail!("unresolved conflicts:\n{}", reasons.join("\n"));
    }

    let mut deployments = Vec::new();

    for item in &plan.items {
        match &item.status {
            DeploymentStatus::Create | DeploymentStatus::Update => {
                let target = root.join(&item.target_path);
                if !check {
                    if let Some(parent) = target.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::write(&target, &item.rendered_content)?;
                }
                // Built-in skills are written but not tracked in the lock.
                if item.rule_id != builtin::BUILTIN_RTANGO_RULE_ID {
                    deployments.push(Deployment {
                        rule_id: item.rule_id.clone(),
                        agent: item.agent.clone(),
                        source: item.source.clone(),
                        source_hash: item.source_hash.clone(),
                        content: item.target_path.clone(),
                        content_hash: super::hash_content(&item.rendered_content),
                    });
                }
            }
            DeploymentStatus::UpToDate => {
                // Built-in skills are not tracked in the lock.
                if item.rule_id != builtin::BUILTIN_RTANGO_RULE_ID {
                    // Keep in lock with current info
                    deployments.push(Deployment {
                        rule_id: item.rule_id.clone(),
                        agent: item.agent.clone(),
                        source: item.source.clone(),
                        source_hash: item.source_hash.clone(),
                        content: item.target_path.clone(),
                        content_hash: super::hash_content(&item.rendered_content),
                    });
                }
            }
            DeploymentStatus::Orphan => {
                let target = root.join(&item.target_path);
                if !check && target.exists() {
                    fs::remove_file(&target)?;
                }
                // Don't add to new lock — it's removed
            }
            DeploymentStatus::Conflict { .. } => {
                // Already handled above with early return
                unreachable!();
            }
        }
    }

    Ok(Lock {
        version: 1,
        tracked_agents: vec![], // Will be set by caller if needed
        owners: plan.owners.clone(),
        deployments,
    })
}
