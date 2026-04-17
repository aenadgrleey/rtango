use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use rtango::spec::io::{load_lock, load_lock_or_empty, save_lock};
use rtango::spec::{
    AgentName, Defaults, Lock, Ownership, Rule, RuleKind, Source, Spec,
};

fn write_spec(root: &Path, spec: &Spec) {
    fs::create_dir_all(root.join(".rtango")).unwrap();
    let yaml = serde_yml::to_string(spec).unwrap();
    fs::write(root.join(".rtango/spec.yaml"), yaml).unwrap();
}

fn skill_set_rule(id: &str, path: &str, schema: &str) -> Rule {
    Rule {
        id: id.to_string(),
        source: Source::Local(PathBuf::from(path)),
        schema_agent: AgentName::new(schema),
        on_target_modified: None,
        kind: RuleKind::skill_set(),
    }
}

fn two_set_spec() -> Spec {
    Spec {
        version: 1,
        agents: vec![AgentName::new("claude-code")],
        defaults: Defaults::default(),
        rules: vec![
            skill_set_rule("a", ".github/skills", "copilot"),
            skill_set_rule("b", ".github/skills", "copilot"),
        ],
    }
}

#[test]
fn own_set_writes_ownership_entry() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    write_spec(root, &two_set_spec());

    let target = root.join(".github/skills/foo/SKILL.md");
    rtango::cmd::own::exec(root, target.clone(), Some("a".into()), false).unwrap();

    let lock = load_lock(root).unwrap();
    assert_eq!(lock.owners.len(), 1);
    assert_eq!(lock.owners[0].path, target);
    assert_eq!(lock.owners[0].rule_id, "a");
}

#[test]
fn own_rejects_unknown_rule() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    write_spec(root, &two_set_spec());

    let target = root.join(".github/skills/foo/SKILL.md");
    let err =
        rtango::cmd::own::exec(root, target, Some("does-not-exist".into()), false).unwrap_err();
    assert!(
        err.to_string().contains("does-not-exist"),
        "err: {}",
        err
    );
}

#[test]
fn own_replaces_existing_decision() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    write_spec(root, &two_set_spec());

    let target = root.join(".github/skills/foo/SKILL.md");
    let mut lock = load_lock_or_empty(root).unwrap();
    lock.owners.push(Ownership {
        path: target.clone(),
        rule_id: "a".into(),
    });
    save_lock(root, &lock).unwrap();

    rtango::cmd::own::exec(root, target.clone(), Some("b".into()), false).unwrap();

    let lock = load_lock(root).unwrap();
    assert_eq!(lock.owners.len(), 1, "expected replace, not duplicate");
    assert_eq!(lock.owners[0].rule_id, "b");
}

#[test]
fn own_clear_removes_entry() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    write_spec(root, &two_set_spec());

    let target = root.join(".github/skills/foo/SKILL.md");
    let mut lock = load_lock_or_empty(root).unwrap();
    lock.owners.push(Ownership {
        path: target.clone(),
        rule_id: "a".into(),
    });
    save_lock(root, &lock).unwrap();

    rtango::cmd::own::exec(root, target.clone(), None, true).unwrap();

    let lock = load_lock(root).unwrap();
    assert!(lock.owners.is_empty());
}

#[test]
fn own_clear_on_missing_entry_is_noop() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    write_spec(root, &two_set_spec());

    let target = root.join(".github/skills/foo/SKILL.md");
    // No lock yet — clear should succeed silently and initialize an empty lock.
    rtango::cmd::own::exec(root, target, None, true).unwrap();

    let lock = load_lock_or_empty(root).unwrap();
    assert!(lock.owners.is_empty());
}

#[test]
fn own_set_accepts_relative_path() {
    // User can pass a path relative to `root`; it's stored as an absolute
    // path so the lock stays stable under the `root.join(...)` keys used by
    // resolve_owners.
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    write_spec(root, &two_set_spec());

    let rel = PathBuf::from(".github/skills/foo/SKILL.md");
    rtango::cmd::own::exec(root, rel.clone(), Some("a".into()), false).unwrap();

    let lock = load_lock(root).unwrap();
    assert_eq!(lock.owners.len(), 1);
    assert_eq!(lock.owners[0].path, root.join(&rel));
}

#[test]
fn own_requires_rule_or_clear() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    write_spec(root, &two_set_spec());

    let target = root.join(".github/skills/foo/SKILL.md");
    let err = rtango::cmd::own::exec(root, target, None, false).unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("rule"),
        "err: {}",
        err
    );
    let _ = Lock {
        version: 1,
        tracked_agents: vec![],
        owners: vec![],
        deployments: vec![],
    };
}
