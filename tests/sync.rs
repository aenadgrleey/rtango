use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use rtango::engine::hash_content;
use rtango::spec::{
    AgentName, Defaults, Deployment, Lock, OnTargetModified, Rule, RuleKind, Source, Spec,
};
use rtango::spec::io::{load_lock, save_lock};

// ── Helpers ──────────────────────────────────────────────────────────

fn setup_copilot_skill(root: &Path, name: &str, body: &str) {
    let dir = root.join(format!(".github/skills/{}", name));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("SKILL.md"), body).unwrap();
}

fn write_spec(root: &Path, spec: &Spec) {
    let yaml = serde_yml::to_string(spec).unwrap();
    fs::create_dir_all(root.join(".rtango")).unwrap();
    fs::write(root.join(".rtango/spec.yaml"), yaml).unwrap();
}

fn make_spec(agents: Vec<&str>, rules: Vec<Rule>) -> Spec {
    Spec {
        version: 1,
        agents: agents.into_iter().map(AgentName::new).collect(),
        defaults: Defaults::default(),
        rules,
    }
}

fn skill_set_rule(id: &str, path: &str, schema: &str) -> Rule {
    Rule {
        id: id.to_string(),
        source: Source::Local(PathBuf::from(path)),
        schema_agent: AgentName::new(schema),
        on_target_modified: None,
        kind: RuleKind::SkillSet {},
    }
}

fn single_skill_rule(id: &str, path: &str, schema: &str) -> Rule {
    Rule {
        id: id.to_string(),
        source: Source::Local(PathBuf::from(path)),
        schema_agent: AgentName::new(schema),
        on_target_modified: None,
        kind: RuleKind::Skill {},
    }
}

fn empty_lock() -> Lock {
    Lock {
        version: 1,
        tracked_agents: vec![],
        deployments: vec![],
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
    assert!(result.is_ok(), "check mode failed when synced: {:?}", result.err());
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
