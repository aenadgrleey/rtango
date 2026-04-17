use std::path::{Path, PathBuf};

use crate::spec::io::{load_spec, save_spec};
use crate::spec::{AgentName, GithubSource, Rule, RuleKind, Source};

/// Append a new rule to `.rtango/spec.yaml`.
///
/// Exactly one of `local` / `repo` must be given (source); exactly one of
/// `agent_set` / `skill_set` must be given (kind). `schema_agent` is required
/// only when the spec declares more than one agent — otherwise we pick the
/// sole agent automatically.
pub fn exec(
    root: &Path,
    id: String,
    local: Option<PathBuf>,
    repo: Option<String>,
    agent_set: bool,
    skill_set: bool,
    schema_agent: Option<String>,
) -> anyhow::Result<()> {
    let source = match (local, repo) {
        (Some(p), None) => Source::Local(p),
        (None, Some(spec)) => Source::Github(parse_repo_spec(&spec)?),
        (Some(_), Some(_)) => anyhow::bail!("pass only one of --local/-l or --repo/-r"),
        (None, None) => anyhow::bail!("source required: pass --local/-l PATH or --repo/-r SPEC"),
    };

    let kind = match (agent_set, skill_set) {
        (true, false) => RuleKind::AgentSet {},
        (false, true) => RuleKind::SkillSet {},
        (true, true) => anyhow::bail!("pass only one of --agent-set/--as or --skill-set/--ss"),
        (false, false) => {
            anyhow::bail!("kind required: pass --agent-set/--as or --skill-set/--ss")
        }
    };

    let mut spec = load_spec(root)?;

    if spec.rules.iter().any(|r| r.id == id) {
        anyhow::bail!("rule '{}' already exists in spec", id);
    }

    let schema = match schema_agent {
        Some(name) => {
            let name = AgentName::new(name);
            if !spec.agents.contains(&name) {
                anyhow::bail!(
                    "agent '{}' is not declared in spec.agents",
                    name
                );
            }
            name
        }
        None => match spec.agents.as_slice() {
            [only] => only.clone(),
            [] => anyhow::bail!("spec has no agents; cannot infer schema_agent"),
            _ => anyhow::bail!(
                "spec has multiple agents; specify one with --agent/-g"
            ),
        },
    };

    spec.rules.push(Rule {
        id: id.clone(),
        source,
        schema_agent: schema,
        on_target_modified: None,
        kind,
    });

    save_spec(root, &spec)?;
    println!("added rule '{}'", id);
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
