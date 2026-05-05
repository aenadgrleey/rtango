use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use rtango::agent::frontmatter::{FrontMatter, FrontMatterMapper, split_frontmatter};
use rtango::agent::permission::Permission;
use rtango::agent::write::{
    AgentsWriter, FrontMatterWriter, SkillsWriter, write_standard_agent, write_standard_skill,
};
use rtango::agent::{Agent, ClaudeCodeParser, CopilotParser, Skill};

// ── Copilot: format_permission ────────────────────────────────────

#[test]
fn copilot_format_known_permissions() {
    let w = CopilotParser;
    assert_eq!(w.format_permission(&Permission::Read), Some("read".into()));
    assert_eq!(
        w.format_permission(&Permission::Write),
        Some("write".into())
    );
    assert_eq!(w.format_permission(&Permission::Edit), Some("edit".into()));
    assert_eq!(
        w.format_permission(&Permission::Shell(None)),
        Some("shell".into())
    );
    assert_eq!(
        w.format_permission(&Permission::Shell(Some("npm *".into()))),
        Some("shell".into())
    );
    assert_eq!(w.format_permission(&Permission::Grep), Some("grep".into()));
    assert_eq!(w.format_permission(&Permission::Glob), Some("glob".into()));
    assert_eq!(
        w.format_permission(&Permission::WebFetch),
        Some("web_fetch".into())
    );
    assert_eq!(
        w.format_permission(&Permission::WebSearch),
        Some("web_search".into())
    );
}

#[test]
fn copilot_format_claude_only_permissions_are_none() {
    let w = CopilotParser;
    assert_eq!(w.format_permission(&Permission::NotebookEdit), None);
    assert_eq!(w.format_permission(&Permission::TodoWrite), None);
}

#[test]
fn copilot_format_other_permission() {
    let w = CopilotParser;
    assert_eq!(
        w.format_permission(&Permission::Other("custom_tool".into())),
        Some("custom_tool".into()),
    );
}

// ── Copilot: format_frontmatter ───────────────────────────────────

#[test]
fn copilot_format_full_frontmatter() {
    let w = CopilotParser;
    let fm = FrontMatter {
        name: Some("my-skill".into()),
        description: Some("does stuff".into()),
        allowed_tools: vec![Permission::Read, Permission::Write, Permission::Shell(None)],
        extra: BTreeMap::new(),
    };
    let yaml = w.format_frontmatter(&fm);
    // Parse it back and verify roundtrip
    let parsed = w.parse_frontmatter(&yaml).unwrap();
    assert_eq!(parsed.name.as_deref(), Some("my-skill"));
    assert_eq!(parsed.description.as_deref(), Some("does stuff"));
    assert_eq!(
        parsed.allowed_tools,
        vec![Permission::Read, Permission::Write, Permission::Shell(None)],
    );
}

#[test]
fn copilot_format_frontmatter_no_tools() {
    let w = CopilotParser;
    let fm = FrontMatter {
        name: Some("bare".into()),
        description: None,
        allowed_tools: vec![],
        extra: BTreeMap::new(),
    };
    let yaml = w.format_frontmatter(&fm);
    let parsed = w.parse_frontmatter(&yaml).unwrap();
    assert_eq!(parsed.name.as_deref(), Some("bare"));
    assert!(parsed.description.is_none());
    assert!(parsed.allowed_tools.is_empty());
}

#[test]
fn copilot_format_frontmatter_filters_unmappable_tools() {
    let w = CopilotParser;
    let fm = FrontMatter {
        name: Some("mixed".into()),
        description: None,
        allowed_tools: vec![Permission::Read, Permission::NotebookEdit, Permission::Grep],
        extra: BTreeMap::new(),
    };
    let yaml = w.format_frontmatter(&fm);
    let parsed = w.parse_frontmatter(&yaml).unwrap();
    // NotebookEdit should be dropped
    assert_eq!(
        parsed.allowed_tools,
        vec![Permission::Read, Permission::Grep]
    );
}

#[test]
fn copilot_format_frontmatter_preserves_extra() {
    let w = CopilotParser;
    let mut extra = BTreeMap::new();
    extra.insert(
        "custom_key".into(),
        serde_yml::Value::String("value".into()),
    );
    let fm = FrontMatter {
        name: Some("s".into()),
        description: None,
        allowed_tools: vec![],
        extra,
    };
    let yaml = w.format_frontmatter(&fm);
    let parsed = w.parse_frontmatter(&yaml).unwrap();
    assert!(parsed.extra.contains_key("custom_key"));
}

#[test]
fn copilot_format_empty_frontmatter() {
    let w = CopilotParser;
    let fm = FrontMatter::default();
    let yaml = w.format_frontmatter(&fm);
    assert!(yaml.is_empty());
}

// ── Claude Code: format_permission ────────────────────────────────

#[test]
fn claude_code_format_known_permissions() {
    let w = ClaudeCodeParser;
    assert_eq!(w.format_permission(&Permission::Read), Some("Read".into()));
    assert_eq!(
        w.format_permission(&Permission::Write),
        Some("Write".into())
    );
    assert_eq!(w.format_permission(&Permission::Edit), Some("Edit".into()));
    assert_eq!(
        w.format_permission(&Permission::Shell(None)),
        Some("Bash".into())
    );
    assert_eq!(
        w.format_permission(&Permission::Shell(Some("npm *".into()))),
        Some("Bash(npm *)".into()),
    );
    assert_eq!(w.format_permission(&Permission::Grep), Some("Grep".into()));
    assert_eq!(w.format_permission(&Permission::Glob), Some("Glob".into()));
    assert_eq!(
        w.format_permission(&Permission::WebFetch),
        Some("WebFetch".into())
    );
    assert_eq!(
        w.format_permission(&Permission::WebSearch),
        Some("WebSearch".into())
    );
    assert_eq!(
        w.format_permission(&Permission::NotebookEdit),
        Some("NotebookEdit".into())
    );
    assert_eq!(
        w.format_permission(&Permission::TodoWrite),
        Some("TodoWrite".into())
    );
}

#[test]
fn claude_code_format_other_permission() {
    let w = ClaudeCodeParser;
    assert_eq!(
        w.format_permission(&Permission::Other("CustomTool".into())),
        Some("CustomTool".into()),
    );
}

// ── Claude Code: format_frontmatter ───────────────────────────────

#[test]
fn claude_code_format_full_frontmatter() {
    let w = ClaudeCodeParser;
    let fm = FrontMatter {
        name: Some("my-skill".into()),
        description: Some("does stuff".into()),
        allowed_tools: vec![Permission::Read, Permission::Shell(Some("npm *".into()))],
        extra: BTreeMap::new(),
    };
    let yaml = w.format_frontmatter(&fm);
    let parsed = w.parse_frontmatter(&yaml).unwrap();
    assert_eq!(parsed.name.as_deref(), Some("my-skill"));
    assert_eq!(parsed.description.as_deref(), Some("does stuff"));
    assert_eq!(
        parsed.allowed_tools,
        vec![Permission::Read, Permission::Shell(Some("npm *".into()))],
    );
}

#[test]
fn claude_code_format_frontmatter_no_tools() {
    let w = ClaudeCodeParser;
    let fm = FrontMatter {
        name: Some("bare".into()),
        description: None,
        allowed_tools: vec![],
        extra: BTreeMap::new(),
    };
    let yaml = w.format_frontmatter(&fm);
    let parsed = w.parse_frontmatter(&yaml).unwrap();
    assert_eq!(parsed.name.as_deref(), Some("bare"));
    assert!(parsed.allowed_tools.is_empty());
}

#[test]
fn claude_code_format_empty_frontmatter() {
    let w = ClaudeCodeParser;
    let fm = FrontMatter::default();
    let yaml = w.format_frontmatter(&fm);
    assert!(yaml.is_empty());
}

// ── Roundtrip: parse → format → parse ─────────────────────────────

#[test]
fn copilot_roundtrip() {
    let p = CopilotParser;
    let yaml_in = "name: greet\ndescription: says hello\nallowed-tools: read write shell\n";
    let fm = p.parse_frontmatter(yaml_in).unwrap();
    let yaml_out = p.format_frontmatter(&fm);
    let fm2 = p.parse_frontmatter(&yaml_out).unwrap();
    assert_eq!(fm.name, fm2.name);
    assert_eq!(fm.description, fm2.description);
    assert_eq!(fm.allowed_tools, fm2.allowed_tools);
}

#[test]
fn claude_code_roundtrip() {
    let p = ClaudeCodeParser;
    let yaml_in = "name: greet\ndescription: says hello\nallowed-tools: Read Bash(npm *) Grep\n";
    let fm = p.parse_frontmatter(yaml_in).unwrap();
    let yaml_out = p.format_frontmatter(&fm);
    let fm2 = p.parse_frontmatter(&yaml_out).unwrap();
    assert_eq!(fm.name, fm2.name);
    assert_eq!(fm.description, fm2.description);
    assert_eq!(fm.allowed_tools, fm2.allowed_tools);
}

// ── write_standard_skill ──────────────────────────────────────────

fn make_skill(name: &str, body: &str, tools: Vec<Permission>) -> Skill {
    Skill {
        name: name.into(),
        dir: Default::default(),
        file: Default::default(),
        front_matter: FrontMatter {
            name: Some(name.into()),
            description: Some(format!("{name} skill")),
            allowed_tools: tools,
            extra: BTreeMap::new(),
        },
        body: body.into(),
    }
}

fn make_agent(name: &str, body: &str, tools: Vec<Permission>) -> Agent {
    Agent {
        name: name.into(),
        file: Default::default(),
        front_matter: FrontMatter {
            name: Some(name.into()),
            description: Some(format!("{name} agent")),
            allowed_tools: tools,
            extra: BTreeMap::new(),
        },
        body: body.into(),
    }
}

#[test]
fn write_skill_creates_correct_layout() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("skills");
    let skill = make_skill("greet", "Hello world!\n", vec![Permission::Read]);
    let path = write_standard_skill(&dir, &skill, &CopilotParser).unwrap();

    assert_eq!(path, dir.join("greet/SKILL.md"));
    assert!(path.is_file());

    // Verify content can be parsed back
    let content = fs::read_to_string(&path).unwrap();
    let (yaml, body) = split_frontmatter(&content);
    assert!(yaml.is_some());
    let fm = CopilotParser.parse_frontmatter(yaml.unwrap()).unwrap();
    assert_eq!(fm.name.as_deref(), Some("greet"));
    assert_eq!(fm.allowed_tools, vec![Permission::Read]);
    assert_eq!(body, "Hello world!\n");
}

#[test]
fn write_skill_no_frontmatter_writes_body_only() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("skills");
    let skill = Skill {
        name: "bare".into(),
        dir: Default::default(),
        file: Default::default(),
        front_matter: FrontMatter::default(),
        body: "Just a body.\n".into(),
    };
    let path = write_standard_skill(&dir, &skill, &CopilotParser).unwrap();
    let content = fs::read_to_string(&path).unwrap();
    assert_eq!(content, "Just a body.\n");
}

#[test]
fn write_skill_overwrites_existing() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("skills");
    let skill_v1 = make_skill("greet", "v1\n", vec![Permission::Read]);
    write_standard_skill(&dir, &skill_v1, &CopilotParser).unwrap();

    let skill_v2 = make_skill("greet", "v2\n", vec![Permission::Read, Permission::Write]);
    let path = write_standard_skill(&dir, &skill_v2, &CopilotParser).unwrap();

    let content = fs::read_to_string(&path).unwrap();
    let (yaml, body) = split_frontmatter(&content);
    let fm = CopilotParser.parse_frontmatter(yaml.unwrap()).unwrap();
    assert_eq!(fm.allowed_tools, vec![Permission::Read, Permission::Write]);
    assert_eq!(body, "v2\n");
}

// ── write_standard_agent ──────────────────────────────────────────

#[test]
fn write_agent_creates_correct_layout() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("agents");
    let agent = make_agent(
        "reviewer",
        "You review code.\n",
        vec![Permission::Read, Permission::Edit],
    );
    let path = write_standard_agent(&dir, &agent, &CopilotParser).unwrap();

    assert_eq!(path, dir.join("reviewer.agent.md"));
    assert!(path.is_file());

    let content = fs::read_to_string(&path).unwrap();
    let (yaml, body) = split_frontmatter(&content);
    let fm = CopilotParser.parse_frontmatter(yaml.unwrap()).unwrap();
    assert_eq!(fm.name.as_deref(), Some("reviewer"));
    assert_eq!(fm.allowed_tools, vec![Permission::Read, Permission::Edit]);
    assert_eq!(body, "You review code.\n");
}

#[test]
fn write_agent_no_frontmatter() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("agents");
    let agent = Agent {
        name: "bare".into(),
        file: Default::default(),
        front_matter: FrontMatter::default(),
        body: "Just a body.\n".into(),
    };
    let path = write_standard_agent(&dir, &agent, &CopilotParser).unwrap();
    let content = fs::read_to_string(&path).unwrap();
    assert_eq!(content, "Just a body.\n");
}

// ── SkillsWriter / AgentsWriter via agent parsers ─────────────────

#[test]
fn copilot_writer_uses_github_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let skill = make_skill("greet", "Hi\n", vec![Permission::Read]);
    let path = CopilotParser.write_skill(tmp.path(), &skill).unwrap();
    assert_eq!(path, tmp.path().join(".github/skills/greet/SKILL.md"));
}

#[test]
fn claude_code_writer_uses_claude_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let skill = make_skill("greet", "Hi\n", vec![Permission::Read]);
    let path = ClaudeCodeParser.write_skill(tmp.path(), &skill).unwrap();
    assert_eq!(path, tmp.path().join(".claude/skills/greet/SKILL.md"));
}

#[test]
fn copilot_agent_writer_uses_github_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let agent = make_agent("reviewer", "Review.\n", vec![Permission::Read]);
    let path = CopilotParser.write_agent(tmp.path(), &agent).unwrap();
    assert_eq!(path, tmp.path().join(".github/agents/reviewer.agent.md"));
}

#[test]
fn claude_code_agent_writer_uses_claude_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let agent = make_agent("reviewer", "Review.\n", vec![Permission::Read]);
    let path = ClaudeCodeParser.write_agent(tmp.path(), &agent).unwrap();
    assert_eq!(path, tmp.path().join(".claude/agents/reviewer.agent.md"));
}

// ── Cross-agent roundtrip: parse from copilot → write as claude-code ──

#[test]
fn cross_agent_skill_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();

    // Write a copilot skill
    let dir = tmp.path().join("source/.github/skills");
    let skill_file = dir.join("greet/SKILL.md");
    fs::create_dir_all(skill_file.parent().unwrap()).unwrap();
    fs::write(
        &skill_file,
        "---\nname: greet\ndescription: says hello\nallowed-tools: read write\n---\nHello!\n",
    )
    .unwrap();

    // Parse as copilot
    use rtango::agent::parse_standard_skills;
    let skills = parse_standard_skills(&dir, &CopilotParser).unwrap();
    assert_eq!(skills.len(), 1);

    // Write as claude-code
    let target = tmp.path().join("target");
    let path = ClaudeCodeParser.write_skill(&target, &skills[0]).unwrap();
    assert!(path.is_file());

    // Parse back as claude-code
    let content = fs::read_to_string(&path).unwrap();
    let (yaml, body) = split_frontmatter(&content);
    let fm = ClaudeCodeParser.parse_frontmatter(yaml.unwrap()).unwrap();
    assert_eq!(fm.name.as_deref(), Some("greet"));
    assert_eq!(fm.allowed_tools, vec![Permission::Read, Permission::Write]);
    assert_eq!(body, "Hello!\n");
}
