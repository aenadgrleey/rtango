use rtango::agent::frontmatter::FrontMatterMapper;
use rtango::agent::permission::Permission;
use rtango::agent::OpenCodeParser;

fn parser() -> OpenCodeParser {
    OpenCodeParser
}

#[test]
fn known_permissions() {
    let p = parser();
    assert_eq!(p.parse_permission("read"), Permission::Read);
    assert_eq!(p.parse_permission("write"), Permission::Write);
    assert_eq!(p.parse_permission("edit"), Permission::Edit);
    assert_eq!(p.parse_permission("shell"), Permission::Shell(None));
    assert_eq!(p.parse_permission("bash"), Permission::Shell(None));
    assert_eq!(p.parse_permission("grep"), Permission::Grep);
    assert_eq!(p.parse_permission("glob"), Permission::Glob);
    assert_eq!(p.parse_permission("webfetch"), Permission::WebFetch);
    assert_eq!(p.parse_permission("websearch"), Permission::WebSearch);
}

#[test]
fn unknown_permission() {
    assert_eq!(
        parser().parse_permission("custom_tool"),
        Permission::Other("custom_tool".into()),
    );
}

#[test]
fn parse_full_frontmatter() {
    let yaml = "name: my-skill\ndescription: does stuff\nallowed-tools: read write bash\n";
    let fm = parser().parse_frontmatter(yaml).unwrap();
    assert_eq!(fm.name.as_deref(), Some("my-skill"));
    assert_eq!(fm.description.as_deref(), Some("does stuff"));
    assert_eq!(
        fm.allowed_tools,
        vec![Permission::Read, Permission::Write, Permission::Shell(None)],
    );
    assert!(fm.extra.is_empty());
}

#[test]
fn opencode_specific_tokens() {
    let yaml = "name: s\nallowed-tools: webfetch websearch\n";
    let fm = parser().parse_frontmatter(yaml).unwrap();
    assert_eq!(
        fm.allowed_tools,
        vec![Permission::WebFetch, Permission::WebSearch],
    );
}

#[test]
fn parse_frontmatter_with_extras() {
    let yaml = "name: s\ncustom_key: value\n";
    let fm = parser().parse_frontmatter(yaml).unwrap();
    assert_eq!(fm.name.as_deref(), Some("s"));
    assert!(fm.extra.contains_key("custom_key"));
}
