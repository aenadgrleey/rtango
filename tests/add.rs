use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

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
        kind: RuleKind::SkillSet {},
    });
    s
}

#[test]
fn add_local_skill_set_appends_rule() {
    let tmp = TempDir::new().unwrap();
    write_spec(tmp.path(), &empty_spec(&["claude-code"]));

    rtango::cmd::add::exec(
        tmp.path(),
        "my-skills".into(),
        Some(PathBuf::from("skills/")),
        None,
        false,
        true,
        None,
    )
    .unwrap();

    let spec = load_spec(tmp.path()).unwrap();
    assert_eq!(spec.rules.len(), 1);
    let r = &spec.rules[0];
    assert_eq!(r.id, "my-skills");
    assert_eq!(r.schema_agent, AgentName::new("claude-code"));
    assert!(matches!(r.kind, RuleKind::SkillSet {}));
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
        "my-agents".into(),
        Some(PathBuf::from("agents/")),
        None,
        true,
        false,
        None,
    )
    .unwrap();

    let spec = load_spec(tmp.path()).unwrap();
    assert!(matches!(spec.rules[0].kind, RuleKind::AgentSet {}));
}

#[test]
fn add_github_repo_parses_spec_string() {
    let tmp = TempDir::new().unwrap();
    write_spec(tmp.path(), &empty_spec(&["claude-code"]));

    rtango::cmd::add::exec(
        tmp.path(),
        "upstream".into(),
        None,
        Some("owner/repo@v1.0.0:skills".into()),
        false,
        true,
        None,
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
        "upstream".into(),
        None,
        Some("owner/repo".into()),
        false,
        true,
        None,
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
        "x".into(),
        None,
        None,
        false,
        true,
        None,
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
        "x".into(),
        Some(PathBuf::from("skills/")),
        None,
        false,
        false,
        None,
    )
    .unwrap_err();
    assert!(err.to_string().contains("kind"), "err: {}", err);
}

#[test]
fn add_rejects_duplicate_id() {
    let tmp = TempDir::new().unwrap();
    write_spec(tmp.path(), &seeded_spec());

    let err = rtango::cmd::add::exec(
        tmp.path(),
        "existing".into(),
        Some(PathBuf::from("other/")),
        None,
        false,
        true,
        None,
    )
    .unwrap_err();
    assert!(err.to_string().contains("already exists"), "err: {}", err);
}

#[test]
fn add_requires_agent_when_spec_has_multiple() {
    let tmp = TempDir::new().unwrap();
    write_spec(tmp.path(), &empty_spec(&["claude-code", "copilot"]));

    let err = rtango::cmd::add::exec(
        tmp.path(),
        "x".into(),
        Some(PathBuf::from("skills/")),
        None,
        false,
        true,
        None,
    )
    .unwrap_err();
    assert!(err.to_string().contains("multiple"), "err: {}", err);
}

#[test]
fn add_uses_explicit_agent_when_provided() {
    let tmp = TempDir::new().unwrap();
    write_spec(tmp.path(), &empty_spec(&["claude-code", "copilot"]));

    rtango::cmd::add::exec(
        tmp.path(),
        "x".into(),
        Some(PathBuf::from("skills/")),
        None,
        false,
        true,
        Some("copilot".into()),
    )
    .unwrap();

    let spec = load_spec(tmp.path()).unwrap();
    assert_eq!(spec.rules[0].schema_agent, AgentName::new("copilot"));
}

#[test]
fn add_rejects_agent_not_in_spec() {
    let tmp = TempDir::new().unwrap();
    write_spec(tmp.path(), &empty_spec(&["claude-code"]));

    let err = rtango::cmd::add::exec(
        tmp.path(),
        "x".into(),
        Some(PathBuf::from("skills/")),
        None,
        false,
        true,
        Some("nonesuch".into()),
    )
    .unwrap_err();
    assert!(err.to_string().contains("nonesuch"), "err: {}", err);
}

#[test]
fn add_persists_to_spec_yaml() {
    let tmp = TempDir::new().unwrap();
    write_spec(tmp.path(), &empty_spec(&["claude-code"]));

    rtango::cmd::add::exec(
        tmp.path(),
        "persisted".into(),
        Some(PathBuf::from("skills/")),
        None,
        false,
        true,
        None,
    )
    .unwrap();

    let yaml = fs::read_to_string(tmp.path().join(".rtango/spec.yaml")).unwrap();
    assert!(yaml.contains("persisted"));
    assert!(yaml.contains("skill-set"));
}
