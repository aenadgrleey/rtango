use std::collections::BTreeMap;

use super::permission::Permission;

#[derive(Debug, Clone, Default)]
pub struct FrontMatter {
    pub name: Option<String>,
    pub description: Option<String>,
    pub allowed_tools: Vec<Permission>,
    pub extra: BTreeMap<String, serde_yml::Value>,
}

pub trait FrontMatterMapper {
    fn parse_frontmatter(&self, yaml: &str) -> anyhow::Result<FrontMatter>;
    fn parse_permission(&self, token: &str) -> Permission;
}

pub fn split_frontmatter(content: &str) -> (Option<&str>, &str) {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return (None, content);
    }
    let after_open = &trimmed[3..];
    let after_open = match after_open.find('\n') {
        Some(i) => &after_open[i + 1..],
        None => return (None, content),
    };
    match after_open.find("\n---") {
        Some(i) => {
            let yaml = &after_open[..i];
            let rest = &after_open[i + 4..];
            let body = match rest.find('\n') {
                Some(j) => &rest[j + 1..],
                None => "",
            };
            (Some(yaml), body)
        }
        None => (None, content),
    }
}

pub fn join_frontmatter(yaml: &str, body: &str) -> String {
    let mut out = String::with_capacity(yaml.len() + body.len() + 10);
    out.push_str("---\n");
    out.push_str(yaml);
    if !yaml.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("---\n");
    out.push_str(body);
    out
}

pub fn tokenize_tools(s: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut depth = 0u32;
    for ch in s.chars() {
        match ch {
            '(' => { depth += 1; current.push(ch); }
            ')' => { depth = depth.saturating_sub(1); current.push(ch); }
            ' ' | '\t' if depth == 0 => {
                let t = current.trim().to_string();
                if !t.is_empty() { tokens.push(t); }
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    let t = current.trim().to_string();
    if !t.is_empty() { tokens.push(t); }
    tokens
}
