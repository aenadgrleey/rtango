use rtango::agent::{frontmatter_mapper, skills_parser};
use rtango::spec::AgentName;

const KNOWN_AGENTS: &[&str] = &[
    "copilot",
    "cursor",
    "claude-code",
    "codex",
    "hermes",
    "pi",
    "opencode",
];

#[test]
fn skills_parser_known() {
    for name in KNOWN_AGENTS {
        assert!(
            skills_parser(&AgentName::new(*name)).is_some(),
            "missing skills_parser for {name}"
        );
    }
}

#[test]
fn skills_parser_unknown() {
    assert!(skills_parser(&AgentName::new("unknown")).is_none());
}

#[test]
fn frontmatter_mapper_known() {
    for name in KNOWN_AGENTS {
        assert!(
            frontmatter_mapper(&AgentName::new(*name)).is_some(),
            "missing frontmatter_mapper for {name}"
        );
    }
}

#[test]
fn frontmatter_mapper_unknown() {
    assert!(frontmatter_mapper(&AgentName::new("unknown")).is_none());
}
