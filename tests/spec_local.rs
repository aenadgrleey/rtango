use std::fs;
use std::path::{Path, PathBuf};

use rtango::cmd::add::AddOptions;
use rtango::spec::io::{load_main_spec, load_spec};
use rtango::spec::{AgentName, OnTargetModified, RuleKind, Source};

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

fn write_main_spec(root: &Path) {
    write_file(
        &root.join(".rtango/spec.yaml"),
        r#"version: 1
agents:
  - claude-code
defaults:
  on_target_modified: fail
  gitignore_targets: false
rules:
  - id: keep
    source: main/keep.md
    schema_agent: claude-code
    kind: skill
  - id: replace
    source: main/replace.md
    schema_agent: claude-code
    kind: skill
  - id: remove
    source: main/remove.md
    schema_agent: claude-code
    kind: skill
"#,
    );
}

#[test]
fn local_spec_overrides_fields_replaces_rules_and_excludes_main_rules() {
    let tmp = tempfile::tempdir().unwrap();
    write_main_spec(tmp.path());
    write_file(
        &tmp.path().join(".rtango/spec.local.yaml"),
        r#"version: 1
agents:
  - cursor
defaults:
  on_target_modified: overwrite
exclude:
  - remove
rules:
  - id: replace
    source: local/replace.md
    schema_agent: cursor
    kind: skill
  - id: local-only
    source: local/only.md
    schema_agent: cursor
    kind: system
"#,
    );

    let spec = load_spec(tmp.path()).unwrap();
    assert_eq!(spec.agents, vec![AgentName::new("cursor")]);
    assert_eq!(
        spec.defaults.on_target_modified,
        OnTargetModified::Overwrite
    );
    assert!(!spec.defaults.gitignore_targets);
    assert_eq!(
        spec.rules
            .iter()
            .map(|rule| rule.id.as_str())
            .collect::<Vec<_>>(),
        vec!["keep", "replace", "local-only"]
    );
    match &spec.rules[1].source {
        Source::Local(path) => assert_eq!(path, &PathBuf::from("local/replace.md")),
        Source::Github(_) => panic!("expected local source"),
    }
    assert_eq!(spec.rules[1].schema_agent, AgentName::new("cursor"));
    assert!(matches!(spec.rules[2].kind, RuleKind::System));
}

#[test]
fn local_defaults_only_override_fields_that_are_present() {
    let tmp = tempfile::tempdir().unwrap();
    write_file(
        &tmp.path().join(".rtango/spec.yaml"),
        r#"version: 1
agents: [claude-code]
defaults:
  on_target_modified: skip
  gitignore_targets: true
rules: []
"#,
    );
    write_file(
        &tmp.path().join(".rtango/spec.local.yaml"),
        r#"version: 1
defaults:
  gitignore_targets: false
"#,
    );

    let spec = load_spec(tmp.path()).unwrap();
    assert_eq!(spec.defaults.on_target_modified, OnTargetModified::Skip);
    assert!(!spec.defaults.gitignore_targets);
}

#[test]
fn local_spec_rejects_unknown_exclusion() {
    let tmp = tempfile::tempdir().unwrap();
    write_main_spec(tmp.path());
    write_file(
        &tmp.path().join(".rtango/spec.local.yaml"),
        "version: 1\nexclude: [missing]\n",
    );

    let error = format!("{:#}", load_spec(tmp.path()).unwrap_err());
    assert!(error.contains("local spec excludes unknown main rule 'missing'"));
}

#[test]
fn local_spec_rejects_excluding_and_overriding_the_same_rule() {
    let tmp = tempfile::tempdir().unwrap();
    write_main_spec(tmp.path());
    write_file(
        &tmp.path().join(".rtango/spec.local.yaml"),
        r#"version: 1
exclude: [replace]
rules:
  - id: replace
    source: local/replace.md
    schema_agent: claude-code
    kind: skill
"#,
    );

    let error = format!("{:#}", load_spec(tmp.path()).unwrap_err());
    assert!(error.contains("cannot be both excluded and overridden"));
}

#[test]
fn add_updates_only_main_spec_when_local_overlay_exists() {
    let tmp = tempfile::tempdir().unwrap();
    write_main_spec(tmp.path());
    let local_path = tmp.path().join(".rtango/spec.local.yaml");
    let local_content = "version: 1\nexclude: [remove]\n";
    write_file(&local_path, local_content);

    rtango::cmd::add::exec(
        tmp.path(),
        AddOptions {
            id: "added".into(),
            local: Some(PathBuf::from("main/added.md")),
            skill: true,
            ..AddOptions::default()
        },
    )
    .unwrap();

    assert_eq!(fs::read_to_string(local_path).unwrap(), local_content);
    let main = load_main_spec(tmp.path()).unwrap();
    assert_eq!(
        main.rules
            .iter()
            .map(|rule| rule.id.as_str())
            .collect::<Vec<_>>(),
        vec!["keep", "replace", "remove", "added"]
    );
    let effective = load_spec(tmp.path()).unwrap();
    assert_eq!(
        effective
            .rules
            .iter()
            .map(|rule| rule.id.as_str())
            .collect::<Vec<_>>(),
        vec!["keep", "replace", "added"]
    );
}

#[test]
fn sync_uses_local_rule_override() {
    let tmp = tempfile::tempdir().unwrap();
    write_file(
        &tmp.path().join(".rtango/spec.yaml"),
        r#"version: 1
agents: [cursor]
rules:
  - id: reviewer
    source: main/reviewer
    schema_agent: cursor
    kind: skill
"#,
    );
    write_file(
        &tmp.path().join(".rtango/spec.local.yaml"),
        r#"version: 1
rules:
  - id: reviewer
    source: local/reviewer
    schema_agent: cursor
    kind: skill
"#,
    );
    write_file(
        &tmp.path().join("main/reviewer/SKILL.md"),
        "---\nname: reviewer\ndescription: main\n---\nMain body.\n",
    );
    write_file(
        &tmp.path().join("local/reviewer/SKILL.md"),
        "---\nname: reviewer\ndescription: local\n---\nLocal body.\n",
    );

    rtango::cmd::sync::exec(tmp.path(), false, false, None, false).unwrap();

    let target = fs::read_to_string(tmp.path().join(".cursor/skills/reviewer/SKILL.md")).unwrap();
    assert!(target.contains("description: local"));
    assert!(target.contains("Local body."));
    assert!(!target.contains("Main body."));
}

#[test]
fn local_exclusion_removes_a_previously_synced_main_rule() {
    let tmp = tempfile::tempdir().unwrap();
    write_file(
        &tmp.path().join(".rtango/spec.yaml"),
        r#"version: 1
agents: [cursor]
rules:
  - id: reviewer
    source: skills/reviewer
    schema_agent: cursor
    kind: skill
"#,
    );
    write_file(
        &tmp.path().join("skills/reviewer/SKILL.md"),
        "---\nname: reviewer\ndescription: review\n---\nReview.\n",
    );

    rtango::cmd::sync::exec(tmp.path(), false, false, None, false).unwrap();
    let target = tmp.path().join(".cursor/skills/reviewer/SKILL.md");
    assert!(target.exists());

    write_file(
        &tmp.path().join(".rtango/spec.local.yaml"),
        "version: 1\nexclude: [reviewer]\n",
    );
    rtango::cmd::sync::exec(tmp.path(), false, false, None, false).unwrap();

    assert!(!target.exists());
    let lock = rtango::spec::io::load_lock(tmp.path()).unwrap();
    assert!(
        lock.deployments
            .iter()
            .all(|entry| entry.rule_id != "reviewer")
    );
}
