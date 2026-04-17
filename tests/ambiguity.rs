use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use rtango::cmd::sync::Prompter;
use rtango::engine::{AmbiguousPath, find_ambiguities};
use rtango::spec::io::{load_lock, load_lock_or_empty};
use rtango::spec::{
    AgentName, Defaults, Lock, Rule, RuleKind, Source, Spec,
};

// ── Helpers ──────────────────────────────────────────────────────────

fn setup_copilot_skill(root: &Path, name: &str, body: &str) {
    let dir = root.join(format!(".github/skills/{}", name));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("SKILL.md"), body).unwrap();
}

fn write_spec(root: &Path, spec: &Spec) {
    fs::create_dir_all(root.join(".rtango")).unwrap();
    let yaml = serde_yml::to_string(spec).unwrap();
    fs::write(root.join(".rtango/spec.yaml"), yaml).unwrap();
}

fn empty_lock() -> Lock {
    Lock {
        version: 1,
        tracked_agents: vec![],
        owners: vec![],
        deployments: vec![],
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

/// Canned prompter that replays answers in order, one per ambiguity.
/// `None` entries simulate the user aborting.
struct ScriptedPrompter {
    answers: Vec<Option<String>>,
    calls: Vec<AmbiguousPath>,
}

impl Prompter for ScriptedPrompter {
    fn choose_owner(&mut self, ambiguity: &AmbiguousPath) -> anyhow::Result<Option<String>> {
        self.calls.push(ambiguity.clone());
        Ok(self.answers.remove(0))
    }
}

struct PanickingPrompter;

impl Prompter for PanickingPrompter {
    fn choose_owner(&mut self, ambiguity: &AmbiguousPath) -> anyhow::Result<Option<String>> {
        panic!("prompter should not be called; got {:?}", ambiguity);
    }
}

// ── find_ambiguities tests ───────────────────────────────────────────

#[test]
fn find_ambiguities_reports_set_vs_set_paths() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_skill(root, "foo", "Foo body");

    let spec = two_set_spec();
    let ambiguities = find_ambiguities(root, &spec, &empty_lock()).unwrap();
    assert!(!ambiguities.is_empty(), "expected at least one ambiguity");
    for a in &ambiguities {
        let mut names = a.candidates.clone();
        names.sort();
        assert_eq!(names, vec!["a".to_string(), "b".to_string()]);
    }
}

#[test]
fn find_ambiguities_is_empty_when_lock_resolves_all() {
    use rtango::spec::Ownership;

    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_skill(root, "foo", "Foo body");

    let spec = two_set_spec();
    let mut lock = empty_lock();
    // Record decisions for every contested path.
    lock.owners.push(Ownership {
        path: root.join(".github/skills/foo/SKILL.md"),
        rule_id: "a".into(),
    });
    lock.owners.push(Ownership {
        path: root.join(".claude/skills/foo/SKILL.md"),
        rule_id: "a".into(),
    });

    let ambiguities = find_ambiguities(root, &spec, &lock).unwrap();
    assert!(
        ambiguities.is_empty(),
        "lock decisions should resolve everything; got {:?}",
        ambiguities
    );
}

// ── sync interactive prompt tests ────────────────────────────────────

#[test]
fn sync_prompts_on_set_vs_set_and_records_decision() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_skill(root, "foo", "Foo body");
    write_spec(root, &two_set_spec());

    let mut prompter = ScriptedPrompter {
        // Two contested paths (source + target). Answer "a" for both.
        answers: vec![Some("a".into()), Some("a".into())],
        calls: vec![],
    };

    let result = rtango::cmd::sync::exec_with_prompter(
        root, false, false, None, false, &mut prompter,
    );
    assert!(result.is_ok(), "sync failed: {:?}", result.err());

    assert_eq!(prompter.calls.len(), 2);

    // Lock now holds the decisions.
    let lock = load_lock(root).unwrap();
    assert!(
        !lock.owners.is_empty(),
        "expected owner decisions to persist in lock"
    );
    assert!(lock.owners.iter().all(|o| o.rule_id == "a"));

    // And no ambiguities remain.
    let spec = rtango::spec::io::load_spec(root).unwrap();
    let lock = load_lock_or_empty(root).unwrap();
    assert!(find_ambiguities(root, &spec, &lock).unwrap().is_empty());
}

#[test]
fn sync_errors_when_prompter_aborts() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_skill(root, "foo", "Foo body");
    write_spec(root, &two_set_spec());

    let mut prompter = ScriptedPrompter {
        answers: vec![None],
        calls: vec![],
    };

    let err = rtango::cmd::sync::exec_with_prompter(
        root, false, false, None, false, &mut prompter,
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("ambiguous ownership"),
        "err: {}",
        err
    );
}

#[test]
fn sync_does_not_prompt_when_unambiguous() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    setup_copilot_skill(root, "foo", "Foo body");

    let spec = Spec {
        version: 1,
        agents: vec![AgentName::new("claude-code")],
        defaults: Defaults::default(),
        rules: vec![skill_set_rule("only", ".github/skills", "copilot")],
    };
    write_spec(root, &spec);

    let mut prompter = PanickingPrompter;
    let result = rtango::cmd::sync::exec_with_prompter(
        root, false, false, None, false, &mut prompter,
    );
    assert!(result.is_ok(), "sync failed: {:?}", result.err());
}
