use std::path::Path;

use anyhow::bail;

use crate::agent::{self, DetectedAgent, SourceKind};
use crate::error::RtangoError;
use crate::spec::io::{lock_path, rtango_dir, spec_path};
use crate::spec::{AgentName, Defaults, Lock, Rule, RuleKind, Source, Spec};

pub fn exec(
    root: &Path,
    force: bool,
    agents: Vec<String>,
    no_detect: bool,
) -> anyhow::Result<()> {
    let spec_path = spec_path(root);
    let lock_path = lock_path(root);

    if spec_path.exists() && !force {
        bail!(RtangoError::SpecExists);
    }

    let detected = if no_detect {
        if agents.is_empty() {
            bail!(RtangoError::NoAgentsDetected);
        }
        agents
            .into_iter()
            .map(|name| DetectedAgent {
                name: AgentName::new(name),
                sources: vec![],
            })
            .collect::<Vec<_>>()
    } else if !agents.is_empty() {
        agents
            .into_iter()
            .map(|name| DetectedAgent {
                name: AgentName::new(name),
                sources: vec![],
            })
            .collect::<Vec<_>>()
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
                    SourceKind::SkillSet => RuleKind::SkillSet {},
                    SourceKind::AgentSet => RuleKind::AgentSet {},
                },
            });
        }
    }

    let spec = Spec {
        version: 1,
        agents: agent_names.clone(),
        defaults: Defaults::default(),
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

    Ok(())
}
