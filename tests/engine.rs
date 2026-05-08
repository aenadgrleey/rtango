use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use rtango::engine::{
    DeploymentStatus, ExpandedKind, compute_plan, execute_plan, expand_rule,
    managed_gitignore_entries, render_for_agent,
};
use rtango::spec::{AgentName, Defaults, Lock, OnTargetModified, Rule, RuleKind, Source, Spec};

// ── Helpers ──────────────────────────────────────────────────────────

fn setup_copilot_skill(root: &std::path::Path, name: &str, body: &str) {
    let dir = root.join(format!(".github/skills/{}", name));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("SKILL.md"), body).unwrap();
}

fn setup_copilot_agent(root: &std::path::Path, name: &str, body: &str) {
    let dir = root.join(".github/agents");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(format!("{}.agent.md", name)), body).unwrap();
}

fn setup_claude_skill(root: &std::path::Path, name: &str, body: &str) {
    let dir = root.join(format!(".claude/skills/{}", name));
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

fn agent_set_rule(id: &str, path: &str, schema: &str) -> Rule {
    Rule {
        id: id.to_string(),
        source: Source::Local(PathBuf::from(path)),
        schema_agent: AgentName::new(schema),
        on_target_modified: None,
        kind: RuleKind::agent_set(),
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

fn single_agent_rule(id: &str, path: &str, schema: &str) -> Rule {
    Rule {
        id: id.to_string(),
        source: Source::Local(PathBuf::from(path)),
        schema_agent: AgentName::new(schema),
        on_target_modified: None,
        kind: RuleKind::agent(),
    }
}

// ── expand_rule tests ────────────────────────────────────────────────

#[test]
fn expand_skill_set_finds_all_skills() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_skill(root, "alpha", "Alpha body");
    setup_copilot_skill(root, "beta", "Beta body");

    let rule = skill_set_rule("skills", ".github/skills", "copilot");
    let items = expand_rule(root, &rule).unwrap();

    assert_eq!(items.len(), 2);
    let names: Vec<&str> = items
        .iter()
        .map(|i| match &i.kind {
            ExpandedKind::Skill(s) => s.name.as_str(),
            _ => panic!("expected skill"),
        })
        .collect();
    assert!(names.contains(&"alpha"));
    assert!(names.contains(&"beta"));
}

#[test]
fn expand_agent_set_finds_all_agents() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_agent(root, "reviewer", "Review stuff");
    setup_copilot_agent(root, "planner", "Plan stuff");

    let rule = agent_set_rule("agents", ".github/agents", "copilot");
    let items = expand_rule(root, &rule).unwrap();

    assert_eq!(items.len(), 2);
    let names: Vec<&str> = items
        .iter()
        .map(|i| match &i.kind {
            ExpandedKind::Agent(a) => a.name.as_str(),
            _ => panic!("expected agent"),
        })
        .collect();
    assert!(names.contains(&"reviewer"));
    assert!(names.contains(&"planner"));
}

#[test]
fn expand_single_skill() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_skill(root, "my-skill", "---\nname: My Skill\n---\nHello world");

    let rule = single_skill_rule("s1", ".github/skills/my-skill", "copilot");
    let items = expand_rule(root, &rule).unwrap();

    assert_eq!(items.len(), 1);
    match &items[0].kind {
        ExpandedKind::Skill(s) => {
            assert_eq!(s.name, "my-skill");
            assert_eq!(s.body, "Hello world");
        }
        _ => panic!("expected skill"),
    }
}

#[test]
fn expand_single_agent() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_agent(
        root,
        "helper",
        "---\nname: Helper\n---\nDoes helpful things",
    );

    let rule = single_agent_rule("a1", ".github/agents/helper.agent.md", "copilot");
    let items = expand_rule(root, &rule).unwrap();

    assert_eq!(items.len(), 1);
    match &items[0].kind {
        ExpandedKind::Agent(a) => {
            assert_eq!(a.name, "helper");
            assert_eq!(a.body, "Does helpful things");
        }
        _ => panic!("expected agent"),
    }
}

#[test]
fn expand_skill_set_honors_include_filter() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_skill(root, "alpha", "Alpha body");
    setup_copilot_skill(root, "beta", "Beta body");
    setup_copilot_skill(root, "gamma", "Gamma body");

    let rule = Rule {
        id: "picked".into(),
        source: Source::Local(PathBuf::from(".github/skills")),
        schema_agent: AgentName::new("copilot"),
        on_target_modified: None,
        kind: RuleKind::SkillSet {
            include: vec!["alpha".into(), "gamma".into()],
            exclude: vec![],
        },
    };
    let items = expand_rule(root, &rule).unwrap();

    let names: Vec<&str> = items
        .iter()
        .map(|i| match &i.kind {
            ExpandedKind::Skill(s) => s.name.as_str(),
            _ => panic!("expected skill"),
        })
        .collect();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"alpha"));
    assert!(names.contains(&"gamma"));
    assert!(!names.contains(&"beta"));
}

#[test]
fn expand_agent_set_honors_exclude_filter() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_agent(root, "reviewer", "Review");
    setup_copilot_agent(root, "planner", "Plan");
    setup_copilot_agent(root, "draft", "Draft");

    let rule = Rule {
        id: "mostly".into(),
        source: Source::Local(PathBuf::from(".github/agents")),
        schema_agent: AgentName::new("copilot"),
        on_target_modified: None,
        kind: RuleKind::AgentSet {
            include: vec![],
            exclude: vec!["draft".into()],
        },
    };
    let items = expand_rule(root, &rule).unwrap();

    let names: Vec<&str> = items
        .iter()
        .map(|i| match &i.kind {
            ExpandedKind::Agent(a) => a.name.as_str(),
            _ => panic!("expected agent"),
        })
        .collect();
    assert_eq!(names.len(), 2);
    assert!(!names.contains(&"draft"));
}

#[test]
fn expand_single_skill_applies_overrides() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_skill(
        root,
        "my-skill",
        "---\nname: Old Name\ndescription: Old desc\nallowed-tools: Read\n---\nBody",
    );

    let rule = Rule {
        id: "s1".into(),
        source: Source::Local(PathBuf::from(".github/skills/my-skill")),
        schema_agent: AgentName::new("copilot"),
        on_target_modified: None,
        kind: RuleKind::Skill {
            name: Some("New Name".into()),
            description: Some("New desc".into()),
            allowed_tools: Some("Read Grep Bash".into()),
        },
    };
    let items = expand_rule(root, &rule).unwrap();
    assert_eq!(items.len(), 1);
    match &items[0].kind {
        ExpandedKind::Skill(s) => {
            assert_eq!(s.front_matter.name.as_deref(), Some("New Name"));
            assert_eq!(s.front_matter.description.as_deref(), Some("New desc"));
            assert_eq!(s.front_matter.allowed_tools.len(), 3);
        }
        _ => panic!("expected skill"),
    }
}

// ── render_for_agent tests ───────────────────────────────────────────

#[test]
fn render_skill_for_claude_code() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_skill(
        root,
        "deploy",
        "---\nname: Deploy\n---\nDeploy instructions",
    );

    let rule = single_skill_rule("s1", ".github/skills/deploy", "copilot");
    let items = expand_rule(root, &rule).unwrap();
    let item = &items[0];

    let target = AgentName::new("claude-code");
    let rendered = render_for_agent(root, item, &AgentName::new("copilot"), &target).unwrap();

    assert_eq!(
        rendered.target_path,
        PathBuf::from(".claude/skills/deploy/SKILL.md")
    );
    assert!(rendered.content.contains("Deploy instructions"));
    assert!(!rendered.content_hash.is_empty());
}

#[test]
fn render_agent_for_copilot() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_claude_skill(root, "test-skill", "Body only");

    // Set up a claude-code agent
    let agents_dir = root.join(".claude/agents");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::write(
        agents_dir.join("reviewer.agent.md"),
        "---\nname: Reviewer\n---\nReview code",
    )
    .unwrap();

    let rule = single_agent_rule("a1", ".claude/agents/reviewer.agent.md", "claude-code");
    let items = expand_rule(root, &rule).unwrap();
    let item = &items[0];

    let target = AgentName::new("copilot");
    let rendered = render_for_agent(root, item, &AgentName::new("claude-code"), &target).unwrap();

    assert_eq!(
        rendered.target_path,
        PathBuf::from(".github/agents/reviewer.agent.md")
    );
    assert!(rendered.content.contains("Review code"));
}

// ── compute_plan tests ───────────────────────────────────────────────

#[test]
fn plan_create_when_no_lock_and_no_target() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_skill(root, "my-skill", "Skill body");

    let spec = make_spec(
        vec!["claude-code"],
        vec![skill_set_rule("skills", ".github/skills", "copilot")],
    );
    let lock = empty_lock();
    let plan = compute_plan(root, &spec, &lock, false, false).unwrap();

    assert_eq!(plan.items.len(), 1);
    assert_eq!(plan.items[0].status, DeploymentStatus::Create);
    assert_eq!(
        plan.items[0].target_path,
        PathBuf::from(".claude/skills/my-skill/SKILL.md")
    );
}

#[test]
fn plan_conflict_when_no_lock_but_target_exists() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_skill(root, "my-skill", "Skill body");

    // Pre-create the target file
    let target_dir = root.join(".claude/skills/my-skill");
    fs::create_dir_all(&target_dir).unwrap();
    fs::write(target_dir.join("SKILL.md"), "existing content").unwrap();

    let spec = make_spec(
        vec!["claude-code"],
        vec![skill_set_rule("skills", ".github/skills", "copilot")],
    );
    let lock = empty_lock();
    let plan = compute_plan(root, &spec, &lock, false, false).unwrap();

    assert_eq!(plan.items.len(), 1);
    assert!(matches!(
        plan.items[0].status,
        DeploymentStatus::Conflict { .. }
    ));
}

#[test]
fn plan_update_when_no_lock_but_target_exists_and_force() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_skill(root, "my-skill", "Skill body");

    // Pre-create the target file
    let target_dir = root.join(".claude/skills/my-skill");
    fs::create_dir_all(&target_dir).unwrap();
    fs::write(target_dir.join("SKILL.md"), "existing content").unwrap();

    let spec = make_spec(
        vec!["claude-code"],
        vec![skill_set_rule("skills", ".github/skills", "copilot")],
    );
    let lock = empty_lock();
    let plan = compute_plan(root, &spec, &lock, true, false).unwrap();

    assert_eq!(plan.items.len(), 1);
    assert_eq!(plan.items[0].status, DeploymentStatus::Update);
}

#[test]
fn plan_up_to_date_when_lock_matches() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let skill_content = "Skill body";
    setup_copilot_skill(root, "my-skill", skill_content);

    // First, compute plan and execute to set up initial state
    let spec = make_spec(
        vec!["claude-code"],
        vec![skill_set_rule("skills", ".github/skills", "copilot")],
    );
    let lock = empty_lock();
    let plan = compute_plan(root, &spec, &lock, false, false).unwrap();
    let new_lock = execute_plan(root, &plan, &lock, false).unwrap();

    // Now compute again with the new lock — should be up-to-date
    let plan2 = compute_plan(root, &spec, &new_lock, false, false).unwrap();
    assert_eq!(plan2.items.len(), 1);
    assert_eq!(plan2.items[0].status, DeploymentStatus::UpToDate);
    assert!(plan2.is_clean());
}

#[test]
fn plan_update_when_source_changes() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_skill(root, "my-skill", "Original body");

    let spec = make_spec(
        vec!["claude-code"],
        vec![skill_set_rule("skills", ".github/skills", "copilot")],
    );
    let lock = empty_lock();
    let plan = compute_plan(root, &spec, &lock, false, false).unwrap();
    let new_lock = execute_plan(root, &plan, &lock, false).unwrap();

    // Modify source
    setup_copilot_skill(root, "my-skill", "Updated body");

    let plan2 = compute_plan(root, &spec, &new_lock, false, false).unwrap();
    assert_eq!(plan2.items.len(), 1);
    assert_eq!(plan2.items[0].status, DeploymentStatus::Update);
}

#[test]
fn plan_conflict_when_target_modified_externally_and_policy_fail() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_skill(root, "my-skill", "Skill body");

    let spec = make_spec(
        vec!["claude-code"],
        vec![skill_set_rule("skills", ".github/skills", "copilot")],
    );
    let lock = empty_lock();
    let plan = compute_plan(root, &spec, &lock, false, false).unwrap();
    let new_lock = execute_plan(root, &plan, &lock, false).unwrap();

    // Externally modify the target
    let target = root.join(".claude/skills/my-skill/SKILL.md");
    fs::write(&target, "someone else edited this").unwrap();

    let plan2 = compute_plan(root, &spec, &new_lock, false, false).unwrap();
    assert_eq!(plan2.items.len(), 1);
    assert!(matches!(
        plan2.items[0].status,
        DeploymentStatus::Conflict { .. }
    ));
    assert!(plan2.has_conflicts());
}

#[test]
fn plan_overwrite_when_target_modified_and_policy_overwrite() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_skill(root, "my-skill", "Skill body");

    let mut rule = skill_set_rule("skills", ".github/skills", "copilot");
    rule.on_target_modified = Some(OnTargetModified::Overwrite);

    let spec = make_spec(vec!["claude-code"], vec![rule]);
    let lock = empty_lock();
    let plan = compute_plan(root, &spec, &lock, false, false).unwrap();
    let new_lock = execute_plan(root, &plan, &lock, false).unwrap();

    // Externally modify the target
    let target = root.join(".claude/skills/my-skill/SKILL.md");
    fs::write(&target, "someone else edited this").unwrap();

    let mut rule2 = skill_set_rule("skills", ".github/skills", "copilot");
    rule2.on_target_modified = Some(OnTargetModified::Overwrite);
    let spec2 = make_spec(vec!["claude-code"], vec![rule2]);

    let plan2 = compute_plan(root, &spec2, &new_lock, false, false).unwrap();
    assert_eq!(plan2.items.len(), 1);
    assert_eq!(plan2.items[0].status, DeploymentStatus::Update);
}

#[test]
fn plan_skip_when_target_modified_and_policy_skip() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_skill(root, "my-skill", "Skill body");

    let mut rule = skill_set_rule("skills", ".github/skills", "copilot");
    rule.on_target_modified = Some(OnTargetModified::Skip);

    let spec = make_spec(vec!["claude-code"], vec![rule]);
    let lock = empty_lock();
    let plan = compute_plan(root, &spec, &lock, false, false).unwrap();
    let new_lock = execute_plan(root, &plan, &lock, false).unwrap();

    // Externally modify the target
    let target = root.join(".claude/skills/my-skill/SKILL.md");
    fs::write(&target, "someone else edited this").unwrap();

    let mut rule2 = skill_set_rule("skills", ".github/skills", "copilot");
    rule2.on_target_modified = Some(OnTargetModified::Skip);
    let spec2 = make_spec(vec!["claude-code"], vec![rule2]);

    let plan2 = compute_plan(root, &spec2, &new_lock, false, false).unwrap();
    assert_eq!(plan2.items.len(), 1);
    assert_eq!(plan2.items[0].status, DeploymentStatus::UpToDate);
}

#[test]
fn plan_orphan_detection() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_skill(root, "skill-a", "A body");
    setup_copilot_skill(root, "skill-b", "B body");

    let spec = make_spec(
        vec!["claude-code"],
        vec![skill_set_rule("skills", ".github/skills", "copilot")],
    );
    let lock = empty_lock();
    let plan = compute_plan(root, &spec, &lock, false, false).unwrap();
    let new_lock = execute_plan(root, &plan, &lock, false).unwrap();

    assert_eq!(new_lock.deployments.len(), 2);

    // Remove skill-b from source
    fs::remove_dir_all(root.join(".github/skills/skill-b")).unwrap();

    let plan2 = compute_plan(root, &spec, &new_lock, false, false).unwrap();
    let orphans: Vec<_> = plan2
        .items
        .iter()
        .filter(|i| i.status == DeploymentStatus::Orphan)
        .collect();
    assert_eq!(orphans.len(), 1);
    assert_eq!(
        orphans[0].target_path,
        PathBuf::from(".claude/skills/skill-b/SKILL.md")
    );
    assert!(plan2.has_orphans());
}

// ── execute_plan tests ───────────────────────────────────────────────

#[test]
fn execute_creates_files() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_skill(root, "deploy", "Deploy instructions");

    let spec = make_spec(
        vec!["claude-code"],
        vec![skill_set_rule("skills", ".github/skills", "copilot")],
    );
    let lock = empty_lock();
    let plan = compute_plan(root, &spec, &lock, false, false).unwrap();
    let new_lock = execute_plan(root, &plan, &lock, false).unwrap();

    // Verify file was created
    let target = root.join(".claude/skills/deploy/SKILL.md");
    assert!(target.exists());
    let content = fs::read_to_string(&target).unwrap();
    assert!(content.contains("Deploy instructions"));

    // Verify lock was updated
    assert_eq!(new_lock.deployments.len(), 1);
    assert_eq!(new_lock.deployments[0].rule_id, "skills");
    assert_eq!(new_lock.deployments[0].agent, AgentName::new("claude-code"));
}

#[test]
fn execute_deletes_orphans() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_skill(root, "skill-a", "A body");
    setup_copilot_skill(root, "skill-b", "B body");

    let spec = make_spec(
        vec!["claude-code"],
        vec![skill_set_rule("skills", ".github/skills", "copilot")],
    );
    let lock = empty_lock();
    let plan = compute_plan(root, &spec, &lock, false, false).unwrap();
    let new_lock = execute_plan(root, &plan, &lock, false).unwrap();

    let target_b = root.join(".claude/skills/skill-b/SKILL.md");
    assert!(target_b.exists());

    // Remove skill-b from source
    fs::remove_dir_all(root.join(".github/skills/skill-b")).unwrap();

    let plan2 = compute_plan(root, &spec, &new_lock, false, false).unwrap();
    let new_lock2 = execute_plan(root, &plan2, &new_lock, false).unwrap();

    assert!(!target_b.exists());
    assert_eq!(new_lock2.deployments.len(), 1);
}

#[test]
fn execute_check_mode_does_not_write() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_skill(root, "my-skill", "Skill body");

    let spec = make_spec(
        vec!["claude-code"],
        vec![skill_set_rule("skills", ".github/skills", "copilot")],
    );
    let lock = empty_lock();
    let plan = compute_plan(root, &spec, &lock, false, false).unwrap();
    let new_lock = execute_plan(root, &plan, &lock, true).unwrap();

    let target = root.join(".claude/skills/my-skill/SKILL.md");
    assert!(!target.exists());
    assert_eq!(new_lock.deployments.len(), 1);
}

#[test]
fn execute_errors_on_conflicts() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_skill(root, "my-skill", "Skill body");

    // Pre-create the target file to cause conflict
    let target_dir = root.join(".claude/skills/my-skill");
    fs::create_dir_all(&target_dir).unwrap();
    fs::write(target_dir.join("SKILL.md"), "existing content").unwrap();

    let spec = make_spec(
        vec!["claude-code"],
        vec![skill_set_rule("skills", ".github/skills", "copilot")],
    );
    let lock = empty_lock();
    let plan = compute_plan(root, &spec, &lock, false, false).unwrap();
    assert!(plan.has_conflicts());

    let result = execute_plan(root, &plan, &lock, false);
    assert!(result.is_err());
}

// ── Multi-agent tests ────────────────────────────────────────────────

#[test]
fn plan_deploys_to_multiple_agents() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_skill(root, "my-skill", "Skill body");

    let spec = make_spec(
        vec!["claude-code", "codex"],
        vec![skill_set_rule("skills", ".github/skills", "copilot")],
    );
    let lock = empty_lock();
    let plan = compute_plan(root, &spec, &lock, false, false).unwrap();

    assert_eq!(plan.items.len(), 2);
    let paths: Vec<PathBuf> = plan.items.iter().map(|i| i.target_path.clone()).collect();
    assert!(paths.contains(&PathBuf::from(".claude/skills/my-skill/SKILL.md")));
    assert!(paths.contains(&PathBuf::from(".codex/skills/my-skill/SKILL.md")));
}

#[test]
fn plan_skips_when_source_equals_target() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_skill(root, "my-skill", "Skill body");

    // copilot source AND copilot target — would round-trip onto itself.
    let spec = make_spec(
        vec!["copilot", "claude-code"],
        vec![skill_set_rule("skills", ".github/skills", "copilot")],
    );
    let lock = empty_lock();
    let plan = compute_plan(root, &spec, &lock, false, false).unwrap();

    // copilot self-write is dropped; only claude-code remains.
    assert_eq!(plan.items.len(), 1);
    assert_eq!(plan.items[0].agent, AgentName::new("claude-code"));
    assert_eq!(
        plan.items[0].target_path,
        PathBuf::from(".claude/skills/my-skill/SKILL.md")
    );
}

#[test]
fn full_roundtrip_with_agents() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Set up source agents in copilot format
    setup_copilot_agent(
        root,
        "reviewer",
        "---\nname: Reviewer\n---\nReview code carefully",
    );

    let spec = make_spec(
        vec!["claude-code"],
        vec![agent_set_rule("agents", ".github/agents", "copilot")],
    );
    let lock = empty_lock();

    let plan = compute_plan(root, &spec, &lock, false, false).unwrap();
    assert_eq!(plan.items.len(), 1);
    assert_eq!(plan.items[0].status, DeploymentStatus::Create);

    let new_lock = execute_plan(root, &plan, &lock, false).unwrap();

    let target = root.join(".claude/agents/reviewer.agent.md");
    assert!(target.exists());
    let content = fs::read_to_string(&target).unwrap();
    assert!(content.contains("Review code carefully"));
    assert!(content.contains("name:"));

    // Second sync — should be up-to-date
    let plan2 = compute_plan(root, &spec, &new_lock, false, false).unwrap();
    assert!(plan2.is_clean());
}

// ── Ownership tests ──────────────────────────────────────────────────

#[test]
fn single_file_rule_beats_set_for_shared_path() {
    // Single-file rule writes into a directory that a skill-set rule also
    // scans. The set must skip the file the single rule owns; otherwise we'd
    // get "target file exists but is not tracked in lock" conflicts on the
    // second sync.
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_claude_skill(root, "git-helper", "Git helper body");
    setup_copilot_skill(root, "other", "Other body");

    let spec = make_spec(
        vec!["claude-code", "copilot"],
        vec![
            single_skill_rule("single", ".claude/skills/git-helper", "claude-code"),
            skill_set_rule("set", ".github/skills", "copilot"),
        ],
    );
    let lock = empty_lock();

    // First sync: single writes .github/skills/git-helper; set expands to [other].
    let plan = compute_plan(root, &spec, &lock, false, false).unwrap();
    let lock1 = execute_plan(root, &plan, &lock, false).unwrap();
    assert!(root.join(".github/skills/git-helper/SKILL.md").exists());
    assert!(root.join(".claude/skills/other/SKILL.md").exists());

    // Second sync: set would see git-helper in .github/skills but the single
    // rule owns it — no conflict, no duplicate deployment.
    let plan2 = compute_plan(root, &spec, &lock1, false, false).unwrap();
    assert!(
        !plan2.has_conflicts(),
        "expected no conflicts, got {:?}",
        plan2.items
    );
    assert!(plan2.is_clean(), "second sync should be up-to-date");

    // Set rule must not track git-helper.
    let set_entries: Vec<_> = lock1
        .deployments
        .iter()
        .filter(|d| d.rule_id == "set")
        .collect();
    assert_eq!(set_entries.len(), 1);
    assert_eq!(
        set_entries[0].content,
        PathBuf::from(".claude/skills/other/SKILL.md")
    );
}

#[test]
fn set_vs_set_errors_without_lock_decision() {
    // Two skill-set rules both claim the same directory. Without a user
    // decision recorded in the lock, the engine refuses to guess.
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_skill(root, "foo", "Foo body");

    let spec = make_spec(
        vec!["claude-code"],
        vec![
            skill_set_rule("a", ".github/skills", "copilot"),
            skill_set_rule("b", ".github/skills", "copilot"),
        ],
    );
    let lock = empty_lock();
    let err = compute_plan(root, &spec, &lock, false, false).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("ambiguous ownership"), "got: {}", msg);
    assert!(msg.contains("\"a\""), "got: {}", msg);
    assert!(msg.contains("\"b\""), "got: {}", msg);
}

#[test]
fn set_vs_set_respects_lock_owners_override() {
    // Given two set rules that both claim a file, a lock entry in `owners:`
    // picks the winner.
    use rtango::spec::Ownership;

    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_skill(root, "foo", "Foo body");

    let spec = make_spec(
        vec!["claude-code"],
        vec![
            skill_set_rule("a", ".github/skills", "copilot"),
            skill_set_rule("b", ".github/skills", "copilot"),
        ],
    );
    let mut lock = empty_lock();
    lock.owners.push(Ownership {
        path: root.join(".github/skills/foo/SKILL.md"),
        rule_id: "a".into(),
    });
    lock.owners.push(Ownership {
        path: root.join(".claude/skills/foo/SKILL.md"),
        rule_id: "a".into(),
    });

    let plan = compute_plan(root, &spec, &lock, false, false).unwrap();
    let foo_items: Vec<_> = plan
        .items
        .iter()
        .filter(|i| i.target_path.as_path() == Path::new(".claude/skills/foo/SKILL.md"))
        .collect();
    assert_eq!(foo_items.len(), 1);
    assert_eq!(foo_items[0].rule_id, "a");

    // Contested owners survive into the new lock so the decision persists.
    let new_lock = execute_plan(root, &plan, &lock, false).unwrap();
    assert!(new_lock.owners.iter().any(|o| o.rule_id == "a"));
}

#[test]
fn reparented_path_is_not_marked_as_orphan() {
    // A stale lock entry (rule A, path X) must not appear as an orphan when
    // the current spec has rule B owning X. The old behavior would emit an
    // Orphan and the executor would delete the file — which is the file B
    // now depends on.
    use rtango::spec::{AgentName, Deployment, Source};

    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_skill(root, "foo", "Foo body");

    // Lock captures a prior sync where rule "single" owned foo's deployment.
    let lock = Lock {
        version: 1,
        tracked_agents: vec![],
        owners: vec![],
        deployments: vec![Deployment {
            rule_id: "single".into(),
            agent: AgentName::new("claude-code"),
            source: Source::Local(PathBuf::from(".github/skills/foo")),
            source_hash: "deadbeef".into(),
            content: PathBuf::from(".claude/skills/foo/SKILL.md"),
            content_hash: "deadbeef".into(),
        }],
    };

    // Current spec: rule "set" now owns foo.
    let spec = make_spec(
        vec!["claude-code"],
        vec![skill_set_rule("set", ".github/skills", "copilot")],
    );
    let plan = compute_plan(root, &spec, &lock, false, false).unwrap();
    let orphans: Vec<_> = plan
        .items
        .iter()
        .filter(|i| i.status == DeploymentStatus::Orphan)
        .collect();
    assert_eq!(
        orphans.len(),
        0,
        "stale (single,...) lock entry must not become an orphan; got {:?}",
        orphans
    );
}

#[test]
fn reparented_target_adopts_existing_content_without_conflict() {
    // Rule A owned .claude/skills/foo/SKILL.md on a prior sync. Spec now has
    // rule B owning it. The on-disk file still matches the lock's recorded
    // content_hash, so the new plan should adopt it as UpToDate — not emit
    // a "target file exists but is not tracked in lock" conflict.
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_skill(root, "foo", "Foo body");

    // First sync with rule "single" — writes disk + records a lock entry.
    let spec_v1 = make_spec(
        vec!["claude-code"],
        vec![single_skill_rule("single", ".github/skills/foo", "copilot")],
    );
    let plan_v1 = compute_plan(root, &spec_v1, &empty_lock(), false, false).unwrap();
    let lock_v1 = execute_plan(root, &plan_v1, &empty_lock(), false).unwrap();
    assert!(root.join(".claude/skills/foo/SKILL.md").exists());
    assert_eq!(
        lock_v1
            .deployments
            .iter()
            .filter(|d| d.rule_id == "single")
            .count(),
        1
    );

    // Spec v2: same source file, now owned by a set rule.
    let spec_v2 = make_spec(
        vec!["claude-code"],
        vec![skill_set_rule("set", ".github/skills", "copilot")],
    );
    let plan_v2 = compute_plan(root, &spec_v2, &lock_v1, false, false).unwrap();

    assert!(
        !plan_v2.has_conflicts(),
        "reparent should not produce a conflict; got {:?}",
        plan_v2.items
    );
    let set_item = plan_v2
        .items
        .iter()
        .find(|i| {
            i.rule_id == "set"
                && i.target_path.as_path() == Path::new(".claude/skills/foo/SKILL.md")
        })
        .expect("expected a set-rule deployment for foo");
    assert_eq!(
        set_item.status,
        DeploymentStatus::UpToDate,
        "expected adopted content to be UpToDate, got {:?}",
        set_item.status
    );

    // Second sync with the set rule should succeed and leave the file alone.
    let lock_v2 = execute_plan(root, &plan_v2, &lock_v1, false).unwrap();
    assert!(lock_v2.deployments.iter().any(|d| d.rule_id == "set"));
    assert!(!lock_v2.deployments.iter().any(|d| d.rule_id == "single"));
}

#[test]
fn reparented_target_with_modified_source_is_updated() {
    // Reparent + the source content changed since the prior lock. The new
    // owner should plan an Update (not a conflict), because the on-disk file
    // still matches the prior content_hash but the source now differs.
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_skill(root, "foo", "Foo body");

    let spec_v1 = make_spec(
        vec!["claude-code"],
        vec![single_skill_rule("single", ".github/skills/foo", "copilot")],
    );
    let plan_v1 = compute_plan(root, &spec_v1, &empty_lock(), false, false).unwrap();
    let lock_v1 = execute_plan(root, &plan_v1, &empty_lock(), false).unwrap();

    // Source changes.
    setup_copilot_skill(root, "foo", "Foo body v2");

    // Reparent to set rule.
    let spec_v2 = make_spec(
        vec!["claude-code"],
        vec![skill_set_rule("set", ".github/skills", "copilot")],
    );
    let plan_v2 = compute_plan(root, &spec_v2, &lock_v1, false, false).unwrap();

    assert!(!plan_v2.has_conflicts(), "got {:?}", plan_v2.items);
    let set_item = plan_v2
        .items
        .iter()
        .find(|i| {
            i.rule_id == "set"
                && i.target_path.as_path() == Path::new(".claude/skills/foo/SKILL.md")
        })
        .unwrap();
    assert_eq!(set_item.status, DeploymentStatus::Update);
}

// ── Builtin skill tests ───────────────────────────────────────────────

#[test]
fn builtin_injects_rtango_skill_when_enabled() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_claude_skill(root, "my-skill", "# My Skill\n");

    let spec = make_spec(
        vec!["claude-code"],
        vec![single_skill_rule(
            "my-skill",
            ".claude/skills/my-skill",
            "claude-code",
        )],
    );
    let lock = empty_lock();

    let plan = compute_plan(root, &spec, &lock, false, true).unwrap();

    // The user skill is a self-write (claude-code → claude-code), so it's skipped.
    // Only the builtin should appear.
    let builtin_items: Vec<_> = plan
        .items
        .iter()
        .filter(|i| i.rule_id == "_builtin_rtango")
        .collect();
    assert_eq!(builtin_items.len(), 1);
    assert_eq!(builtin_items[0].status, DeploymentStatus::Create);
    assert_eq!(
        builtin_items[0].target_path,
        PathBuf::from(".claude/skills/rtango/SKILL.md")
    );
}

#[test]
fn builtin_not_injected_when_disabled() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_claude_skill(root, "my-skill", "# My Skill\n");

    let spec = make_spec(
        vec!["claude-code"],
        vec![single_skill_rule(
            "my-skill",
            ".claude/skills/my-skill",
            "claude-code",
        )],
    );
    let lock = empty_lock();

    let plan = compute_plan(root, &spec, &lock, false, false).unwrap();
    let builtin_items: Vec<_> = plan
        .items
        .iter()
        .filter(|i| i.rule_id == "_builtin_rtango")
        .collect();
    assert!(builtin_items.is_empty());
}

#[test]
fn builtin_not_tracked_in_lock() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_claude_skill(root, "my-skill", "# My Skill\n");

    let spec = make_spec(
        vec!["claude-code"],
        vec![single_skill_rule(
            "my-skill",
            ".claude/skills/my-skill",
            "claude-code",
        )],
    );
    let lock = empty_lock();

    let plan = compute_plan(root, &spec, &lock, false, true).unwrap();
    let new_lock = execute_plan(root, &plan, &lock, false).unwrap();

    // Lock should be empty: user skill is a self-write (no deployment),
    // builtin is not tracked.
    assert!(new_lock.deployments.is_empty());
}

#[test]
fn builtin_skips_source_dirs_of_user_rules() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    // User has a skill-set rule for .github/skills/
    setup_copilot_skill(root, "my-skill", "# My Skill\n");

    let spec = make_spec(
        vec!["claude-code", "copilot"],
        vec![skill_set_rule("my-skills", ".github/skills", "copilot")],
    );
    let lock = empty_lock();

    let plan = compute_plan(root, &spec, &lock, false, true).unwrap();
    let builtin_items: Vec<_> = plan
        .items
        .iter()
        .filter(|i| i.rule_id == "_builtin_rtango")
        .collect();

    // Builtin should skip .github/skills/rtango/SKILL.md (it's a source dir)
    // but should write .claude/skills/rtango/SKILL.md
    for bi in &builtin_items {
        assert!(
            !bi.target_path.starts_with(".github/"),
            "builtin should not write to .github/: {:?}",
            bi.target_path
        );
    }
    assert!(
        !builtin_items.is_empty(),
        "should write to at least one agent"
    );
}

#[test]
fn managed_gitignore_entries_include_builtins() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_skill(root, "my-skill", "body\n");

    let spec = make_spec(
        vec!["claude-code"],
        vec![skill_set_rule("skills", ".github/skills", "copilot")],
    );
    let lock = empty_lock();

    let plan = compute_plan(root, &spec, &lock, false, true).unwrap();
    let entries = managed_gitignore_entries(&plan, None);

    assert!(entries.contains(&".claude/skills/my-skill/".to_string()));
    assert!(entries.contains(&".claude/skills/rtango/".to_string()));
}
