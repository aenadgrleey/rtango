use rtango::agent::ClaudeCodeParser;
use rtango::agent::frontmatter::FrontMatterMapper;
use rtango::agent::permission::Permission;

fn parser() -> ClaudeCodeParser {
    ClaudeCodeParser
}

// ── parse_permission ───────────────────────────────────────────

#[test]
fn known_permissions() {
    let p = parser();
    assert_eq!(p.parse_permission("Read"), Permission::Read);
    assert_eq!(p.parse_permission("Write"), Permission::Write);
    assert_eq!(p.parse_permission("Edit"), Permission::Edit);
    assert_eq!(p.parse_permission("MultiEdit"), Permission::Edit);
    assert_eq!(p.parse_permission("Bash"), Permission::Shell(None));
    assert_eq!(p.parse_permission("Grep"), Permission::Grep);
    assert_eq!(p.parse_permission("Glob"), Permission::Glob);
    assert_eq!(p.parse_permission("WebFetch"), Permission::WebFetch);
    assert_eq!(p.parse_permission("WebSearch"), Permission::WebSearch);
    assert_eq!(p.parse_permission("NotebookEdit"), Permission::NotebookEdit);
    assert_eq!(p.parse_permission("TodoWrite"), Permission::TodoWrite);
}

#[test]
fn obsolete_tokens_fall_through() {
    // LS, NotebookRead, TodoRead, MultiEdit are no longer valid Claude Code tools.
    // Legacy frontmatter still round-trips via Other (except MultiEdit, which is
    // remapped to Edit because the ergonomic intent is the same).
    let p = parser();
    assert_eq!(
        p.parse_permission("LS"),
        Permission::Other("LS".into()),
        "LS was removed from Claude Code (use Bash/Glob)"
    );
    assert_eq!(
        p.parse_permission("NotebookRead"),
        Permission::Other("NotebookRead".into()),
        "NotebookRead was merged into Read"
    );
    assert_eq!(
        p.parse_permission("TodoRead"),
        Permission::Other("TodoRead".into()),
        "TodoRead was removed; only TodoWrite remains"
    );
}

#[test]
fn bash_with_pattern() {
    assert_eq!(
        parser().parse_permission("Bash(npm*)"),
        Permission::Shell(Some("npm*".into())),
    );
}

#[test]
fn bash_with_complex_pattern() {
    assert_eq!(
        parser().parse_permission("Bash(cd /tmp && ls)"),
        Permission::Shell(Some("cd /tmp && ls".into())),
    );
}

#[test]
fn unknown_permission() {
    assert_eq!(
        parser().parse_permission("CustomTool"),
        Permission::Other("CustomTool".into()),
    );
}

// ── parse_frontmatter ──────────────────────────────────────────

#[test]
fn parse_full_frontmatter() {
    let yaml =
        "name: code-review\ndescription: reviews code\nallowed-tools: Read Bash(npm*) Grep\n";
    let fm = parser().parse_frontmatter(yaml).unwrap();
    assert_eq!(fm.name.as_deref(), Some("code-review"));
    assert_eq!(fm.description.as_deref(), Some("reviews code"));
    assert_eq!(
        fm.allowed_tools,
        vec![
            Permission::Read,
            Permission::Shell(Some("npm*".into())),
            Permission::Grep,
        ]
    );
}

#[test]
fn parse_frontmatter_preserves_extras() {
    let yaml = "name: s\nversion: 2\n";
    let fm = parser().parse_frontmatter(yaml).unwrap();
    assert!(fm.extra.contains_key("version"));
}

#[test]
fn parse_frontmatter_empty_tools() {
    let yaml = "description: no tools\n";
    let fm = parser().parse_frontmatter(yaml).unwrap();
    assert!(fm.allowed_tools.is_empty());
    assert!(fm.name.is_none());
}
