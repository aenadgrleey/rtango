//! Built-in skills bundled with the rtango binary.
//!
//! These are automatically injected into every sync so that any repo using
//! rtango gets usage instructions without the user adding a rule. Built-in
//! deployments are written to disk on every sync but are **not** recorded in
//! the lock — they never cause conflicts, orphan detection, or hash
//! mismatches.

use std::path::Path;

use crate::agent::Skill;
use crate::spec::AgentName;

use super::{ExpandedItem, ExpandedKind, RenderedTarget, hash_content, render_for_agent};

/// The rule id used for the built-in rtango skill.
pub const BUILTIN_RTANGO_RULE_ID: &str = "_builtin_rtango";

/// Raw content of the bundled rtango usage skill.
const RTANGO_SKILL_MD: &str = include_str!("../../skills/rtango/SKILL.md");

/// Return the built-in expanded items for the given root.
///
/// Currently this is just the rtango usage skill. The source file is a
/// phantom — it doesn't exist on disk, so we use a stable sentinel path.
pub fn builtin_expanded_items(root: &Path) -> Vec<ExpandedItem> {
    let content = RTANGO_SKILL_MD;
    let hash = hash_content(content);
    let (yaml, body) = crate::agent::frontmatter::split_frontmatter(content);
    let fm = crate::agent::frontmatter::FrontMatter::default();
    // Use a sentinel path that won't collide with real files.
    let sentinel_dir = root.join(".rtango").join("builtin").join("rtango");
    let sentinel_file = sentinel_dir.join("SKILL.md");

    // Parse frontmatter from the embedded content if present.
    let front_matter = if let Some(y) = yaml {
        let mapper = crate::agent::frontmatter_mapper(&AgentName::new("claude-code"))
            .expect("claude-code mapper always exists");
        mapper.parse_frontmatter(y).unwrap_or(fm)
    } else {
        fm
    };

    vec![ExpandedItem {
        rule_id: BUILTIN_RTANGO_RULE_ID.to_string(),
        source: crate::spec::Source::Local(sentinel_dir.clone()),
        source_content: content.to_string(),
        source_hash: hash,
        kind: ExpandedKind::Skill(Skill {
            name: "rtango".to_string(),
            dir: sentinel_dir,
            file: sentinel_file,
            front_matter,
            body: body.to_string(),
        }),
    }]
}

/// Render the built-in items for every agent in `agents`.
pub fn builtin_rendered_targets(root: &Path, agents: &[AgentName]) -> Vec<RenderedTarget> {
    let items = builtin_expanded_items(root);
    let mut targets = Vec::new();
    for item in &items {
        for agent in agents {
            if let Ok(rendered) =
                render_for_agent(root, item, &AgentName::new("claude-code"), agent)
            {
                targets.push(rendered);
            }
        }
    }
    targets
}
