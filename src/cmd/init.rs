use std::path::Path;

use anyhow::bail;

use crate::agent::{self, SourceKind};
use crate::engine::{compute_plan, managed_gitignore_entries};
use crate::error::RtangoError;
use crate::spec::io::{gitignore_update, lock_path, rtango_dir, spec_path, write_gitignore};
use crate::spec::{AgentName, Defaults, Lock, Rule, RuleKind, Source, Spec};

pub fn exec(
    root: &Path,
    force: bool,
    no_detect: bool,
    gitignore_targets: bool,
) -> anyhow::Result<()> {
    let spec_path = spec_path(root);
    let lock_path = lock_path(root);

    if spec_path.exists() && !force {
        bail!(RtangoError::SpecExists);
    }

    let detected = if no_detect {
        Vec::new()
    } else {
        let detected = agent::detect_agents(root);
        if detected.is_empty() {
            bail!(RtangoError::NoAgentsDetected);
        }
        detected
    };

    let agent_names: Vec<AgentName> = detected.iter().map(|d| d.name.clone()).collect();

    let mut rules = Vec::new();
    for agent in &detected {
        for source in &agent.sources {
            rules.push(Rule {
                id: source.id.clone(),
                source: Source::Local(source.path.clone()),
                schema_agent: agent.name.clone(),
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
        agents: agent_names.clone(),
        defaults: Defaults {
            gitignore_targets,
            ..Defaults::default()
        },
        rules,
    };

    let lock = Lock {
        version: 1,
        tracked_agents: agent_names,
        owners: vec![],
        deployments: vec![],
    };

    let spec_yaml = serde_yml::to_string(&spec)?;
    let lock_yaml = serde_yml::to_string(&lock)?;

    std::fs::create_dir_all(rtango_dir(root))?;
    std::fs::write(&spec_path, &spec_yaml)?;
    std::fs::write(&lock_path, &lock_yaml)?;

    if gitignore_targets {
        let plan = compute_plan(root, &spec, &lock, true, true)?;
        let update = gitignore_update(root, &managed_gitignore_entries(&plan, None))?;
        if update.changed {
            write_gitignore(root, &update.content)?;
        }
    }

    Ok(())
}
