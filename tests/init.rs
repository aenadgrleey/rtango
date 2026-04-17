use std::fs;
use std::path::Path;

use rtango::agent::{DetectedSource, SourceKind, detect_agents};
use rtango::spec::{AgentName, RuleKind};

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

// ── detect_agents ─────────────────────────────────────────────

#[test]
fn detect_empty_project() {
    let tmp = tempfile::tempdir().unwrap();
    let agents = detect_agents(tmp.path());
    assert!(agents.is_empty());
}

#[test]
fn detect_claude_code_by_skills_dir() {
    let tmp = tempfile::tempdir().unwrap();
    write_file(
        &tmp.path().join(".claude/skills/foo/SKILL.md"),
        "---\nname: foo\n---\nbody",
    );

    let agents = detect_agents(tmp.path());
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].name, AgentName::new("claude-code"));
    assert!(
        agents[0]
            .sources
            .iter()
            .any(|s| s.kind == SourceKind::SkillSet)
    );
}

#[test]
fn detect_claude_code_by_agents_dir() {
    let tmp = tempfile::tempdir().unwrap();
    write_file(
        &tmp.path().join(".claude/agents/reviewer.agent.md"),
        "---\nname: reviewer\n---\nbody",
    );

    let agents = detect_agents(tmp.path());
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].name, AgentName::new("claude-code"));
    assert!(
        agents[0]
            .sources
            .iter()
            .any(|s| s.kind == SourceKind::AgentSet)
    );
}

#[test]
fn detect_copilot_by_skills_dir() {
    let tmp = tempfile::tempdir().unwrap();
    write_file(
        &tmp.path().join(".github/skills/bar/SKILL.md"),
        "---\nname: bar\n---\nbody",
    );

    let agents = detect_agents(tmp.path());
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].name, AgentName::new("copilot"));
    assert!(
        agents[0]
            .sources
            .iter()
            .any(|s| s.kind == SourceKind::SkillSet)
    );
}

#[test]
fn detect_copilot_by_agents_dir() {
    let tmp = tempfile::tempdir().unwrap();
    write_file(
        &tmp.path().join(".github/agents/helper.agent.md"),
        "---\nname: helper\n---\nbody",
    );

    let agents = detect_agents(tmp.path());
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].name, AgentName::new("copilot"));
    assert!(
        agents[0]
            .sources
            .iter()
            .any(|s| s.kind == SourceKind::AgentSet)
    );
}

#[test]
fn detect_both_agents() {
    let tmp = tempfile::tempdir().unwrap();
    write_file(
        &tmp.path().join(".claude/skills/s1/SKILL.md"),
        "---\nname: s1\n---\n",
    );
    write_file(
        &tmp.path().join(".github/skills/s2/SKILL.md"),
        "---\nname: s2\n---\n",
    );

    let agents = detect_agents(tmp.path());
    assert_eq!(agents.len(), 2);

    let names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
    assert!(names.contains(&"claude-code"));
    assert!(names.contains(&"copilot"));
}

#[test]
fn detect_claude_code_multiple_sources() {
    let tmp = tempfile::tempdir().unwrap();
    write_file(
        &tmp.path().join(".claude/skills/s1/SKILL.md"),
        "---\nname: s1\n---\n",
    );
    write_file(
        &tmp.path().join(".claude/agents/a1.agent.md"),
        "---\nname: a1\n---\n",
    );

    let agents = detect_agents(tmp.path());
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].sources.len(), 2);

    let kinds: Vec<&SourceKind> = agents[0].sources.iter().map(|s| &s.kind).collect();
    assert!(kinds.contains(&&SourceKind::SkillSet));
    assert!(kinds.contains(&&SourceKind::AgentSet));
}

#[test]
fn detect_source_paths_are_relative() {
    let tmp = tempfile::tempdir().unwrap();
    write_file(
        &tmp.path().join(".claude/skills/foo/SKILL.md"),
        "---\nname: foo\n---\n",
    );

    let agents = detect_agents(tmp.path());
    let src = &agents[0].sources[0];
    assert!(!src.path.is_absolute());
    assert_eq!(src.path, Path::new(".claude/skills/"));
}

#[test]
fn detect_source_has_correct_id() {
    let tmp = tempfile::tempdir().unwrap();
    write_file(
        &tmp.path().join(".claude/skills/foo/SKILL.md"),
        "---\nname: foo\n---\n",
    );
    write_file(
        &tmp.path().join(".github/agents/a.agent.md"),
        "---\nname: a\n---\n",
    );

    let agents = detect_agents(tmp.path());

    for agent in &agents {
        for src in &agent.sources {
            assert!(!src.id.is_empty());
            assert!(
                src.id.contains(agent.name.as_str()),
                "id '{}' should contain agent name '{}'",
                src.id,
                agent.name
            );
        }
    }
}

#[test]
fn detect_empty_skills_dir_not_detected() {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir_all(tmp.path().join(".claude/skills")).unwrap();

    let agents = detect_agents(tmp.path());
    assert!(agents.is_empty());
}

#[test]
fn detect_empty_agents_dir_not_detected() {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir_all(tmp.path().join(".github/agents")).unwrap();

    let agents = detect_agents(tmp.path());
    assert!(agents.is_empty());
}

// ── DetectedSource → Rule conversion ──────────────────────────

#[test]
fn detected_source_to_rule_kind() {
    let skill_set = DetectedSource {
        id: "test-skills".into(),
        path: ".claude/skills/".into(),
        kind: SourceKind::SkillSet,
    };
    let rule_kind: RuleKind = skill_set.kind.into();
    assert!(matches!(rule_kind, RuleKind::SkillSet { .. }));

    let agent_set = DetectedSource {
        id: "test-agents".into(),
        path: ".github/agents/".into(),
        kind: SourceKind::AgentSet,
    };
    let rule_kind: RuleKind = agent_set.kind.into();
    assert!(matches!(rule_kind, RuleKind::AgentSet { .. }));
}

// ── init exec ─────────────────────────────────────────────────

mod exec {
    use super::*;
    use rtango::cmd::init;

    #[test]
    fn creates_spec_and_lock() {
        let tmp = tempfile::tempdir().unwrap();
        write_file(
            &tmp.path().join(".claude/skills/foo/SKILL.md"),
            "---\nname: foo\n---\n",
        );

        init::exec(tmp.path(), false, false).unwrap();

        assert!(tmp.path().join(".rtango/spec.yaml").exists());
        assert!(tmp.path().join(".rtango/lock.yaml").exists());
    }

    #[test]
    fn fails_if_spec_exists() {
        let tmp = tempfile::tempdir().unwrap();
        write_file(
            &tmp.path().join(".claude/skills/foo/SKILL.md"),
            "---\nname: foo\n---\n",
        );
        write_file(
            &tmp.path().join(".rtango/spec.yaml"),
            "version: 1\nagents: []\n",
        );

        let result = init::exec(tmp.path(), false, false);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn force_overwrites_existing_spec() {
        let tmp = tempfile::tempdir().unwrap();
        write_file(
            &tmp.path().join(".claude/skills/foo/SKILL.md"),
            "---\nname: foo\n---\n",
        );
        write_file(
            &tmp.path().join(".rtango/spec.yaml"),
            "version: 1\nagents: []\n",
        );

        init::exec(tmp.path(), true, false).unwrap();

        let content = fs::read_to_string(tmp.path().join(".rtango/spec.yaml")).unwrap();
        assert!(content.contains("claude-code"));
    }

    #[test]
    fn fails_if_no_agents_detected() {
        let tmp = tempfile::tempdir().unwrap();

        let result = init::exec(tmp.path(), false, false);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("no agents"));
    }

    #[test]
    fn no_detect_writes_empty_spec() {
        let tmp = tempfile::tempdir().unwrap();
        write_file(
            &tmp.path().join(".claude/skills/foo/SKILL.md"),
            "---\nname: foo\n---\n",
        );

        init::exec(tmp.path(), false, true).unwrap();

        let content = fs::read_to_string(tmp.path().join(".rtango/spec.yaml")).unwrap();
        assert!(content.contains("agents: []"));
        assert!(content.contains("rules: []"));
        assert!(!content.contains("claude-code"));
    }

    #[test]
    fn spec_contains_detected_rules() {
        let tmp = tempfile::tempdir().unwrap();
        write_file(
            &tmp.path().join(".claude/skills/foo/SKILL.md"),
            "---\nname: foo\n---\nbody",
        );

        init::exec(tmp.path(), false, false).unwrap();

        let content = fs::read_to_string(tmp.path().join(".rtango/spec.yaml")).unwrap();
        assert!(content.contains("claude-code"));
        assert!(content.contains(".claude/skills/"));
        assert!(content.contains("skill-set"));
    }

    #[test]
    fn lock_is_empty_on_init() {
        let tmp = tempfile::tempdir().unwrap();
        write_file(
            &tmp.path().join(".claude/skills/foo/SKILL.md"),
            "---\nname: foo\n---\n",
        );

        init::exec(tmp.path(), false, false).unwrap();

        let content = fs::read_to_string(tmp.path().join(".rtango/lock.yaml")).unwrap();
        assert!(content.contains("version: 1"));
        assert!(content.contains("deployments: []"));
    }

    #[test]
    fn spec_version_is_one() {
        let tmp = tempfile::tempdir().unwrap();
        write_file(
            &tmp.path().join(".claude/skills/foo/SKILL.md"),
            "---\nname: foo\n---\n",
        );

        init::exec(tmp.path(), false, false).unwrap();

        let content = fs::read_to_string(tmp.path().join(".rtango/spec.yaml")).unwrap();
        assert!(content.contains("version: 1"));
    }
}
