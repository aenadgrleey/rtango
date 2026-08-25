use std::path::{Path, PathBuf};

use crate::cmd::global_sync::default_spec_path;
use crate::spec::io::{load_main_spec, load_spec_file, save_spec, save_spec_file};
use crate::spec::{AgentName, GithubSource, Rule, RuleKind, Source, Spec, Target};

/// Options forwarded from the `rtango add` CLI.
///
/// Exactly one of `local` / `repo` must be set (source); exactly one of
/// `skill` / `agent` / `skill_set` / `agent_set` / `system` / `collection_kind`
/// must be set (kind).
#[derive(Debug, Default, Clone)]
pub struct AddOptions {
    pub id: String,
    pub local: Option<PathBuf>,
    pub repo: Option<String>,
    pub skill: bool,
    pub agent: bool,
    pub skill_set: bool,
    pub agent_set: bool,
    pub system: bool,
    pub collection_kind: bool,
    pub schema: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub allowed_tools: Option<String>,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

/// Append a new rule to `.rtango/spec.yaml`.
pub fn exec(root: &Path, opts: AddOptions) -> anyhow::Result<()> {
    let mut spec = load_main_spec(root)?;
    if spec.rules.iter().any(|rule| rule.id == opts.id) {
        anyhow::bail!("rule '{}' already exists in spec", opts.id);
    }
    spec.rules.push(build_rule(&opts, &spec, None, &[])?);

    save_spec(root, &spec)?;
    println!("added rule '{}'", opts.id);
    Ok(())
}

/// Add a single skill or agent to the user's global registry spec. If no
/// source is supplied, a native-free editable scaffold is created beside the
/// spec so the command remains useful from an empty home directory.
pub fn exec_global(_root: &Path, opts: AddOptions, target_args: Vec<String>) -> anyhow::Result<()> {
    let home =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("could not determine home directory"))?;
    exec_global_at(&home, opts, target_args)
}

pub fn exec_global_at(
    home: &Path,
    mut opts: AddOptions,
    target_args: Vec<String>,
) -> anyhow::Result<()> {
    let path = default_spec_path(home);
    let mut spec = if path.exists() {
        load_spec_file(&path)?
    } else {
        Spec {
            version: 1,
            agents: Vec::new(),
            defaults: Default::default(),
            rules: Vec::new(),
        }
    };
    if spec.rules.iter().any(|rule| rule.id == opts.id) {
        anyhow::bail!("rule '{}' already exists in global spec", opts.id);
    }

    let targets = parse_targets(&target_args, home)?;
    scaffold_global_source(home, &mut opts)?;
    let rule = build_rule(&opts, &spec, Some(home), &targets)?;
    if spec.agents.is_empty() && !targets.is_empty() {
        spec.agents = targets.iter().map(|target| target.agent.clone()).collect();
        spec.agents.sort_by(|a, b| a.0.cmp(&b.0));
        spec.agents.dedup();
    }
    spec.rules.push(rule);
    save_spec_file(&path, &spec)?;
    println!("added rule '{}' to {}", opts.id, path.display());
    Ok(())
}

fn build_rule(
    opts: &AddOptions,
    spec: &Spec,
    global_home: Option<&Path>,
    targets: &[Target],
) -> anyhow::Result<Rule> {
    let source = match (opts.local.as_ref(), opts.repo.as_ref()) {
        (Some(_), Some(_)) => anyhow::bail!("pass only one of --local/-l or --repo/-r"),
        (Some(p), None) => {
            let path = if global_home.is_some() {
                if p.is_absolute() {
                    p.clone()
                } else {
                    std::env::current_dir()?.join(p)
                }
            } else {
                p.clone()
            };
            if let Some(home) = global_home {
                let registry_root = home.join(".rtango");
                path.strip_prefix(&registry_root)
                    .map(PathBuf::from)
                    .map(Source::Local)
                    .unwrap_or(Source::Local(path))
            } else {
                Source::Local(path)
            }
        }
        (None, Some(repo)) => Source::Github(parse_repo_spec(repo)?),
        (None, None) => anyhow::bail!(
            "source required: pass --local/-l PATH, --repo/-r SPEC, or use global scaffold mode"
        ),
    };
    let kind = match (
        opts.skill,
        opts.agent,
        opts.skill_set,
        opts.agent_set,
        opts.system,
        opts.collection_kind,
    ) {
        (true, false, false, false, false, false) => RuleKind::Skill {
            name: opts.name.clone(),
            description: opts.description.clone(),
            allowed_tools: opts.allowed_tools.clone(),
        },
        (false, true, false, false, false, false) => RuleKind::Agent {
            name: opts.name.clone(),
            description: opts.description.clone(),
            allowed_tools: opts.allowed_tools.clone(),
        },
        (false, false, true, false, false, false) => RuleKind::SkillSet {
            include: opts.include.clone(),
            exclude: opts.exclude.clone(),
        },
        (false, false, false, true, false, false) => RuleKind::AgentSet {
            include: opts.include.clone(),
            exclude: opts.exclude.clone(),
        },
        (false, false, false, false, true, false) => RuleKind::System,
        (false, false, false, false, false, true) => RuleKind::Collection {
            include: opts.include.clone(),
            exclude: opts.exclude.clone(),
            schema_override: opts.schema.as_ref().map(AgentName::new),
        },
        (false, false, false, false, false, false) => anyhow::bail!(
            "kind required: pass --skill, --agent, --skill-set/--ss, --agent-set/--as, --system, or --collection-kind/--col"
        ),
        _ => anyhow::bail!(
            "pass only one kind of --skill, --agent, --skill-set/--ss, --agent-set/--as, --system, or --collection-kind/--col"
        ),
    };
    let schema = match opts.schema.as_deref() {
        Some(name) => AgentName::new(name),
        None if let Some(target) = targets.first() => target.agent.clone(),
        None if spec.agents.len() == 1 => spec.agents[0].clone(),
        None if global_home.is_some() => AgentName::new("plain"),
        None if opts.collection_kind => spec
            .agents
            .first()
            .cloned()
            .unwrap_or_else(|| AgentName::new("plain")),
        None => anyhow::bail!("spec has multiple agents; specify one with --schema/-g"),
    };
    if global_home.is_none() && !spec.agents.contains(&schema) {
        anyhow::bail!("agent '{}' is not declared in spec.agents", schema);
    }
    Ok(Rule {
        id: opts.id.clone(),
        source,
        schema_agent: schema,
        targets: (!targets.is_empty()).then(|| targets.to_vec()),
        on_target_modified: None,
        kind,
    })
}

fn parse_targets(values: &[String], home: &Path) -> anyhow::Result<Vec<Target>> {
    values
        .iter()
        .map(|value| {
            let (agent, path) = value
                .split_once('=')
                .map_or((value.as_str(), None), |(agent, path)| (agent, Some(path)));
            if agent.is_empty() {
                anyhow::bail!("target agent cannot be empty");
            }
            let home_path = path.map(|path| {
                let path = PathBuf::from(path);
                if path == Path::new("~") {
                    home.to_path_buf()
                } else if let Ok(relative) = path.strip_prefix("~/") {
                    home.join(relative)
                } else {
                    path
                }
            });
            Ok(Target {
                agent: AgentName::new(agent),
                home: home_path,
            })
        })
        .collect()
}

fn scaffold_global_source(home: &Path, opts: &mut AddOptions) -> anyhow::Result<()> {
    if opts.local.is_some() || opts.repo.is_some() || (!opts.skill && !opts.agent) {
        return Ok(());
    }
    let (directory, filename, body): (&str, String, String) = if opts.skill {
        (
            "skills",
            "SKILL.md".into(),
            format!(
                "---\nname: {}\ndescription: {}\n---\n\n",
                opts.id,
                opts.description.as_deref().unwrap_or("")
            ),
        )
    } else {
        ("agents", format!("{}.agent.md", opts.id), String::new())
    };
    let relative = PathBuf::from("sources").join(directory).join(&opts.id);
    let path = home.join(".rtango").join(&relative).join(filename);
    if path.exists() {
        anyhow::bail!("refusing to overwrite existing scaffold {}", path.display());
    }
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(&path, body)?;
    opts.local = Some(if opts.skill {
        path.parent().unwrap().to_path_buf()
    } else {
        path
    });
    if opts.schema.is_none() {
        opts.schema = Some("plain".into());
    }
    Ok(())
}

/// Parse `owner/repo[@ref][:path]` into a `GithubSource`.
fn parse_repo_spec(s: &str) -> anyhow::Result<GithubSource> {
    let (head, path) = match s.split_once(':') {
        Some((h, p)) => (h, p.to_string()),
        None => (s, String::new()),
    };
    let (github, r#ref) = match head.split_once('@') {
        Some((g, r)) => (g.to_string(), r.to_string()),
        None => (head.to_string(), "main".to_string()),
    };
    if github.is_empty() || !github.contains('/') {
        anyhow::bail!(
            "invalid repo spec '{}': expected owner/repo[@ref][:path]",
            s
        );
    }
    Ok(GithubSource {
        github,
        r#ref,
        path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bare_repo() {
        let g = parse_repo_spec("owner/repo").unwrap();
        assert_eq!(g.github, "owner/repo");
        assert_eq!(g.r#ref, "main");
        assert_eq!(g.path, "");
    }

    #[test]
    fn parses_repo_with_ref() {
        let g = parse_repo_spec("owner/repo@v1.2.3").unwrap();
        assert_eq!(g.github, "owner/repo");
        assert_eq!(g.r#ref, "v1.2.3");
        assert_eq!(g.path, "");
    }

    #[test]
    fn parses_repo_with_path() {
        let g = parse_repo_spec("owner/repo:skills/").unwrap();
        assert_eq!(g.github, "owner/repo");
        assert_eq!(g.r#ref, "main");
        assert_eq!(g.path, "skills/");
    }

    #[test]
    fn parses_repo_with_ref_and_path() {
        let g = parse_repo_spec("owner/repo@abc123:sub/dir").unwrap();
        assert_eq!(g.github, "owner/repo");
        assert_eq!(g.r#ref, "abc123");
        assert_eq!(g.path, "sub/dir");
    }

    #[test]
    fn rejects_missing_slash() {
        assert!(parse_repo_spec("plainname").is_err());
    }
}
