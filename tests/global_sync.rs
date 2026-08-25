use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use rtango::engine::GlobalEnvironment;

fn write_spec(root: &Path, rules: &str) -> PathBuf {
    let path = root.join("global.yaml");
    fs::write(
        &path,
        format!("version: 1\ndefaults:\n  gitignore_targets: true\nrules:\n{rules}"),
    )
    .unwrap();
    path
}

fn skill_rule(id: &str, source: &str) -> String {
    format!("  - id: {id}\n    source: {source}\n    schema_agent: claude-code\n    kind: skill\n")
}

#[test]
fn global_sync_resolves_sources_beside_spec_and_writes_native_global_paths() {
    let spec_root = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let skill_dir = spec_root.path().join("skills/review");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: review\ndescription: Review changes\nallowed-tools: Read Bash\n---\nReview carefully.\n",
    )
    .unwrap();
    fs::write(spec_root.path().join("instructions.md"), "# Shared rules\n").unwrap();

    let mut rules = skill_rule("review", "skills/review");
    rules.push_str(
        "  - id: instructions\n    source: instructions.md\n    schema_agent: plain\n    kind: system\n",
    );
    let spec = write_spec(spec_root.path(), &rules);

    rtango::cmd::global_sync::exec_at(
        &spec,
        home.path(),
        vec![
            "claude-code".into(),
            "codex".into(),
            "copilot".into(),
            "opencode".into(),
            "pi".into(),
        ],
        None,
        false,
        false,
        false,
    )
    .unwrap();

    for path in [
        ".claude/skills/review/SKILL.md",
        ".agents/skills/review/SKILL.md",
        ".copilot/skills/review/SKILL.md",
        ".config/opencode/skills/review/SKILL.md",
        ".pi/agent/skills/review/SKILL.md",
    ] {
        assert!(home.path().join(path).is_file(), "missing {path}");
    }
    assert_eq!(
        fs::read_to_string(home.path().join(".claude/CLAUDE.md")).unwrap(),
        "# Shared rules\n"
    );
    assert_eq!(
        fs::read_to_string(home.path().join(".codex/AGENTS.md")).unwrap(),
        "# Shared rules\n"
    );
    assert!(!spec_root.path().join(".gitignore").exists());
    assert!(spec.with_extension("lock.yaml").is_file());
}

#[test]
fn global_sync_honors_an_injected_codex_profile_home() {
    let spec_root = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let codex_profile = TempDir::new().unwrap();
    let skill_dir = spec_root.path().join("x");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: x\ndescription: X\n---\nbody\n",
    )
    .unwrap();
    fs::write(spec_root.path().join("instructions.md"), "profile rules\n").unwrap();
    let mut rules = skill_rule("x", "x");
    rules.push_str(
        "  - id: instructions\n    source: instructions.md\n    schema_agent: plain\n    kind: system\n",
    );
    let spec = write_spec(spec_root.path(), &rules);
    let mut environment = GlobalEnvironment::for_home(home.path());
    environment.codex_home = codex_profile.path().to_path_buf();

    rtango::cmd::global_sync::exec_at_with_environment(
        &spec,
        environment,
        vec!["codex".into()],
        None,
        false,
        false,
        false,
    )
    .unwrap();

    assert!(home.path().join(".agents/skills/x/SKILL.md").is_file());
    assert_eq!(
        fs::read_to_string(codex_profile.path().join("AGENTS.md")).unwrap(),
        "profile rules\n"
    );
    assert!(!home.path().join(".codex/AGENTS.md").exists());
}

#[test]
fn global_sync_uses_spec_agents_and_rule_level_profiles() {
    let spec_root = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let profile_a = TempDir::new().unwrap();
    let profile_b = TempDir::new().unwrap();
    for name in ["default", "profiles"] {
        let skill_dir = spec_root.path().join(name);
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: {name}\n---\nbody\n"),
        )
        .unwrap();
    }
    let spec = spec_root.path().join("global.yaml");
    fs::write(
        &spec,
        format!(
            "version: 1\nagents: [claude-code, codex]\nrules:\n{}  - id: profiles\n    source: profiles\n    schema_agent: claude-code\n    kind: skill\n    targets:\n      - agent: codex\n        home: {}\n      - agent: codex\n        home: {}\n",
            skill_rule("default", "default"),
            profile_a.path().display(),
            profile_b.path().display()
        ),
    )
    .unwrap();

    rtango::cmd::global_sync::exec_at(&spec, home.path(), Vec::new(), None, false, false, false)
        .unwrap();

    assert!(home.path().join(".claude/skills/default/SKILL.md").exists());
    assert!(home.path().join(".agents/skills/default/SKILL.md").exists());
    assert!(profile_a.path().join("skills/profiles/SKILL.md").exists());
    assert!(profile_b.path().join("skills/profiles/SKILL.md").exists());
    assert!(!profile_a.path().join("skills/default/SKILL.md").exists());
}

#[test]
fn global_sync_uses_lock_for_conflicts_and_force_overwrites() {
    let spec_root = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let skill_dir = spec_root.path().join("x");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: x\ndescription: X\n---\nbody\n",
    )
    .unwrap();
    let spec = write_spec(spec_root.path(), &skill_rule("x", "x"));

    rtango::cmd::global_sync::exec_at(
        &spec,
        home.path(),
        vec!["claude-code".into()],
        None,
        false,
        false,
        false,
    )
    .unwrap();
    let target = home.path().join(".claude/skills/x/SKILL.md");
    fs::write(&target, "manual edit\n").unwrap();

    assert!(
        rtango::cmd::global_sync::exec_at(
            &spec,
            home.path(),
            vec!["claude-code".into()],
            None,
            false,
            false,
            false,
        )
        .is_err()
    );
    rtango::cmd::global_sync::exec_at(
        &spec,
        home.path(),
        vec!["claude-code".into()],
        None,
        false,
        true,
        false,
    )
    .unwrap();
    assert!(fs::read_to_string(target).unwrap().contains("body"));
}

#[test]
fn global_sync_does_not_prune_without_explicit_flag() {
    let spec_root = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let skill_dir = spec_root.path().join("x");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: x\ndescription: X\n---\nbody\n",
    )
    .unwrap();
    let spec = write_spec(spec_root.path(), &skill_rule("x", "x"));

    rtango::cmd::global_sync::exec_at(
        &spec,
        home.path(),
        vec!["claude-code".into()],
        None,
        false,
        false,
        false,
    )
    .unwrap();
    let target = home.path().join(".claude/skills/x/SKILL.md");
    let lock = spec.with_extension("lock.yaml");
    fs::write(&spec, "version: 1\nrules: []\n").unwrap();

    rtango::cmd::global_sync::exec_at(
        &spec,
        home.path(),
        vec!["claude-code".into()],
        None,
        false,
        false,
        false,
    )
    .unwrap();
    assert!(target.exists());
    assert!(lock.exists());

    rtango::cmd::global_sync::exec_at(
        &spec,
        home.path(),
        vec!["claude-code".into()],
        None,
        false,
        false,
        true,
    )
    .unwrap();
    assert!(!target.exists());
}

#[test]
fn cursor_global_skills_work_but_system_files_are_rejected() {
    let spec_root = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let skill_dir = spec_root.path().join("x");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: x\ndescription: X\n---\nbody\n",
    )
    .unwrap();
    let spec = write_spec(spec_root.path(), &skill_rule("x", "x"));

    rtango::cmd::global_sync::exec_at(
        &spec,
        home.path(),
        vec!["cursor".into()],
        None,
        false,
        false,
        false,
    )
    .unwrap();
    assert!(home.path().join(".cursor/skills/x/SKILL.md").exists());

    fs::write(spec_root.path().join("system.md"), "rules\n").unwrap();
    let system_spec = write_spec(
        spec_root.path(),
        "  - id: system\n    source: system.md\n    schema_agent: plain\n    kind: system\n",
    );
    assert!(
        rtango::cmd::global_sync::exec_at(
            &system_spec,
            home.path(),
            vec!["cursor".into()],
            None,
            false,
            false,
            false,
        )
        .is_err()
    );
}
