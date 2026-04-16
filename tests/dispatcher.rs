use rtango::agent::{agents_parser, frontmatter_mapper, skills_parser};
use rtango::spec::AgentName;

#[test]
fn skills_parser_known() {
    assert!(skills_parser(&AgentName::new("copilot")).is_some());
    assert!(skills_parser(&AgentName::new("claude-code")).is_some());
}

#[test]
fn skills_parser_unknown() {
    assert!(skills_parser(&AgentName::new("unknown")).is_none());
}

#[test]
fn agents_parser_known() {
    assert!(agents_parser(&AgentName::new("copilot")).is_some());
    assert!(agents_parser(&AgentName::new("claude-code")).is_some());
}

#[test]
fn agents_parser_unknown() {
    assert!(agents_parser(&AgentName::new("unknown")).is_none());
}

#[test]
fn frontmatter_mapper_known() {
    assert!(frontmatter_mapper(&AgentName::new("copilot")).is_some());
    assert!(frontmatter_mapper(&AgentName::new("claude-code")).is_some());
}

#[test]
fn frontmatter_mapper_unknown() {
    assert!(frontmatter_mapper(&AgentName::new("unknown")).is_none());
}
