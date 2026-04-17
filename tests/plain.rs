use rtango::agent::PlainParser;
use rtango::agent::frontmatter::FrontMatterMapper;
use rtango::agent::permission::Permission;
use rtango::agent::write::FrontMatterWriter;

fn parser() -> PlainParser {
    PlainParser
}

#[test]
fn canonical_permissions_round_trip() {
    let p = parser();
    let cases = [
        Permission::Read,
        Permission::Write,
        Permission::Edit,
        Permission::Shell(None),
        Permission::Shell(Some("git *".into())),
        Permission::Grep,
        Permission::Glob,
        Permission::WebFetch,
        Permission::WebSearch,
        Permission::NotebookRead,
        Permission::NotebookEdit,
        Permission::TodoRead,
        Permission::TodoWrite,
        Permission::ListDir,
        Permission::Other("CustomTool".into()),
    ];
    for perm in cases {
        let token = p
            .format_permission(&perm)
            .expect("plain emits every permission");
        assert_eq!(
            p.parse_permission(&token),
            perm,
            "round-trip failed for {perm:?}"
        );
    }
}

#[test]
fn parse_frontmatter_preserves_permissions() {
    let yaml = "name: my-skill\ndescription: does stuff\nallowed-tools: Read Write Shell(git *) CustomTool\n";
    let fm = parser().parse_frontmatter(yaml).unwrap();
    assert_eq!(fm.name.as_deref(), Some("my-skill"));
    assert_eq!(fm.description.as_deref(), Some("does stuff"));
    assert_eq!(
        fm.allowed_tools,
        vec![
            Permission::Read,
            Permission::Write,
            Permission::Shell(Some("git *".into())),
            Permission::Other("CustomTool".into()),
        ],
    );
}
