use std::path::{Path, PathBuf};

use tempfile::TempDir;

use rtango::cmd::add::AddOptions;
use rtango::spec::io::{load_spec, save_spec};
use rtango::spec::{AgentName, Defaults, GithubSource, Rule, RuleKind, Source, Spec};

fn write_spec(root: &Path, spec: &Spec) {
    save_spec(root, spec).unwrap();
}

fn empty_spec(agents: &[&str]) -> Spec {
    Spec {
        version: 1,
        agents: agents.iter().map(|n| AgentName::new(*n)).collect(),
        defaults: Defaults::default(),
        rules: vec![],
    }
}

// ── rtango add --collection-kind ─────────────────────────────────────────────

#[test]
fn add_local_collection_creates_collection_rule() {
    let tmp = TempDir::new().unwrap();
    write_spec(tmp.path(), &empty_spec(&["claude-code"]));

    let opts = AddOptions {
        id: "my-collection".into(),
        local: Some(PathBuf::from("./some/repo")),
        collection_kind: true,
        ..AddOptions::default()
    };

    rtango::cmd::add::exec(tmp.path(), opts).unwrap();

    let spec = load_spec(tmp.path()).unwrap();
    assert_eq!(spec.rules.len(), 1);
    let r = &spec.rules[0];
    assert_eq!(r.id, "my-collection");
    assert!(
        matches!(&r.kind, RuleKind::Collection { include, exclude, schema_override }
            if include.is_empty() && exclude.is_empty() && schema_override.is_none())
    );
    match &r.source {
        Source::Local(p) => assert_eq!(p, &PathBuf::from("./some/repo")),
        _ => panic!("expected Local source"),
    }
}

#[test]
fn add_github_collection_creates_collection_rule() {
    let tmp = TempDir::new().unwrap();
    write_spec(tmp.path(), &empty_spec(&["claude-code"]));

    let opts = AddOptions {
        id: "my-collection".into(),
        repo: Some("owner/repo@abc123".into()),
        collection_kind: true,
        ..AddOptions::default()
    };

    rtango::cmd::add::exec(tmp.path(), opts).unwrap();

    let spec = load_spec(tmp.path()).unwrap();
    let r = &spec.rules[0];
    match &r.source {
        Source::Github(g) => {
            assert_eq!(g.github, "owner/repo");
            assert_eq!(g.r#ref, "abc123");
        }
        _ => panic!("expected Github source"),
    }
    assert!(matches!(r.kind, RuleKind::Collection { .. }));
}

#[test]
fn add_collection_with_include() {
    let tmp = TempDir::new().unwrap();
    write_spec(tmp.path(), &empty_spec(&["claude-code"]));

    let opts = AddOptions {
        id: "my-collection".into(),
        local: Some(PathBuf::from("./repo")),
        collection_kind: true,
        include: vec!["skill-a".into()],
        ..AddOptions::default()
    };

    rtango::cmd::add::exec(tmp.path(), opts).unwrap();

    let spec = load_spec(tmp.path()).unwrap();
    match &spec.rules[0].kind {
        RuleKind::Collection { include, .. } => {
            assert_eq!(include, &["skill-a".to_string()]);
        }
        _ => panic!("expected Collection kind"),
    }
}

#[test]
fn add_collection_with_schema_override() {
    let tmp = TempDir::new().unwrap();
    write_spec(tmp.path(), &empty_spec(&["claude-code", "copilot"]));

    let opts = AddOptions {
        id: "my-collection".into(),
        local: Some(PathBuf::from("./repo")),
        collection_kind: true,
        schema: Some("copilot".into()),
        ..AddOptions::default()
    };

    rtango::cmd::add::exec(tmp.path(), opts).unwrap();

    let spec = load_spec(tmp.path()).unwrap();
    match &spec.rules[0].kind {
        RuleKind::Collection {
            schema_override, ..
        } => {
            assert_eq!(schema_override, &Some(AgentName::new("copilot")));
        }
        _ => panic!("expected Collection kind"),
    }
}

// ── spec.yaml round-trips ─────────────────────────────────────────────────────

#[test]
fn local_collection_round_trips_through_yaml() {
    let spec = Spec {
        version: 1,
        agents: vec![AgentName::new("claude-code")],
        defaults: Defaults::default(),
        rules: vec![Rule {
            id: "remote-skills".into(),
            source: Source::Local(PathBuf::from("../personal-ai-tools")),
            schema_agent: AgentName::new("claude-code"),
            targets: None,
            on_target_modified: None,
            kind: RuleKind::Collection {
                include: vec!["pi-android-sandbox".into()],
                exclude: vec![],
                schema_override: None,
            },
        }],
    };

    let yaml = serde_yml::to_string(&spec).unwrap();
    let back: Spec = serde_yml::from_str(&yaml).unwrap();
    assert_eq!(back.rules.len(), 1);
    match &back.rules[0].source {
        Source::Local(p) => assert_eq!(p, &PathBuf::from("../personal-ai-tools")),
        _ => panic!("expected Local source"),
    }
    match &back.rules[0].kind {
        RuleKind::Collection { include, .. } => {
            assert_eq!(include, &["pi-android-sandbox".to_string()]);
        }
        _ => panic!("expected Collection kind"),
    }
}

#[test]
fn github_collection_round_trips_through_yaml() {
    let spec = Spec {
        version: 1,
        agents: vec![AgentName::new("claude-code")],
        defaults: Defaults::default(),
        rules: vec![Rule {
            id: "remote-skills".into(),
            source: Source::Github(GithubSource {
                github: "aenadgrleey/personal-ai-tools".into(),
                r#ref: "main".into(),
                path: String::new(),
            }),
            schema_agent: AgentName::new("claude-code"),
            targets: None,
            on_target_modified: None,
            kind: RuleKind::Collection {
                include: vec!["pi-android-sandbox".into()],
                exclude: vec![],
                schema_override: None,
            },
        }],
    };

    let yaml = serde_yml::to_string(&spec).unwrap();
    let back: Spec = serde_yml::from_str(&yaml).unwrap();
    match &back.rules[0].source {
        Source::Github(g) => {
            assert_eq!(g.github, "aenadgrleey/personal-ai-tools");
            assert_eq!(g.r#ref, "main");
        }
        _ => panic!("expected Github source"),
    }
    match &back.rules[0].kind {
        RuleKind::Collection { include, .. } => {
            assert_eq!(include, &["pi-android-sandbox".to_string()]);
        }
        _ => panic!("expected Collection kind"),
    }
}

#[test]
fn collection_schema_override_round_trips_through_yaml() {
    let spec = Spec {
        version: 1,
        agents: vec![AgentName::new("claude-code")],
        defaults: Defaults::default(),
        rules: vec![Rule {
            id: "remote-skills".into(),
            source: Source::Local(PathBuf::from("../other-repo")),
            schema_agent: AgentName::new("claude-code"),
            targets: None,
            on_target_modified: None,
            kind: RuleKind::Collection {
                include: vec![],
                exclude: vec![],
                schema_override: Some(AgentName::new("copilot")),
            },
        }],
    };

    let yaml = serde_yml::to_string(&spec).unwrap();
    let back: Spec = serde_yml::from_str(&yaml).unwrap();
    match &back.rules[0].kind {
        RuleKind::Collection {
            schema_override, ..
        } => {
            assert_eq!(schema_override, &Some(AgentName::new("copilot")));
        }
        _ => panic!("expected Collection kind"),
    }
}
