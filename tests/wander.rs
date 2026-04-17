use std::fs;
use std::path::Path;

use rtango::cmd::wander;

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

#[test]
fn renders_for_extra_target_without_creating_rtango_dir() {
    let tmp = tempfile::tempdir().unwrap();
    write_file(
        &tmp.path().join(".claude/skills/foo/SKILL.md"),
        "---\nname: foo\ndescription: d\n---\nbody",
    );

    wander::exec(tmp.path(), vec!["copilot".into()]).unwrap();

    assert!(
        tmp.path().join(".github/skills/foo/SKILL.md").exists(),
        "copilot target should be rendered"
    );
    assert!(
        !tmp.path().join(".rtango").exists(),
        ".rtango/ should never be created"
    );
}

#[test]
fn second_run_is_idempotent() {
    let tmp = tempfile::tempdir().unwrap();
    write_file(
        &tmp.path().join(".claude/skills/foo/SKILL.md"),
        "---\nname: foo\ndescription: d\n---\nbody",
    );

    wander::exec(tmp.path(), vec!["copilot".into()]).unwrap();
    wander::exec(tmp.path(), vec!["copilot".into()]).unwrap();

    assert!(tmp.path().join(".github/skills/foo/SKILL.md").exists());
    assert!(!tmp.path().join(".rtango").exists());
}

#[test]
fn empty_target_list_is_noop_when_only_self_writes() {
    let tmp = tempfile::tempdir().unwrap();
    write_file(
        &tmp.path().join(".claude/skills/foo/SKILL.md"),
        "---\nname: foo\ndescription: d\n---\nbody",
    );

    wander::exec(tmp.path(), vec![]).unwrap();

    assert!(!tmp.path().join(".rtango").exists());
    assert!(!tmp.path().join(".github/skills/foo/SKILL.md").exists());
}

#[test]
fn fails_if_no_agents_detected() {
    let tmp = tempfile::tempdir().unwrap();
    let result = wander::exec(tmp.path(), vec!["copilot".into()]);
    assert!(result.is_err());
}
