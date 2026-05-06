/// Tests for conflict resolution when two collection rules expand to the same
/// target paths (collection-vs-collection ambiguity).
///
/// Mirrors the existing `ambiguity.rs` test structure but uses local
/// collections instead of skill-set rules, covering the full pipeline:
/// find_ambiguities → sync prompt → lock persistence → own reassignment.
use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use rtango::cmd::own;
use rtango::cmd::sync::Prompter;
use rtango::engine::{AmbiguousPath, builtin::BUILTIN_RTANGO_RULE_ID, find_ambiguities};
use rtango::spec::io::{load_lock, load_lock_or_empty, load_spec};
use rtango::spec::{AgentName, Defaults, Ownership, Rule, RuleKind, Source, Spec};

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Create a minimal local collection directory:
///   <root>/collections/<name>/.rtango/spec.yaml
///   <root>/collections/<name>/skills/<skill>/SKILL.md  (for each skill)
///
/// The collection uses `pi` as schema_agent — ensures that frontmatter is
/// correctly re-rendered when the host project targets other agents.
fn setup_collection(root: &Path, name: &str, skills: &[&str]) -> PathBuf {
    let col_dir = root.join("collections").join(name);

    let col_spec = Spec {
        version: 1,
        agents: vec![AgentName::new("pi")],
        defaults: Defaults::default(),
        rules: skills
            .iter()
            .map(|s| Rule {
                id: s.to_string(),
                source: Source::Local(PathBuf::from(format!("skills/{s}"))),
                schema_agent: AgentName::new("pi"),
                on_target_modified: None,
                kind: RuleKind::skill(),
            })
            .collect(),
    };

    let rtango_dir = col_dir.join(".rtango");
    fs::create_dir_all(&rtango_dir).unwrap();
    let yaml = serde_yml::to_string(&col_spec).unwrap();
    fs::write(rtango_dir.join("spec.yaml"), yaml).unwrap();

    for s in skills {
        let skill_dir = col_dir.join("skills").join(s);
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            format!("---\nname: {s}\ndescription: {s} skill\n---\n\n# {s}\nBody.\n"),
        )
        .unwrap();
    }

    col_dir
}

/// Build a spec with two local-collection rules that both contain `skills`.
fn two_collection_spec(_root: &Path, col_a: &Path, col_b: &Path) -> Spec {
    Spec {
        version: 1,
        agents: vec![AgentName::new("claude-code")],
        defaults: Defaults::default(),
        rules: vec![
            Rule {
                id: "col-a".into(),
                source: Source::Local(col_a.to_path_buf()),
                schema_agent: AgentName::new("claude-code"),
                on_target_modified: None,
                kind: RuleKind::collection(),
            },
            Rule {
                id: "col-b".into(),
                source: Source::Local(col_b.to_path_buf()),
                schema_agent: AgentName::new("claude-code"),
                on_target_modified: None,
                kind: RuleKind::collection(),
            },
        ],
    }
}

fn write_spec(root: &Path, spec: &Spec) {
    fs::create_dir_all(root.join(".rtango")).unwrap();
    let yaml = serde_yml::to_string(spec).unwrap();
    fs::write(root.join(".rtango/spec.yaml"), yaml).unwrap();
}

fn empty_lock() -> rtango::spec::Lock {
    rtango::spec::Lock {
        version: 1,
        tracked_agents: vec![],
        owners: vec![],
        deployments: vec![],
    }
}

/// Scripted prompter: replays prepared answers in order.
/// `None` simulates the user aborting (empty input / EOF).
struct ScriptedPrompter {
    answers: Vec<Option<String>>,
    /// Accumulates all ambiguities the prompter was called with, for assertions.
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
        panic!("prompter should not have been called; got {:?}", ambiguity);
    }
}

// ── find_ambiguities tests ────────────────────────────────────────────────────

#[test]
fn two_collections_with_same_skill_report_ambiguity() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let col_a = setup_collection(root, "alpha", &["shared-skill"]);
    let col_b = setup_collection(root, "beta", &["shared-skill"]);
    let spec = two_collection_spec(root, &col_a, &col_b);

    let ambiguities = find_ambiguities(root, &spec, &empty_lock()).unwrap();

    assert!(
        !ambiguities.is_empty(),
        "expected ambiguities for overlapping collections"
    );
    for a in &ambiguities {
        let mut cands = a.candidates.clone();
        cands.sort();
        assert_eq!(
            cands,
            vec!["col-a".to_string(), "col-b".to_string()],
            "claimants should be both collection rule ids"
        );
    }
}

#[test]
fn two_collections_distinct_skills_no_ambiguity() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // col-a has skill-x, col-b has skill-y — no path overlap
    let col_a = setup_collection(root, "alpha", &["skill-x"]);
    let col_b = setup_collection(root, "beta", &["skill-y"]);
    let spec = two_collection_spec(root, &col_a, &col_b);

    let ambiguities = find_ambiguities(root, &spec, &empty_lock()).unwrap();

    assert!(
        ambiguities.is_empty(),
        "distinct skills produce no ambiguity; got {:?}",
        ambiguities
    );
}

#[test]
fn overlapping_collections_ambiguity_clears_when_lock_decides() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let col_a = setup_collection(root, "alpha", &["shared"]);
    let col_b = setup_collection(root, "beta", &["shared"]);
    let spec = two_collection_spec(root, &col_a, &col_b);

    // Seed the lock with a pre-recorded ownership decision for the target path.
    let mut lock = empty_lock();
    lock.owners.push(Ownership {
        path: root.join(".claude/skills/shared/SKILL.md"),
        rule_id: "col-a".into(),
    });

    let ambiguities = find_ambiguities(root, &spec, &lock).unwrap();
    assert!(
        ambiguities.is_empty(),
        "pre-recorded lock decision should eliminate ambiguity; got {:?}",
        ambiguities
    );
}

// ── sync interactive-prompt tests ────────────────────────────────────────────

#[test]
fn sync_prompts_for_each_contested_target_and_persists_decisions() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let col_a = setup_collection(root, "alpha", &["shared"]);
    let col_b = setup_collection(root, "beta", &["shared"]);
    write_spec(root, &two_collection_spec(root, &col_a, &col_b));

    // One target path is contested: .claude/skills/shared/SKILL.md
    // (source-file paths are different per collection, so only 1 prompt)
    let mut prompter = ScriptedPrompter {
        answers: vec![Some("col-a".into())],
        calls: vec![],
    };

    rtango::cmd::sync::exec_with_prompter(root, false, false, None, false, &mut prompter)
        .expect("sync should succeed after prompt");

    assert_eq!(prompter.calls.len(), 1, "expected exactly one prompt");
    assert_eq!(
        prompter.calls[0].candidates,
        vec!["col-a".to_string(), "col-b".to_string()]
    );

    // Lock persists the decision.
    let lock = load_lock(root).unwrap();
    assert!(
        lock.owners.iter().any(|o| o.rule_id == "col-a"),
        "lock should record col-a as owner"
    );

    // Skill was written from col-a.
    let content = fs::read_to_string(root.join(".claude/skills/shared/SKILL.md")).unwrap();
    assert!(content.contains("shared"), "rendered file should have skill content");
}

#[test]
fn sync_writes_correct_owner_content_after_resolution() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Give each collection a distinguishable skill body so we can tell them apart.
    let col_a = setup_collection(root, "alpha", &["shared"]);
    let col_b = setup_collection(root, "beta", &["shared"]);

    // Overwrite col-b's skill with distinct content.
    fs::write(
        col_b.join("skills/shared/SKILL.md"),
        "---\nname: shared\ndescription: from-beta\n---\n\n# From beta\n",
    )
    .unwrap();

    write_spec(root, &two_collection_spec(root, &col_a, &col_b));

    // Pick col-b as owner.
    let mut prompter = ScriptedPrompter {
        answers: vec![Some("col-b".into())],
        calls: vec![],
    };

    rtango::cmd::sync::exec_with_prompter(root, false, false, None, false, &mut prompter)
        .expect("sync should succeed");

    let content = fs::read_to_string(root.join(".claude/skills/shared/SKILL.md")).unwrap();
    assert!(
        content.contains("from-beta"),
        "file should contain col-b's content; got:\n{content}"
    );
}

#[test]
fn sync_aborts_when_prompter_returns_none() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let col_a = setup_collection(root, "alpha", &["shared"]);
    let col_b = setup_collection(root, "beta", &["shared"]);
    write_spec(root, &two_collection_spec(root, &col_a, &col_b));

    let mut prompter = ScriptedPrompter {
        answers: vec![None],
        calls: vec![],
    };

    let err =
        rtango::cmd::sync::exec_with_prompter(root, false, false, None, false, &mut prompter)
            .unwrap_err();

    assert!(
        err.to_string().contains("ambiguous ownership"),
        "expected ambiguity error; got: {err}"
    );
}

#[test]
fn single_collection_never_prompts() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let col_a = setup_collection(root, "alpha", &["only-skill"]);

    let spec = Spec {
        version: 1,
        agents: vec![AgentName::new("claude-code")],
        defaults: Defaults::default(),
        rules: vec![Rule {
            id: "col-a".into(),
            source: Source::Local(col_a.clone()),
            schema_agent: AgentName::new("claude-code"),
            on_target_modified: None,
            kind: RuleKind::collection(),
        }],
    };
    write_spec(root, &spec);

    let mut prompter = PanickingPrompter;
    rtango::cmd::sync::exec_with_prompter(root, false, false, None, false, &mut prompter)
        .expect("single collection should sync without prompts");

    assert!(
        root.join(".claude/skills/only-skill/SKILL.md").exists(),
        "skill file should have been created"
    );
}

// ── post-resolution idempotency ───────────────────────────────────────────────

#[test]
fn resolved_lock_survives_subsequent_syncs_without_reprompt() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let col_a = setup_collection(root, "alpha", &["shared"]);
    let col_b = setup_collection(root, "beta", &["shared"]);
    write_spec(root, &two_collection_spec(root, &col_a, &col_b));

    // First sync: resolve interactively.
    let mut prompter = ScriptedPrompter {
        answers: vec![Some("col-a".into())],
        calls: vec![],
    };
    rtango::cmd::sync::exec_with_prompter(root, false, false, None, false, &mut prompter)
        .expect("first sync");

    // Second sync: no prompt at all.
    let mut second_prompter = PanickingPrompter;
    rtango::cmd::sync::exec_with_prompter(root, false, false, None, false, &mut second_prompter)
        .expect("second sync should be clean without prompts");

    // --check also exits cleanly.
    let mut check_prompter = PanickingPrompter;
    rtango::cmd::sync::exec_with_prompter(root, true, false, None, false, &mut check_prompter)
        .expect("sync --check should pass");
}

// ── include filter removes overlap ───────────────────────────────────────────

#[test]
fn include_filter_on_collection_eliminates_overlap() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Both collections contain skill-a and skill-b.
    let col_a = setup_collection(root, "alpha", &["skill-a", "skill-b"]);
    let col_b = setup_collection(root, "beta", &["skill-a", "skill-b"]);

    // col-a includes only skill-a; col-b includes only skill-b → no overlap.
    let spec = Spec {
        version: 1,
        agents: vec![AgentName::new("claude-code")],
        defaults: Defaults::default(),
        rules: vec![
            Rule {
                id: "col-a".into(),
                source: Source::Local(col_a.clone()),
                schema_agent: AgentName::new("claude-code"),
                on_target_modified: None,
                kind: RuleKind::Collection {
                    include: vec!["skill-a".into()],
                    exclude: vec![],
                    schema_override: None,
                },
            },
            Rule {
                id: "col-b".into(),
                source: Source::Local(col_b.clone()),
                schema_agent: AgentName::new("claude-code"),
                on_target_modified: None,
                kind: RuleKind::Collection {
                    include: vec!["skill-b".into()],
                    exclude: vec![],
                    schema_override: None,
                },
            },
        ],
    };
    write_spec(root, &spec);

    // No ambiguities because the include filters partition the skill space.
    let ambiguities = find_ambiguities(root, &spec, &empty_lock()).unwrap();
    assert!(
        ambiguities.is_empty(),
        "include filters should eliminate overlap; got {:?}",
        ambiguities
    );

    // Sync without any prompts.
    let mut prompter = PanickingPrompter;
    rtango::cmd::sync::exec_with_prompter(root, false, false, None, false, &mut prompter)
        .expect("filtered sync should need no prompts");

    // Each skill was written by the correct collection.
    assert!(root.join(".claude/skills/skill-a/SKILL.md").exists());
    assert!(root.join(".claude/skills/skill-b/SKILL.md").exists());
}

// ── rtango own reassignment ───────────────────────────────────────────────────

#[test]
fn own_reassigns_contested_path_between_collections() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let col_a = setup_collection(root, "alpha", &["shared"]);
    let col_b = setup_collection(root, "beta", &["shared"]);

    // Give col-b a distinct body so we can verify reassignment.
    fs::write(
        col_b.join("skills/shared/SKILL.md"),
        "---\nname: shared\ndescription: from-beta\n---\n\n# From beta\n",
    )
    .unwrap();

    write_spec(root, &two_collection_spec(root, &col_a, &col_b));

    // First sync: col-a wins.
    let mut prompter = ScriptedPrompter {
        answers: vec![Some("col-a".into())],
        calls: vec![],
    };
    rtango::cmd::sync::exec_with_prompter(root, false, false, None, false, &mut prompter)
        .expect("first sync");

    let content_before = fs::read_to_string(root.join(".claude/skills/shared/SKILL.md")).unwrap();
    assert!(
        !content_before.contains("from-beta"),
        "col-a should have written the file first"
    );

    // Reassign ownership to col-b.
    own::exec(
        root,
        PathBuf::from(".claude/skills/shared/SKILL.md"),
        Some("col-b".into()),
        false,
    )
    .expect("own should succeed");

    // Verify the lock now records col-b as owner.
    let lock = load_lock(root).unwrap();
    let owner_entry = lock
        .owners
        .iter()
        .find(|o| o.path.ends_with("shared/SKILL.md"))
        .expect("owner entry should exist");
    assert_eq!(owner_entry.rule_id, "col-b");

    // Second sync: col-b's content is written without prompts.
    let mut second_prompter = PanickingPrompter;
    rtango::cmd::sync::exec_with_prompter(root, false, false, None, false, &mut second_prompter)
        .expect("second sync after own");

    let content_after = fs::read_to_string(root.join(".claude/skills/shared/SKILL.md")).unwrap();
    assert!(
        content_after.contains("from-beta"),
        "col-b's content should now be on disk; got:\n{content_after}"
    );

    // Third sync: clean.
    let spec_loaded = load_spec(root).unwrap();
    let lock_loaded = load_lock_or_empty(root).unwrap();
    assert!(
        find_ambiguities(root, &spec_loaded, &lock_loaded)
            .unwrap()
            .is_empty(),
        "no ambiguities after reassignment"
    );
}

// ── multi-agent: conflicts appear per-agent ───────────────────────────────────

#[test]
fn two_collections_two_agents_each_target_path_contested_independently() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let col_a = setup_collection(root, "alpha", &["shared"]);
    let col_b = setup_collection(root, "beta", &["shared"]);

    let spec = Spec {
        version: 1,
        // Two target agents → 2 contested paths (one per agent).
        agents: vec![AgentName::new("claude-code"), AgentName::new("pi")],
        defaults: Defaults::default(),
        rules: vec![
            Rule {
                id: "col-a".into(),
                source: Source::Local(col_a.clone()),
                schema_agent: AgentName::new("claude-code"),
                on_target_modified: None,
                kind: RuleKind::collection(),
            },
            Rule {
                id: "col-b".into(),
                source: Source::Local(col_b.clone()),
                schema_agent: AgentName::new("claude-code"),
                on_target_modified: None,
                kind: RuleKind::collection(),
            },
        ],
    };
    write_spec(root, &spec);

    let ambiguities = find_ambiguities(root, &spec, &empty_lock()).unwrap();

    // One contested path per agent.
    assert_eq!(
        ambiguities.len(),
        2,
        "expected 2 contested paths (one per agent); got {:?}",
        ambiguities.iter().map(|a| &a.path).collect::<Vec<_>>()
    );

    // Sync: provide one answer per contested path.
    let mut prompter = ScriptedPrompter {
        answers: vec![Some("col-a".into()), Some("col-a".into())],
        calls: vec![],
    };
    rtango::cmd::sync::exec_with_prompter(root, false, false, None, false, &mut prompter)
        .expect("sync with two-agent spec");

    assert_eq!(prompter.calls.len(), 2, "should have been prompted twice");

    assert!(root.join(".claude/skills/shared/SKILL.md").exists());
    assert!(root.join(".pi/skills/shared/SKILL.md").exists());
}

// ── Built-in rtango skill vs user rules ──────────────────────────────────────
//
// The built-in is injected outside spec.rules, so it never appears in
// find_ambiguities. A user rule claiming the same target path wins silently:
// the built-in is suppressed via the `produced` set in compute_plan.
//
// These tests check three source shapes that can produce a "rtango" skill:
//   1. kind: collection  (containing a "rtango" sub-rule)
//   2. kind: skill       (single skill file named "rtango")
//   3. kind: skill-set   (a set directory containing "rtango")
// And the cross-case: two such rules in the same spec.

/// Body text embedded in test skills named "rtango". Intentionally different
/// from the built-in content so tests can assert which version is on disk.
const CUSTOM_RTANGO_BODY: &str =
    "---\nname: rtango\ndescription: custom rtango override\n---\n\n# Custom rtango\n";

/// Write a skill directory `<root>/<rel>/SKILL.md` with `CUSTOM_RTANGO_BODY`.
fn write_rtango_skill(root: &Path, rel: &str) {
    let dir = root.join(rel);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("SKILL.md"), CUSTOM_RTANGO_BODY).unwrap();
}

// ── collection ────────────────────────────────────────────────────────────────

#[test]
fn collection_rtango_skill_suppresses_builtin_no_prompt() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // A collection that contains a "rtango" skill.
    let col = setup_collection(root, "alpha", &["rtango"]);
    fs::write(
        col.join("skills/rtango/SKILL.md"),
        CUSTOM_RTANGO_BODY,
    )
    .unwrap();

    let spec = Spec {
        version: 1,
        agents: vec![AgentName::new("claude-code")],
        defaults: Defaults::default(),
        rules: vec![Rule {
            id: "col-a".into(),
            source: Source::Local(col),
            schema_agent: AgentName::new("claude-code"),
            on_target_modified: None,
            kind: RuleKind::collection(),
        }],
    };
    write_spec(root, &spec);

    // Single collection — no contest with built-in, no prompt.
    let mut prompter = PanickingPrompter;
    rtango::cmd::sync::exec_with_prompter(root, false, false, None, false, &mut prompter)
        .expect("sync should succeed without prompts");

    let content = fs::read_to_string(root.join(".claude/skills/rtango/SKILL.md")).unwrap();
    assert!(
        content.contains("custom rtango override"),
        "collection's rtango should win over built-in; got:\n{content}"
    );
}

#[test]
fn two_collections_with_rtango_skill_conflict_builtin_not_a_candidate() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let col_a = setup_collection(root, "alpha", &["rtango"]);
    let col_b = setup_collection(root, "beta", &["rtango"]);
    // Give col-b a distinct body so we can verify winner content.
    fs::write(
        col_b.join("skills/rtango/SKILL.md"),
        "---\nname: rtango\ndescription: from-beta\n---\n\n# From beta\n",
    )
    .unwrap();

    let spec = two_collection_spec(root, &col_a, &col_b);
    write_spec(root, &spec);

    let ambiguities = find_ambiguities(root, &spec, &empty_lock()).unwrap();

    // The built-in must not appear as a candidate in any ambiguity.
    for a in &ambiguities {
        assert!(
            !a.candidates.iter().any(|c| c == BUILTIN_RTANGO_RULE_ID),
            "built-in rule id should never appear in ambiguity candidates; got {:?}",
            a.candidates
        );
    }
    // The two collection rules must contest the rtango target path.
    assert!(!ambiguities.is_empty(), "expected col-a vs col-b ambiguity");

    // Resolve in favour of col-b.
    let mut prompter = ScriptedPrompter {
        answers: vec![Some("col-b".into())],
        calls: vec![],
    };
    rtango::cmd::sync::exec_with_prompter(root, false, false, None, false, &mut prompter)
        .expect("sync after prompt");

    let content = fs::read_to_string(root.join(".claude/skills/rtango/SKILL.md")).unwrap();
    assert!(
        content.contains("from-beta"),
        "col-b's content should be on disk, not built-in; got:\n{content}"
    );
}

// ── single skill ─────────────────────────────────────────────────────────────

#[test]
fn single_skill_rule_named_rtango_suppresses_builtin() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // A single kind:skill rule pointing at a directory named "rtango".
    write_rtango_skill(root, ".claude/skills/rtango");

    let spec = Spec {
        version: 1,
        agents: vec![AgentName::new("claude-code")],
        defaults: Defaults::default(),
        rules: vec![Rule {
            id: "my-rtango".into(),
            source: Source::Local(PathBuf::from(".claude/skills/rtango")),
            schema_agent: AgentName::new("claude-code"),
            on_target_modified: None,
            kind: RuleKind::skill(),
        }],
    };
    write_spec(root, &spec);

    let mut prompter = PanickingPrompter;
    rtango::cmd::sync::exec_with_prompter(root, false, false, None, false, &mut prompter)
        .expect("sync should succeed without prompts");

    let content = fs::read_to_string(root.join(".claude/skills/rtango/SKILL.md")).unwrap();
    assert!(
        content.contains("custom rtango override"),
        "single-skill rule should win over built-in; got:\n{content}"
    );
}

#[test]
fn single_skill_rule_beats_collection_for_rtango_no_prompt() {
    // kind:skill is a "single" rule; kind:collection is a "set-like" rule.
    // The heuristic auto-resolves in favour of the single-file rule.
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    write_rtango_skill(root, ".claude/skills/rtango");
    let col = setup_collection(root, "alpha", &["rtango"]);

    let spec = Spec {
        version: 1,
        agents: vec![AgentName::new("claude-code")],
        defaults: Defaults::default(),
        rules: vec![
            Rule {
                id: "my-rtango".into(),
                source: Source::Local(PathBuf::from(".claude/skills/rtango")),
                schema_agent: AgentName::new("claude-code"),
                on_target_modified: None,
                kind: RuleKind::skill(),
            },
            Rule {
                id: "col-a".into(),
                source: Source::Local(col),
                schema_agent: AgentName::new("claude-code"),
                on_target_modified: None,
                kind: RuleKind::collection(),
            },
        ],
    };
    write_spec(root, &spec);

    // No ambiguities: skill beats collection via the specificity heuristic.
    let ambiguities = find_ambiguities(root, &spec, &empty_lock()).unwrap();
    assert!(
        ambiguities.is_empty(),
        "single-file skill should auto-beat collection; ambiguities: {:?}",
        ambiguities
    );

    let mut prompter = PanickingPrompter;
    rtango::cmd::sync::exec_with_prompter(root, false, false, None, false, &mut prompter)
        .expect("sync without prompts");

    let content = fs::read_to_string(root.join(".claude/skills/rtango/SKILL.md")).unwrap();
    assert!(
        content.contains("custom rtango override"),
        "single-skill rule content should be on disk; got:\n{content}"
    );
}

// ── skill-set ─────────────────────────────────────────────────────────────────

#[test]
fn skill_set_containing_rtango_suppresses_builtin_no_prompt() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // A skill-set directory that contains "rtango" as one of its skills.
    write_rtango_skill(root, ".claude/skills/rtango");

    let spec = Spec {
        version: 1,
        agents: vec![AgentName::new("claude-code")],
        defaults: Defaults::default(),
        rules: vec![Rule {
            id: "my-skills".into(),
            source: Source::Local(PathBuf::from(".claude/skills")),
            schema_agent: AgentName::new("claude-code"),
            on_target_modified: None,
            kind: RuleKind::skill_set(),
        }],
    };
    write_spec(root, &spec);

    let mut prompter = PanickingPrompter;
    rtango::cmd::sync::exec_with_prompter(root, false, false, None, false, &mut prompter)
        .expect("sync without prompts");

    let content = fs::read_to_string(root.join(".claude/skills/rtango/SKILL.md")).unwrap();
    assert!(
        content.contains("custom rtango override"),
        "skill-set's rtango should win over built-in; got:\n{content}"
    );
}

#[test]
fn skill_set_vs_collection_both_claiming_rtango_prompts() {
    // Both are "set-like" rules — no automatic resolution, must prompt.
    // The skill-set uses a copilot source dir (.github/skills) that is
    // distinct from the claude-code target dir (.claude/skills), so no
    // self-skip or pre-existing file conflicts.
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Skill-set source: .github/skills/rtango (copilot format).
    let github_skill = root.join(".github/skills/rtango");
    fs::create_dir_all(&github_skill).unwrap();
    fs::write(github_skill.join("SKILL.md"), CUSTOM_RTANGO_BODY).unwrap();

    // Collection source: also contains a "rtango" skill with distinct body.
    let col = setup_collection(root, "alpha", &["rtango"]);
    fs::write(
        col.join("skills/rtango/SKILL.md"),
        "---\nname: rtango\ndescription: from-collection\n---\n\n# From collection\n",
    )
    .unwrap();

    let spec = Spec {
        version: 1,
        agents: vec![AgentName::new("claude-code")],
        defaults: Defaults::default(),
        rules: vec![
            Rule {
                id: "my-skills".into(),
                // Source is the copilot skills directory.
                source: Source::Local(PathBuf::from(".github/skills")),
                schema_agent: AgentName::new("copilot"),
                on_target_modified: None,
                kind: RuleKind::skill_set(),
            },
            Rule {
                id: "col-a".into(),
                source: Source::Local(col),
                schema_agent: AgentName::new("claude-code"),
                on_target_modified: None,
                kind: RuleKind::collection(),
            },
        ],
    };
    write_spec(root, &spec);

    // Both are set-like — ambiguity expected on the claude-code target.
    let ambiguities = find_ambiguities(root, &spec, &empty_lock()).unwrap();
    assert!(
        !ambiguities.is_empty(),
        "skill-set vs collection should require a prompt"
    );
    // Built-in must not be a candidate.
    for a in &ambiguities {
        assert!(
            !a.candidates.iter().any(|c| c == BUILTIN_RTANGO_RULE_ID),
            "built-in should not appear in candidates; got {:?}",
            a.candidates
        );
    }

    // Resolve in favour of the collection.
    let mut prompter = ScriptedPrompter {
        answers: vec![Some("col-a".into())],
        calls: vec![],
    };
    rtango::cmd::sync::exec_with_prompter(root, false, false, None, false, &mut prompter)
        .expect("sync after prompt");

    let content = fs::read_to_string(root.join(".claude/skills/rtango/SKILL.md")).unwrap();
    assert!(
        content.contains("from-collection"),
        "collection's rtango should be on disk; got:\n{content}"
    );
}
