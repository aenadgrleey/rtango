use rtango::agent::frontmatter::{join_frontmatter, split_frontmatter, tokenize_tools};

// ── split_frontmatter ──────────────────────────────────────────

#[test]
fn split_valid_frontmatter() {
    let input = "---\nname: foo\n---\nbody text\n";
    let (yaml, body) = split_frontmatter(input);
    assert_eq!(yaml, Some("name: foo"));
    assert_eq!(body, "body text\n");
}

#[test]
fn split_no_frontmatter() {
    let input = "just a body";
    let (yaml, body) = split_frontmatter(input);
    assert!(yaml.is_none());
    assert_eq!(body, input);
}

#[test]
fn split_empty_body() {
    let input = "---\nkey: val\n---\n";
    let (yaml, body) = split_frontmatter(input);
    assert_eq!(yaml, Some("key: val"));
    assert_eq!(body, "");
}

#[test]
fn split_missing_closing_fence() {
    let input = "---\nname: foo\nno closing fence";
    let (yaml, body) = split_frontmatter(input);
    assert!(yaml.is_none());
    assert_eq!(body, input);
}

#[test]
fn split_leading_whitespace() {
    let input = "  ---\nname: foo\n---\nbody\n";
    let (yaml, body) = split_frontmatter(input);
    assert_eq!(yaml, Some("name: foo"));
    assert_eq!(body, "body\n");
}

#[test]
fn split_triple_dash_in_body() {
    let input = "---\nname: foo\n---\nline1\n---\nline2\n";
    let (yaml, body) = split_frontmatter(input);
    assert_eq!(yaml, Some("name: foo"));
    assert_eq!(body, "line1\n---\nline2\n");
}

// ── join_frontmatter ───────────────────────────────────────────

#[test]
fn join_basic() {
    let result = join_frontmatter("name: foo\n", "body\n");
    assert_eq!(result, "---\nname: foo\n---\nbody\n");
}

#[test]
fn join_adds_trailing_newline_to_yaml() {
    let result = join_frontmatter("name: foo", "body");
    assert_eq!(result, "---\nname: foo\n---\nbody");
}

#[test]
fn join_split_roundtrip() {
    let yaml = "name: test\ndescription: hello";
    let body = "some body content\n";
    let joined = join_frontmatter(yaml, body);
    let (parsed_yaml, parsed_body) = split_frontmatter(&joined);
    assert_eq!(parsed_yaml, Some(yaml));
    assert_eq!(parsed_body, body);
}

// ── tokenize_tools ─────────────────────────────────────────────

#[test]
fn tokenize_simple() {
    assert_eq!(tokenize_tools("Read Write Edit"), vec!["Read", "Write", "Edit"]);
}

#[test]
fn tokenize_with_parens() {
    assert_eq!(tokenize_tools("Bash(npm*) Read"), vec!["Bash(npm*)", "Read"]);
}

#[test]
fn tokenize_nested_parens() {
    assert_eq!(tokenize_tools("X(a(b)) Y"), vec!["X(a(b))", "Y"]);
}

#[test]
fn tokenize_empty() {
    assert!(tokenize_tools("").is_empty());
    assert!(tokenize_tools("   ").is_empty());
}

#[test]
fn tokenize_extra_whitespace() {
    assert_eq!(tokenize_tools("  Read   Write  "), vec!["Read", "Write"]);
}

#[test]
fn tokenize_tabs() {
    assert_eq!(tokenize_tools("Read\tWrite"), vec!["Read", "Write"]);
}
