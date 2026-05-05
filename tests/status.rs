use std::fs;
use std::path::PathBuf;

use tempfile::TempDir;

use rtango::engine::{DeploymentStatus, compute_plan, execute_plan};
use rtango::spec::{
    AgentName, Defaults, Deployment, Lock, OnTargetModified, Rule, RuleKind, Source, Spec,
};

// ── Helpers ──────────────────────────────────────────────────────────

fn setup_copilot_skill(root: &std::path::Path, name: &str, body: &str) {
    let dir = root.join(format!(".github/skills/{}", name));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("SKILL.md"), body).unwrap();
}

fn empty_lock() -> Lock {
    Lock {
        version: 1,
        tracked_agents: vec![],
        owners: vec![],
        deployments: vec![],
    }
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
        kind: RuleKind::skill_set(),
    }
}

fn write_spec_and_lock(root: &std::path::Path, spec: &Spec, lock: &Lock) {
    let spec_yaml = serde_yml::to_string(spec).unwrap();
    let lock_yaml = serde_yml::to_string(lock).unwrap();
    fs::create_dir_all(root.join(".rtango")).unwrap();
    fs::write(root.join(".rtango/spec.yaml"), spec_yaml).unwrap();
    fs::write(root.join(".rtango/lock.yaml"), lock_yaml).unwrap();
}

// ── Tests ────────────────────────────────────────────────────────────

#[test]
fn status_clean_all_up_to_date() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_skill(root, "my-skill", "Skill body");

    let spec = make_spec(
        vec!["claude-code"],
        vec![skill_set_rule("skills", ".github/skills", "copilot")],
    );
    let lock = empty_lock();

    // Execute to get everything synced
    let plan = compute_plan(root, &spec, &lock, false, false).unwrap();
    let new_lock = execute_plan(root, &plan, &lock, false).unwrap();

    // Now compute plan again — should be clean
    let plan2 = compute_plan(root, &spec, &new_lock, false, false).unwrap();
    assert!(plan2.is_clean());
    assert_eq!(plan2.items.len(), 1);
    assert_eq!(plan2.items[0].status, DeploymentStatus::UpToDate);

    // Also verify via the cmd::status path (loads from disk)
    write_spec_and_lock(root, &spec, &new_lock);
    let result = rtango::cmd::status::exec(root, None, false);
    assert!(result.is_ok());
}

#[test]
fn status_with_creates() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_skill(root, "alpha", "Alpha body");
    setup_copilot_skill(root, "beta", "Beta body");

    let spec = make_spec(
        vec!["claude-code"],
        vec![skill_set_rule("skills", ".github/skills", "copilot")],
    );
    let lock = empty_lock();

    let plan = compute_plan(root, &spec, &lock, false, false).unwrap();
    assert_eq!(plan.items.len(), 2);
    assert!(
        plan.items
            .iter()
            .all(|i| i.status == DeploymentStatus::Create)
    );

    // Verify exec works
    write_spec_and_lock(root, &spec, &lock);
    let result = rtango::cmd::status::exec(root, None, false);
    assert!(result.is_ok());
}

#[test]
fn status_rule_filter() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_skill(root, "alpha", "Alpha body");

    let spec = make_spec(
        vec!["claude-code"],
        vec![skill_set_rule("skills", ".github/skills", "copilot")],
    );
    let lock = empty_lock();

    // Filter by a rule that exists
    write_spec_and_lock(root, &spec, &lock);
    let result = rtango::cmd::status::exec(root, Some("skills".to_string()), false);
    assert!(result.is_ok());

    // Filter by a rule that doesn't exist — should still succeed, just no items
    let result = rtango::cmd::status::exec(root, Some("nonexistent".to_string()), false);
    assert!(result.is_ok());
}

#[test]
fn status_verbose_shows_up_to_date() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_skill(root, "my-skill", "Skill body");

    let spec = make_spec(
        vec!["claude-code"],
        vec![skill_set_rule("skills", ".github/skills", "copilot")],
    );
    let lock = empty_lock();

    // Sync first
    let plan = compute_plan(root, &spec, &lock, false, false).unwrap();
    let new_lock = execute_plan(root, &plan, &lock, false).unwrap();

    // Verbose status should succeed and include up-to-date items
    write_spec_and_lock(root, &spec, &new_lock);
    let result = rtango::cmd::status::exec(root, None, true);
    assert!(result.is_ok());
}
