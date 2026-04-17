use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use rtango::cmd::add::AddOptions;
use rtango::spec::io::{load_spec, save_spec};
use rtango::spec::{AgentName, Defaults, Rule, RuleKind, Source, Spec};

fn write_spec(root: &Path, spec: &Spec) {
    save_spec(root, spec).unwrap();
}

fn empty_spec(agents: &[&str]) -> Spec {
    Spec {
        version: 1,
        agents: agents.iter().map(|n| AgentName::new(*n)).collect(),
        defaults: Defaults::default(),
        rules: vec![],
    }
}

fn seeded_spec() -> Spec {
    let mut s = empty_spec(&["claude-code"]);
    s.rules.push(Rule {
        id: "existing".into(),
        source: Source::Local(PathBuf::from(".claude/skills/")),
        schema_agent: AgentName::new("claude-code"),
        on_target_modified: None,
        kind: RuleKind::skill_set(),
    });
    s
}

fn local_skill_set_opts(id: &str, path: &str) -> AddOptions {
    AddOptions {
        id: id.into(),
        local: Some(PathBuf::from(path)),
        skill_set: true,
        ..AddOptions::default()
    }
}

#[test]
fn add_local_skill_set_appends_rule() {
    let tmp = TempDir::new().unwrap();
    write_spec(tmp.path(), &empty_spec(&["claude-code"]));

    rtango::cmd::add::exec(tmp.path(), local_skill_set_opts("my-skills", "skills/")).unwrap();

    let spec = load_spec(tmp.path()).unwrap();
    assert_eq!(spec.rules.len(), 1);
    let r = &spec.rules[0];
    assert_eq!(r.id, "my-skills");
    assert_eq!(r.schema_agent, AgentName::new("claude-code"));
    assert!(matches!(r.kind, RuleKind::SkillSet { .. }));
    match &r.source {
        Source::Local(p) => assert_eq!(p, &PathBuf::from("skills/")),
        _ => panic!("expected Local source"),
    }
}

#[test]
fn add_local_agent_set_uses_agent_set_kind() {
    let tmp = TempDir::new().unwrap();
    write_spec(tmp.path(), &empty_spec(&["claude-code"]));

    rtango::cmd::add::exec(
        tmp.path(),
        AddOptions {
            id: "my-agents".into(),
            local: Some(PathBuf::from("agents/")),
            agent_set: true,
            ..AddOptions::default()
        },
    )
    .unwrap();

    let spec = load_spec(tmp.path()).unwrap();
    assert!(matches!(spec.rules[0].kind, RuleKind::AgentSet { .. }));
}

#[test]
fn add_github_repo_parses_spec_string() {
    let tmp = TempDir::new().unwrap();
    write_spec(tmp.path(), &empty_spec(&["claude-code"]));

    rtango::cmd::add::exec(
        tmp.path(),
        AddOptions {
            id: "upstream".into(),
            repo: Some("owner/repo@v1.0.0:skills".into()),
            skill_set: true,
            ..AddOptions::default()
        },
    )
    .unwrap();

    let spec = load_spec(tmp.path()).unwrap();
    match &spec.rules[0].source {
        Source::Github(g) => {
            assert_eq!(g.github, "owner/repo");
            assert_eq!(g.r#ref, "v1.0.0");
            assert_eq!(g.path, "skills");
        }
        _ => panic!("expected Github source"),
    }
}

#[test]
fn add_github_repo_defaults_ref_and_path() {
    let tmp = TempDir::new().unwrap();
    write_spec(tmp.path(), &empty_spec(&["claude-code"]));

    rtango::cmd::add::exec(
        tmp.path(),
        AddOptions {
            id: "upstream".into(),
            repo: Some("owner/repo".into()),
            skill_set: true,
            ..AddOptions::default()
        },
    )
    .unwrap();

    let spec = load_spec(tmp.path()).unwrap();
    match &spec.rules[0].source {
        Source::Github(g) => {
            assert_eq!(g.github, "owner/repo");
            assert_eq!(g.r#ref, "main");
            assert_eq!(g.path, "");
        }
        _ => panic!("expected Github source"),
    }
}

#[test]
fn add_requires_source_flag() {
    let tmp = TempDir::new().unwrap();
    write_spec(tmp.path(), &empty_spec(&["claude-code"]));

    let err = rtango::cmd::add::exec(
        tmp.path(),
        AddOptions {
            id: "x".into(),
            skill_set: true,
            ..AddOptions::default()
        },
    )
    .unwrap_err();
    assert!(err.to_string().contains("source"), "err: {}", err);
}

#[test]
fn add_requires_kind_flag() {
    let tmp = TempDir::new().unwrap();
    write_spec(tmp.path(), &empty_spec(&["claude-code"]));

    let err = rtango::cmd::add::exec(
        tmp.path(),
        AddOptions {
            id: "x".into(),
            local: Some(PathBuf::from("skills/")),
            ..AddOptions::default()
        },
    )
    .unwrap_err();
    assert!(err.to_string().contains("kind"), "err: {}", err);
}

#[test]
fn add_rejects_duplicate_id() {
    let tmp = TempDir::new().unwrap();
    write_spec(tmp.path(), &seeded_spec());

    let err =
        rtango::cmd::add::exec(tmp.path(), local_skill_set_opts("existing", "other/")).unwrap_err();
    assert!(err.to_string().contains("already exists"), "err: {}", err);
}

#[test]
fn add_requires_agent_when_spec_has_multiple() {
    let tmp = TempDir::new().unwrap();
    write_spec(tmp.path(), &empty_spec(&["claude-code", "copilot"]));

    let err = rtango::cmd::add::exec(tmp.path(), local_skill_set_opts("x", "skills/")).unwrap_err();
    assert!(err.to_string().contains("multiple"), "err: {}", err);
}

#[test]
fn add_uses_explicit_schema_when_provided() {
    let tmp = TempDir::new().unwrap();
    write_spec(tmp.path(), &empty_spec(&["claude-code", "copilot"]));

    rtango::cmd::add::exec(
        tmp.path(),
        AddOptions {
            id: "x".into(),
            local: Some(PathBuf::from("skills/")),
            skill_set: true,
            schema: Some("copilot".into()),
            ..AddOptions::default()
        },
    )
    .unwrap();

    let spec = load_spec(tmp.path()).unwrap();
    assert_eq!(spec.rules[0].schema_agent, AgentName::new("copilot"));
}

#[test]
fn add_rejects_schema_not_in_spec() {
    let tmp = TempDir::new().unwrap();
    write_spec(tmp.path(), &empty_spec(&["claude-code"]));

    let err = rtango::cmd::add::exec(
        tmp.path(),
        AddOptions {
            id: "x".into(),
            local: Some(PathBuf::from("skills/")),
            skill_set: true,
            schema: Some("nonesuch".into()),
            ..AddOptions::default()
        },
    )
    .unwrap_err();
    assert!(err.to_string().contains("nonesuch"), "err: {}", err);
}

#[test]
fn add_persists_to_spec_yaml() {
    let tmp = TempDir::new().unwrap();
    write_spec(tmp.path(), &empty_spec(&["claude-code"]));

    rtango::cmd::add::exec(tmp.path(), local_skill_set_opts("persisted", "skills/")).unwrap();

    let yaml = fs::read_to_string(tmp.path().join(".rtango/spec.yaml")).unwrap();
    assert!(yaml.contains("persisted"));
    assert!(yaml.contains("skill-set"));
}

#[test]
fn add_single_skill_stores_name_description_and_tools() {
    let tmp = TempDir::new().unwrap();
    write_spec(tmp.path(), &empty_spec(&["claude-code"]));

    rtango::cmd::add::exec(
        tmp.path(),
        AddOptions {
            id: "my-skill".into(),
            local: Some(PathBuf::from(".claude/skills/git-helper")),
            skill: true,
            name: Some("git-helper".into()),
            description: Some("Helps with git".into()),
            allowed_tools: Some("Read Grep Bash".into()),
            ..AddOptions::default()
        },
    )
    .unwrap();

    let spec = load_spec(tmp.path()).unwrap();
    match &spec.rules[0].kind {
        RuleKind::Skill {
            name,
            description,
            allowed_tools,
        } => {
            assert_eq!(name.as_deref(), Some("git-helper"));
            assert_eq!(description.as_deref(), Some("Helps with git"));
            assert_eq!(allowed_tools.as_deref(), Some("Read Grep Bash"));
        }
        other => panic!("expected Skill kind, got {:?}", other),
    }
}

#[test]
fn add_single_agent_stores_overrides() {
    let tmp = TempDir::new().unwrap();
    write_spec(tmp.path(), &empty_spec(&["claude-code"]));

    rtango::cmd::add::exec(
        tmp.path(),
        AddOptions {
            id: "review".into(),
            local: Some(PathBuf::from(".claude/agents/review.agent.md")),
            agent: true,
            name: Some("review".into()),
            description: Some("PR review".into()),
            ..AddOptions::default()
        },
    )
    .unwrap();

    let spec = load_spec(tmp.path()).unwrap();
    match &spec.rules[0].kind {
        RuleKind::Agent {
            name,
            description,
            allowed_tools,
        } => {
            assert_eq!(name.as_deref(), Some("review"));
            assert_eq!(description.as_deref(), Some("PR review"));
            assert!(allowed_tools.is_none());
        }
        other => panic!("expected Agent kind, got {:?}", other),
    }
}

#[test]
fn add_skill_set_stores_include() {
    let tmp = TempDir::new().unwrap();
    write_spec(tmp.path(), &empty_spec(&["claude-code"]));

    rtango::cmd::add::exec(
        tmp.path(),
        AddOptions {
            id: "picked".into(),
            local: Some(PathBuf::from("skills/")),
            skill_set: true,
            include: vec!["alpha".into(), "beta".into()],
            ..AddOptions::default()
        },
    )
    .unwrap();

    let spec = load_spec(tmp.path()).unwrap();
    match &spec.rules[0].kind {
        RuleKind::SkillSet { include, exclude } => {
            assert_eq!(include, &vec!["alpha".to_string(), "beta".to_string()]);
            assert!(exclude.is_empty());
        }
        other => panic!("expected SkillSet kind, got {:?}", other),
    }
}

#[test]
fn add_agent_set_stores_exclude() {
    let tmp = TempDir::new().unwrap();
    write_spec(tmp.path(), &empty_spec(&["claude-code"]));

    rtango::cmd::add::exec(
        tmp.path(),
        AddOptions {
            id: "mostly".into(),
            local: Some(PathBuf::from("agents/")),
            agent_set: true,
            exclude: vec!["draft".into()],
            ..AddOptions::default()
        },
    )
    .unwrap();

    let spec = load_spec(tmp.path()).unwrap();
    match &spec.rules[0].kind {
        RuleKind::AgentSet { include, exclude } => {
            assert!(include.is_empty());
            assert_eq!(exclude, &vec!["draft".to_string()]);
        }
        other => panic!("expected AgentSet kind, got {:?}", other),
    }
}

#[test]
fn add_system_kind_persists_with_no_extra_fields() {
    let tmp = TempDir::new().unwrap();
    write_spec(tmp.path(), &empty_spec(&["claude-code"]));

    rtango::cmd::add::exec(
        tmp.path(),
        AddOptions {
            id: "instructions".into(),
            local: Some(PathBuf::from("docs/CLAUDE.md")),
            system: true,
            ..AddOptions::default()
        },
    )
    .unwrap();

    let spec = load_spec(tmp.path()).unwrap();
    assert!(matches!(spec.rules[0].kind, RuleKind::System));

    let yaml = fs::read_to_string(tmp.path().join(".rtango/spec.yaml")).unwrap();
    assert!(yaml.contains("kind: system"), "yaml: {}", yaml);
}

#[test]
fn add_rejects_multiple_kinds() {
    let tmp = TempDir::new().unwrap();
    write_spec(tmp.path(), &empty_spec(&["claude-code"]));

    let err = rtango::cmd::add::exec(
        tmp.path(),
        AddOptions {
            id: "x".into(),
            local: Some(PathBuf::from("skills/")),
            skill: true,
            skill_set: true,
            ..AddOptions::default()
        },
    )
    .unwrap_err();
    assert!(err.to_string().contains("only one kind"), "err: {}", err);
}
