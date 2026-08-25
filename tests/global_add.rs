use std::fs;

use tempfile::TempDir;

use rtango::cmd::add::{AddOptions, exec_global_at};
use rtango::cmd::global_sync::exec_at;
use rtango::spec::Source;
use rtango::spec::io::{load_spec_file, spec_path};

#[test]
fn global_add_scaffolds_skill_and_records_multiple_codex_targets() {
    let home = TempDir::new().unwrap();
    let personal = home.path().join("codex-personal");
    let work = home.path().join("codex-work");

    exec_global_at(
        home.path(),
        AddOptions {
            id: "reviewer".into(),
            skill: true,
            ..Default::default()
        },
        vec![
            format!("codex={}", personal.display()),
            format!("codex={}", work.display()),
        ],
    )
    .unwrap();

    let path = spec_path(home.path());
    let spec = load_spec_file(&path).unwrap();
    assert_eq!(spec.agents.len(), 1);
    assert_eq!(spec.rules.len(), 1);
    let rule = &spec.rules[0];
    assert_eq!(rule.targets.as_ref().unwrap().len(), 2);
    match &rule.source {
        Source::Local(source) => assert!(home.path().join(".rtango").join(source).is_dir()),
        Source::Github(_) => panic!("scaffold should use a local source"),
    }

    exec_at(&path, home.path(), Vec::new(), None, false, false, false).unwrap();

    assert!(personal.join("skills/reviewer/SKILL.md").is_file());
    assert!(work.join("skills/reviewer/SKILL.md").is_file());
    assert!(
        fs::read_to_string(personal.join("skills/reviewer/SKILL.md"))
            .unwrap()
            .contains("name: reviewer")
    );
}

#[test]
fn global_add_scaffolds_agent_file() {
    let home = TempDir::new().unwrap();
    let profile = home.path().join("codex");

    exec_global_at(
        home.path(),
        AddOptions {
            id: "planner".into(),
            agent: true,
            ..Default::default()
        },
        vec![format!("codex={}", profile.display())],
    )
    .unwrap();

    let spec = load_spec_file(&spec_path(home.path())).unwrap();
    match &spec.rules[0].source {
        Source::Local(source) => assert!(home.path().join(".rtango").join(source).is_file()),
        Source::Github(_) => panic!("scaffold should use a local source"),
    }
    exec_at(
        &spec_path(home.path()),
        home.path(),
        Vec::new(),
        None,
        false,
        false,
        false,
    )
    .unwrap();
    assert!(profile.join("agents/planner.agent.md").is_file());
}
