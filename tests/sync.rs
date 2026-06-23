use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use rtango::spec::io::load_lock;
use rtango::spec::{AgentName, Defaults, Rule, RuleKind, Source, Spec};

// ── Helpers ──────────────────────────────────────────────────────────

fn setup_copilot_skill(root: &Path, name: &str, body: &str) {
    let dir = root.join(format!(".github/skills/{}", name));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("SKILL.md"), body).unwrap();
}

fn setup_copilot_skill_asset(root: &Path, name: &str, relative_path: &str, body: &str) {
    let path = root.join(format!(".github/skills/{}/{}", name, relative_path));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, body).unwrap();
}

fn write_spec(root: &Path, spec: &Spec) {
    let yaml = serde_yml::to_string(spec).unwrap();
    fs::create_dir_all(root.join(".rtango")).unwrap();
    fs::write(root.join(".rtango/spec.yaml"), yaml).unwrap();
}

fn make_spec(agents: Vec<&str>, rules: Vec<Rule>) -> Spec {
    make_spec_with_defaults(agents, Defaults::default(), rules)
}

fn make_spec_with_defaults(agents: Vec<&str>, defaults: Defaults, rules: Vec<Rule>) -> Spec {
    Spec {
        version: 1,
        agents: agents.into_iter().map(AgentName::new).collect(),
        defaults,
        rules,
    }
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

fn single_skill_rule(id: &str, path: &str, schema: &str) -> Rule {
    Rule {
        id: id.to_string(),
        source: Source::Local(PathBuf::from(path)),
        schema_agent: AgentName::new(schema),
        on_target_modified: None,
        kind: RuleKind::skill(),
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[test]
fn basic_sync_creates_files_and_updates_lock() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    setup_copilot_skill(root, "deploy", "Deploy instructions");

    let spec = make_spec(
        vec!["claude-code"],
        vec![skill_set_rule("skills", ".github/skills", "copilot")],
    );
    write_spec(root, &spec);

    // No lock file yet
    let result = rtango::cmd::sync::exec(root, false, false, None, false);
    assert!(result.is_ok(), "sync failed: {:?}", result.err());

    // Verify target file was created
    let target = root.join(".claude/skills/deploy/SKILL.md");
    assert!(target.exists());
    let content = fs::read_to_string(&target).unwrap();
    assert!(content.contains("Deploy instructions"));

    // Verify lock was written
    let lock = load_lock(root).unwrap();
    assert_eq!(lock.deployments.len(), 1);
    assert_eq!(lock.deployments[0].rule_id, "skills");
    assert_eq!(lock.deployments[0].agent, AgentName::new("claude-code"));
    assert_eq!(lock.tracked_agents, vec![AgentName::new("claude-code")]);
}

#[test]
fn sync_materializes_multi_file_skill_assets_and_gitignores_skill_dir() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    setup_copilot_skill(root, "deploy", "Deploy instructions");
    setup_copilot_skill_asset(root, "deploy", "REFERENCE.md", "# Reference\n");
    setup_copilot_skill_asset(root, "deploy", "templates/example.txt", "template\n");
    setup_copilot_skill_asset(root, "deploy", ".gitignore", ".env\n");
    setup_copilot_skill_asset(root, "deploy", ".env", "SECRET=1\n");

    let spec = make_spec_with_defaults(
        vec!["pi"],
        Defaults {
            on_target_modified: Default::default(),
            gitignore_targets: true,
        },
        vec![single_skill_rule(
            "deploy",
            ".github/skills/deploy",
            "copilot",
        )],
    );
    write_spec(root, &spec);

    rtango::cmd::sync::exec(root, false, false, None, false).unwrap();

    assert_eq!(
        fs::read_to_string(root.join(".pi/skills/deploy/SKILL.md")).unwrap(),
        "Deploy instructions"
    );
    assert_eq!(
        fs::read_to_string(root.join(".pi/skills/deploy/REFERENCE.md")).unwrap(),
        "# Reference\n"
    );
    assert_eq!(
        fs::read_to_string(root.join(".pi/skills/deploy/templates/example.txt")).unwrap(),
        "template\n"
    );
    assert_eq!(
        fs::read_to_string(root.join(".pi/skills/deploy/.gitignore")).unwrap(),
        ".env\n"
    );
    assert!(!root.join(".pi/skills/deploy/.env").exists());

    let gitignore = fs::read_to_string(root.join(".gitignore")).unwrap();
    assert!(gitignore.contains(".pi/skills/deploy/"));
    assert!(!gitignore.contains("REFERENCE.md"));
    assert!(!gitignore.contains("templates/example.txt"));

    let lock = load_lock(root).unwrap();
    assert_eq!(lock.deployments.len(), 4);
}

#[test]
fn check_mode_does_not_write_files_and_errors_when_not_clean() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    setup_copilot_skill(root, "deploy", "Deploy instructions");

    let spec = make_spec(
        vec!["claude-code"],
        vec![skill_set_rule("skills", ".github/skills", "copilot")],
    );
    write_spec(root, &spec);

    let result = rtango::cmd::sync::exec(root, true, false, None, false);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not in sync"));

    // Verify no files were created
    let target = root.join(".claude/skills/deploy/SKILL.md");
    assert!(!target.exists());

    // Verify no lock was written
    assert!(!root.join(".rtango/lock.yaml").exists());
}

#[test]
fn check_mode_returns_ok_when_already_synced() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    setup_copilot_skill(root, "deploy", "Deploy instructions");

    let spec = make_spec(
        vec!["claude-code"],
        vec![skill_set_rule("skills", ".github/skills", "copilot")],
    );
    write_spec(root, &spec);

    // First, do a real sync
    rtango::cmd::sync::exec(root, false, false, None, false).unwrap();

    // Now check mode should succeed
    let result = rtango::cmd::sync::exec(root, true, false, None, false);
    assert!(
        result.is_ok(),
        "check mode failed when synced: {:?}",
        result.err()
    );
}

#[test]
fn force_mode_resolves_conflicts() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    setup_copilot_skill(root, "deploy", "Deploy instructions");

    let spec = make_spec(
        vec!["claude-code"],
        vec![skill_set_rule("skills", ".github/skills", "copilot")],
    );
    write_spec(root, &spec);

    // Pre-create the target file to cause a conflict (no lock entry + file exists)
    let target_dir = root.join(".claude/skills/deploy");
    fs::create_dir_all(&target_dir).unwrap();
    fs::write(target_dir.join("SKILL.md"), "existing content").unwrap();

    // Without force, should fail due to conflict
    let result = rtango::cmd::sync::exec(root, false, false, None, false);
    assert!(result.is_err());

    // With force, should succeed
    let result = rtango::cmd::sync::exec(root, false, true, None, false);
    assert!(result.is_ok(), "force sync failed: {:?}", result.err());

    // Verify the file was overwritten with rendered content
    let content = fs::read_to_string(root.join(".claude/skills/deploy/SKILL.md")).unwrap();
    assert!(content.contains("Deploy instructions"));
    assert!(!content.contains("existing content"));
}

#[test]
fn rule_filter_only_processes_matching_rule() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    setup_copilot_skill(root, "alpha", "Alpha body");
    setup_copilot_skill(root, "beta", "Beta body");

    let spec = make_spec(
        vec!["claude-code"],
        vec![
            single_skill_rule("rule-alpha", ".github/skills/alpha", "copilot"),
            single_skill_rule("rule-beta", ".github/skills/beta", "copilot"),
        ],
    );
    write_spec(root, &spec);

    // First, sync everything
    rtango::cmd::sync::exec(root, false, false, None, false).unwrap();

    let lock = load_lock(root).unwrap();
    assert_eq!(lock.deployments.len(), 2);

    // Now modify alpha source
    setup_copilot_skill(root, "alpha", "Alpha updated");

    // Sync only rule-alpha
    rtango::cmd::sync::exec(root, false, false, Some("rule-alpha".into()), false).unwrap();

    // Verify alpha was updated
    let alpha_content = fs::read_to_string(root.join(".claude/skills/alpha/SKILL.md")).unwrap();
    assert!(alpha_content.contains("Alpha updated"));

    // Verify lock still has both entries
    let lock = load_lock(root).unwrap();
    assert_eq!(lock.deployments.len(), 2);

    let has_alpha = lock.deployments.iter().any(|d| d.rule_id == "rule-alpha");
    let has_beta = lock.deployments.iter().any(|d| d.rule_id == "rule-beta");
    assert!(has_alpha, "lock should contain rule-alpha");
    assert!(has_beta, "lock should contain rule-beta");
}

fn system_rule(id: &str, path: &str, schema: &str) -> Rule {
    Rule {
        id: id.to_string(),
        source: Source::Local(PathBuf::from(path)),
        schema_agent: AgentName::new(schema),
        on_target_modified: None,
        kind: RuleKind::System,
    }
}

#[test]
fn system_file_syncs_per_agent_convention_paths() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Source is a single markdown file with no frontmatter.
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::write(
        root.join("docs/INSTRUCTIONS.md"),
        "# House rules\n\nBe terse.\n",
    )
    .unwrap();

    let spec = make_spec(
        vec!["claude-code", "codex", "copilot"],
        vec![system_rule(
            "instructions",
            "docs/INSTRUCTIONS.md",
            "claude-code",
        )],
    );
    write_spec(root, &spec);

    rtango::cmd::sync::exec(root, false, false, None, false).unwrap();

    // Each agent gets its own convention path with verbatim content.
    let claude = fs::read_to_string(root.join("CLAUDE.md")).unwrap();
    let codex = fs::read_to_string(root.join("AGENTS.md")).unwrap();
    let copilot = fs::read_to_string(root.join(".github/copilot-instructions.md")).unwrap();

    let expected = "# House rules\n\nBe terse.\n";
    assert_eq!(claude, expected);
    assert_eq!(codex, expected);
    assert_eq!(copilot, expected);

    // No frontmatter was injected.
    assert!(!claude.starts_with("---"));
    assert!(!codex.starts_with("---"));
    assert!(!copilot.starts_with("---"));

    let lock = load_lock(root).unwrap();
    assert_eq!(lock.deployments.len(), 3);
}

#[test]
fn system_file_handles_agents_sharing_target_path() {
    // codex and opencode both write to AGENTS.md — must not conflict.
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    fs::write(root.join("source.md"), "shared body\n").unwrap();
    let spec = make_spec(
        vec!["codex", "opencode"],
        vec![system_rule("sys", "source.md", "codex")],
    );
    write_spec(root, &spec);

    rtango::cmd::sync::exec(root, false, false, None, false).unwrap();
    assert_eq!(
        fs::read_to_string(root.join("AGENTS.md")).unwrap(),
        "shared body\n"
    );

    // Re-sync stays clean despite two lock entries pointing at the same path.
    rtango::cmd::sync::exec(root, true, false, None, false).unwrap();

    let lock = load_lock(root).unwrap();
    assert_eq!(lock.deployments.len(), 2);
}

#[test]
fn system_file_resync_is_clean_after_first_sync() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    fs::write(root.join("source.md"), "body\n").unwrap();
    let spec = make_spec(
        vec!["claude-code"],
        vec![system_rule("sys", "source.md", "claude-code")],
    );
    write_spec(root, &spec);

    rtango::cmd::sync::exec(root, false, false, None, false).unwrap();
    // Re-running in --check mode must succeed (idempotent).
    rtango::cmd::sync::exec(root, true, false, None, false).unwrap();
}

#[test]
fn adopt_mode_adopts_existing_files() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    setup_copilot_skill(root, "deploy", "Deploy instructions");

    let spec = make_spec(
        vec!["claude-code"],
        vec![skill_set_rule("skills", ".github/skills", "copilot")],
    );
    write_spec(root, &spec);

    // Pre-create the target file (untracked — no lock entry)
    let target_dir = root.join(".claude/skills/deploy");
    fs::create_dir_all(&target_dir).unwrap();
    fs::write(target_dir.join("SKILL.md"), "existing content").unwrap();

    // Without adopt or force, should fail (conflict)
    let result = rtango::cmd::sync::exec(root, false, false, None, false);
    assert!(result.is_err());

    // With adopt, should succeed
    let result = rtango::cmd::sync::exec(root, false, false, None, true);
    assert!(result.is_ok(), "adopt sync failed: {:?}", result.err());

    // Verify the file was overwritten
    let content = fs::read_to_string(root.join(".claude/skills/deploy/SKILL.md")).unwrap();
    assert!(content.contains("Deploy instructions"));

    // Verify lock was created
    let lock = load_lock(root).unwrap();
    assert_eq!(lock.deployments.len(), 1);
}

#[test]
fn sync_updates_gitignore_precisely_when_enabled() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    setup_copilot_skill(root, "deploy", "Deploy instructions");

    let spec = make_spec_with_defaults(
        vec!["copilot", "claude-code"],
        Defaults {
            gitignore_targets: true,
            ..Defaults::default()
        },
        vec![single_skill_rule(
            "deploy",
            ".github/skills/deploy",
            "copilot",
        )],
    );
    write_spec(root, &spec);

    rtango::cmd::sync::exec(root, false, false, None, false).unwrap();

    let gitignore = fs::read_to_string(root.join(".gitignore")).unwrap();
    assert!(gitignore.contains("# >>> rtango managed targets >>>"));
    assert!(gitignore.contains(".claude/skills/deploy/"));
    assert!(gitignore.contains(".claude/skills/rtango/"));
    assert!(gitignore.contains(".github/skills/rtango/"));
    assert!(!gitignore.contains(".claude/\n"));
    assert!(!gitignore.contains(".claude/skills/\n"));
    assert!(!gitignore.contains(".github/skills/deploy/"));
}

#[test]
fn check_mode_fails_when_managed_gitignore_is_out_of_date() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    setup_copilot_skill(root, "deploy", "Deploy instructions");

    let spec = make_spec_with_defaults(
        vec!["copilot", "claude-code"],
        Defaults {
            gitignore_targets: true,
            ..Defaults::default()
        },
        vec![single_skill_rule(
            "deploy",
            ".github/skills/deploy",
            "copilot",
        )],
    );
    write_spec(root, &spec);

    rtango::cmd::sync::exec(root, false, false, None, false).unwrap();
    fs::write(root.join(".gitignore"), "").unwrap();

    let err = rtango::cmd::sync::exec(root, true, false, None, false).unwrap_err();
    assert!(err.to_string().contains("not in sync"));
}

#[test]
fn rule_filtered_check_ignores_unrelated_gitignore_changes() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    setup_copilot_skill(root, "alpha", "Alpha body");

    let initial_spec = make_spec_with_defaults(
        vec!["copilot", "claude-code"],
        Defaults {
            gitignore_targets: true,
            ..Defaults::default()
        },
        vec![single_skill_rule(
            "rule-alpha",
            ".github/skills/alpha",
            "copilot",
        )],
    );
    write_spec(root, &initial_spec);
    rtango::cmd::sync::exec(root, false, false, None, false).unwrap();

    setup_copilot_skill(root, "beta", "Beta body");
    let updated_spec = make_spec_with_defaults(
        vec!["copilot", "claude-code"],
        Defaults {
            gitignore_targets: true,
            ..Defaults::default()
        },
        vec![
            single_skill_rule("rule-alpha", ".github/skills/alpha", "copilot"),
            single_skill_rule("rule-beta", ".github/skills/beta", "copilot"),
        ],
    );
    write_spec(root, &updated_spec);

    rtango::cmd::sync::exec(root, true, false, Some("rule-alpha".into()), false).unwrap();
}

#[test]
fn rule_filtered_sync_does_not_write_gitignore_entries_for_other_rules() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    setup_copilot_skill(root, "alpha", "Alpha body");

    let initial_spec = make_spec_with_defaults(
        vec!["copilot", "claude-code"],
        Defaults {
            gitignore_targets: true,
            ..Defaults::default()
        },
        vec![single_skill_rule(
            "rule-alpha",
            ".github/skills/alpha",
            "copilot",
        )],
    );
    write_spec(root, &initial_spec);
    rtango::cmd::sync::exec(root, false, false, None, false).unwrap();

    let gitignore_before = fs::read_to_string(root.join(".gitignore")).unwrap();

    setup_copilot_skill(root, "beta", "Beta body");
    let updated_spec = make_spec_with_defaults(
        vec!["copilot", "claude-code"],
        Defaults {
            gitignore_targets: true,
            ..Defaults::default()
        },
        vec![
            single_skill_rule("rule-alpha", ".github/skills/alpha", "copilot"),
            single_skill_rule("rule-beta", ".github/skills/beta", "copilot"),
        ],
    );
    write_spec(root, &updated_spec);

    rtango::cmd::sync::exec(root, false, false, Some("rule-alpha".into()), false).unwrap();

    let gitignore_after = fs::read_to_string(root.join(".gitignore")).unwrap();
    assert_eq!(gitignore_after, gitignore_before);
    assert!(!root.join(".claude/skills/beta/SKILL.md").exists());
}
