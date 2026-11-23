use rtango::agent::PiParser;
use rtango::agent::frontmatter::FrontMatterMapper;
use rtango::agent::permission::Permission;
use rtango::agent::write::FrontMatterWriter;

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
