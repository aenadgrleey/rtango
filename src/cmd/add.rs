use std::path::{Path, PathBuf};

use crate::spec::io::{load_spec, save_spec};
use crate::spec::{AgentName, GithubSource, Rule, RuleKind, Source};

/// Options forwarded from the `rtango add` CLI.
///
/// Exactly one of `local` / `repo` must be set (source); exactly one of
/// `skill` / `agent` / `skill_set` / `agent_set` must be set (kind). The
/// override and filter fields are kind-specific and rejected if used with
/// the wrong kind.
#[derive(Debug, Default)]
pub struct AddOptions {
    pub id: String,
    pub local: Option<PathBuf>,
    pub repo: Option<String>,
    pub skill: bool,
    pub agent: bool,
    pub skill_set: bool,
    pub agent_set: bool,
    pub system: bool,
    pub schema: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub allowed_tools: Option<String>,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

/// Append a new rule to `.rtango/spec.yaml`.
pub fn exec(root: &Path, opts: AddOptions) -> anyhow::Result<()> {
    let source = match (opts.local, opts.repo) {
        (Some(p), None) => Source::Local(p),
        (None, Some(spec)) => Source::Github(parse_repo_spec(&spec)?),
        (Some(_), Some(_)) => anyhow::bail!("pass only one of --local/-l or --repo/-r"),
        (None, None) => anyhow::bail!("source required: pass --local/-l PATH or --repo/-r SPEC"),
    };

    let kind = match (opts.skill, opts.agent, opts.skill_set, opts.agent_set, opts.system) {
        (true, false, false, false, false) => RuleKind::Skill {
            name: opts.name,
            description: opts.description,
            allowed_tools: opts.allowed_tools,
        },
        (false, true, false, false, false) => RuleKind::Agent {
            name: opts.name,
            description: opts.description,
            allowed_tools: opts.allowed_tools,
        },
        (false, false, true, false, false) => RuleKind::SkillSet {
            include: opts.include,
            exclude: opts.exclude,
        },
        (false, false, false, true, false) => RuleKind::AgentSet {
            include: opts.include,
            exclude: opts.exclude,
        },
        (false, false, false, false, true) => RuleKind::System,
        (false, false, false, false, false) => anyhow::bail!(
            "kind required: pass --skill, --agent, --skill-set/--ss, --agent-set/--as, or --system"
        ),
        _ => anyhow::bail!(
            "pass only one kind of --skill, --agent, --skill-set/--ss, --agent-set/--as, or --system"
        ),
    };

    let mut spec = load_spec(root)?;

    if spec.rules.iter().any(|r| r.id == opts.id) {
        anyhow::bail!("rule '{}' already exists in spec", opts.id);
    }

    let schema = match opts.schema {
        Some(name) => {
            let name = AgentName::new(name);
            if !spec.agents.contains(&name) {
                anyhow::bail!("agent '{}' is not declared in spec.agents", name);
            }
            name
        }
        None => match spec.agents.as_slice() {
            [only] => only.clone(),
            [] => anyhow::bail!("spec has no agents; cannot infer schema_agent"),
            _ => anyhow::bail!("spec has multiple agents; specify one with --schema/-g"),
        },
    };

    spec.rules.push(Rule {
        id: opts.id.clone(),
        source,
        schema_agent: schema,
        on_target_modified: None,
        kind,
    });

    save_spec(root, &spec)?;
    println!("added rule '{}'", opts.id);
    Ok(())
}

/// Parse `owner/repo[@ref][:path]` into a `GithubSource`. Unset fields fall
/// back to `GithubSource` defaults (`ref = "main"`, empty path).
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
        anyhow::bail!("invalid repo spec '{}': expected owner/repo[@ref][:path]", s);
    }
    Ok(GithubSource { github, r#ref, path })
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
