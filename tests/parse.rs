use std::fs;
use std::path::Path;

use rtango::agent::permission::Permission;
use rtango::agent::{parse_standard_agents, parse_standard_skills, CopilotParser};

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

const SKILL_CONTENT: &str = "\
---
name: greet
description: says hello
allowed-tools: read
---
Hello world!
";

const AGENT_CONTENT: &str = "\
---
name: reviewer
description: reviews code
allowed-tools: read edit
---
You are a code reviewer.
";

// ── parse_standard_skills ──────────────────────────────────────

#[test]
fn skills_from_valid_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let skills_dir = tmp.path().join("skills");
    write_file(&skills_dir.join("greet/SKILL.md"), SKILL_CONTENT);

    let skills = parse_standard_skills(&skills_dir, &CopilotParser).unwrap();
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].name, "greet");
    assert_eq!(skills[0].front_matter.name.as_deref(), Some("greet"));
    assert_eq!(skills[0].front_matter.allowed_tools, vec![Permission::Read]);
    assert_eq!(skills[0].body, "Hello world!\n");
    assert_eq!(skills[0].dir, skills_dir.join("greet"));
    assert_eq!(skills[0].file, skills_dir.join("greet/SKILL.md"));
}

#[test]
fn skills_sorted_by_name() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("skills");
    write_file(&dir.join("beta/SKILL.md"), "---\nname: beta\n---\n");
    write_file(&dir.join("alpha/SKILL.md"), "---\nname: alpha\n---\n");

    let skills = parse_standard_skills(&dir, &CopilotParser).unwrap();
    assert_eq!(skills.len(), 2);
    assert_eq!(skills[0].name, "alpha");
    assert_eq!(skills[1].name, "beta");
}

#[test]
fn skills_empty_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("skills");
    fs::create_dir_all(&dir).unwrap();

    let skills = parse_standard_skills(&dir, &CopilotParser).unwrap();
    assert!(skills.is_empty());
}

#[test]
fn skills_missing_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("nonexistent");

    let skills = parse_standard_skills(&dir, &CopilotParser).unwrap();
    assert!(skills.is_empty());
}

#[test]
fn skills_ignores_dirs_without_skill_md() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("skills");
    write_file(&dir.join("valid/SKILL.md"), "---\nname: v\n---\n");
    write_file(&dir.join("invalid/README.md"), "not a skill");

    let skills = parse_standard_skills(&dir, &CopilotParser).unwrap();
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].name, "valid");
}

#[test]
fn skills_no_frontmatter_gets_default() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("skills");
    write_file(&dir.join("bare/SKILL.md"), "Just a body, no frontmatter.");

    let skills = parse_standard_skills(&dir, &CopilotParser).unwrap();
    assert_eq!(skills.len(), 1);
    assert!(skills[0].front_matter.name.is_none());
    assert!(skills[0].front_matter.allowed_tools.is_empty());
}

// ── parse_standard_agents ──────────────────────────────────────

#[test]
fn agents_from_valid_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let agents_dir = tmp.path().join("agents");
    write_file(&agents_dir.join("reviewer.agent.md"), AGENT_CONTENT);

    let agents = parse_standard_agents(&agents_dir, &CopilotParser).unwrap();
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].name, "reviewer");
    assert_eq!(agents[0].front_matter.description.as_deref(), Some("reviews code"));
    assert_eq!(
        agents[0].front_matter.allowed_tools,
        vec![Permission::Read, Permission::Edit],
    );
    assert_eq!(agents[0].body, "You are a code reviewer.\n");
}

#[test]
fn agents_sorted_by_name() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("agents");
    write_file(&dir.join("zeta.agent.md"), "---\nname: z\n---\n");
    write_file(&dir.join("alpha.agent.md"), "---\nname: a\n---\n");

    let agents = parse_standard_agents(&dir, &CopilotParser).unwrap();
    assert_eq!(agents[0].name, "alpha");
    assert_eq!(agents[1].name, "zeta");
}

#[test]
fn agents_missing_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let agents = parse_standard_agents(&tmp.path().join("nope"), &CopilotParser).unwrap();
    assert!(agents.is_empty());
}

#[test]
fn agents_ignores_non_agent_files() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("agents");
    write_file(&dir.join("ok.agent.md"), "---\nname: ok\n---\n");
    write_file(&dir.join("readme.md"), "not an agent");
    write_file(&dir.join("notes.txt"), "not an agent");

    let agents = parse_standard_agents(&dir, &CopilotParser).unwrap();
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].name, "ok");
}
