use std::collections::BTreeMap;
use std::fs;

use rtango::agent::frontmatter::{FrontMatter, FrontMatterMapper};
use rtango::agent::permission::Permission;
use rtango::agent::write::{FrontMatterWriter, SkillsWriter};
use rtango::agent::{CursorParser, SkillsParser};

#[test]
fn parses_cursor_skills_and_preserves_cursor_frontmatter() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join(".cursor/skills/reviewer/SKILL.md");
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(
        &file,
        "---\nname: reviewer\ndescription: Reviews changes\npaths:\n  - src/**/*.rs\ndisable-model-invocation: true\n---\nReview the diff.\n",
    )
    .unwrap();

    let skills = CursorParser.parse_skills(tmp.path()).unwrap();
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].name, "reviewer");
    assert_eq!(
        skills[0].front_matter.description.as_deref(),
        Some("Reviews changes")
    );
    assert!(skills[0].front_matter.extra.contains_key("paths"));
    assert!(
        skills[0]
            .front_matter
            .extra
            .contains_key("disable-model-invocation")
    );
    assert_eq!(skills[0].body, "Review the diff.\n");
}

#[test]
fn cursor_does_not_emit_unsupported_allowed_tools() {
    let parser = CursorParser;
    let mut extra = BTreeMap::new();
    extra.insert(
        "paths".into(),
        serde_yml::Value::Sequence(vec![serde_yml::Value::String("src/**".into())]),
    );
    let fm = FrontMatter {
        name: Some("reviewer".into()),
        description: Some("Reviews changes".into()),
        allowed_tools: vec![Permission::Read, Permission::Shell(None)],
        extra,
    };

    let yaml = parser.format_frontmatter(&fm);
    assert!(yaml.contains("paths:"));
    assert!(!yaml.contains("allowed-tools:"));
    let parsed = parser.parse_frontmatter(&yaml).unwrap();
    assert!(parsed.allowed_tools.is_empty());
}

#[test]
fn cursor_writer_uses_cursor_skills_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let skill = rtango::agent::Skill {
        name: "reviewer".into(),
        dir: Default::default(),
        file: Default::default(),
        front_matter: FrontMatter {
            name: Some("reviewer".into()),
            description: Some("Reviews changes".into()),
            ..FrontMatter::default()
        },
        body: "Review the diff.\n".into(),
    };

    let path = CursorParser.write_skill(tmp.path(), &skill).unwrap();
    assert_eq!(path, tmp.path().join(".cursor/skills/reviewer/SKILL.md"));
}
