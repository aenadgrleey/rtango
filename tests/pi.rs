use std::fs;

use rtango::agent::frontmatter::{FrontMatter, FrontMatterMapper};
use rtango::agent::permission::Permission;
use rtango::agent::write::{AgentsWriter, FrontMatterWriter};
use rtango::agent::{Agent, AgentsParser, Detector, PiParser};

fn parser() -> PiParser {
    PiParser
}

#[test]
fn parse_permission_is_passthrough() {
    let p = parser();
    assert_eq!(p.parse_permission("read"), Permission::Other("read".into()));
    assert_eq!(p.parse_permission("bash"), Permission::Other("bash".into()));
    assert_eq!(
        p.parse_permission("custom_tool"),
        Permission::Other("custom_tool".into()),
    );
}

#[test]
fn format_permission_emits_nothing() {
    let w = parser();
    assert_eq!(w.format_permission(&Permission::Read), None);
    assert_eq!(w.format_permission(&Permission::Write), None);
    assert_eq!(w.format_permission(&Permission::Shell(None)), None);
    assert_eq!(w.format_permission(&Permission::Other("x".into())), None);
}

#[test]
fn parse_full_frontmatter() {
    let yaml = "name: my-skill\ndescription: does stuff\nallowed-tools: read write bash\n";
    let fm = parser().parse_frontmatter(yaml).unwrap();
    assert_eq!(fm.name.as_deref(), Some("my-skill"));
    assert_eq!(fm.description.as_deref(), Some("does stuff"));
    assert_eq!(
        fm.allowed_tools,
        vec![
            Permission::Other("read".into()),
            Permission::Other("write".into()),
            Permission::Other("bash".into()),
        ],
    );
    assert!(fm.extra.is_empty());
}

#[test]
fn parse_frontmatter_with_extras() {
    let yaml = "name: s\ncustom_key: value\n";
    let fm = parser().parse_frontmatter(yaml).unwrap();
    assert_eq!(fm.name.as_deref(), Some("s"));
    assert!(fm.extra.contains_key("custom_key"));
}

#[test]
fn parse_pi_agents_from_flat_md_files() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join(".pi/agents/reviewer.md");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        &path,
        "---\nname: reviewer\ndescription: reviews code\nallowed-tools: read bash\n---\nYou review code.\n",
    )
    .unwrap();

    let agents = parser().parse_agents(tmp.path()).unwrap();
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].name, "reviewer");
    assert_eq!(
        agents[0].front_matter.description.as_deref(),
        Some("reviews code")
    );
    assert_eq!(
        agents[0].front_matter.allowed_tools,
        vec![
            Permission::Other("read".into()),
            Permission::Other("bash".into())
        ]
    );
    assert_eq!(agents[0].body, "You review code.\n");
}

#[test]
fn pi_writer_uses_flat_md_layout() {
    let tmp = tempfile::tempdir().unwrap();
    let agent = Agent {
        name: "reviewer".into(),
        file: Default::default(),
        front_matter: FrontMatter {
            name: Some("reviewer".into()),
            description: Some("reviews code".into()),
            allowed_tools: vec![Permission::Other("read".into())],
            extra: Default::default(),
        },
        body: "You review code.\n".into(),
    };

    let path = parser().write_agent(tmp.path(), &agent).unwrap();
    assert_eq!(path, tmp.path().join(".pi/agents/reviewer.md"));
    let content = fs::read_to_string(path).unwrap();
    assert!(content.contains("name: reviewer"));
    assert!(content.contains("You review code."));
}

#[test]
fn detect_pi_agents_from_flat_md_files() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join(".pi/agents/reviewer.md");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "---\nname: reviewer\n---\nbody\n").unwrap();

    let detected = parser().detect(tmp.path()).unwrap();
    assert_eq!(detected.name.as_str(), "pi");
    assert!(detected.sources.iter().any(|source| {
        source.id == "pi-agents" && source.path == std::path::Path::new(".pi/agents/")
    }));
}
