use std::fs;
use std::path::Path;

use rtango::agent::permission::Permission;
use rtango::agent::{CopilotParser, parse_standard_skills};

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
