use clap::Parser;

use rtango::cmd::{Cli, Command};

#[test]
fn sync_parses_ignore_fetch_failures_flag() {
    let cli = Cli::parse_from(["rtango", "sync", "--ignore-fetch-failures"]);
    match cli.command {
        Command::Sync {
            ignore_fetch_failures,
            ..
        } => assert!(ignore_fetch_failures),
        _ => panic!("expected sync command"),
    }
}

#[test]
fn status_parses_ignore_fetch_failures_flag() {
    let cli = Cli::parse_from(["rtango", "status", "--ignore-fetch-failures"]);
    match cli.command {
        Command::Status {
            ignore_fetch_failures,
            ..
        } => assert!(ignore_fetch_failures),
        _ => panic!("expected status command"),
    }
}

#[test]
fn wander_parses_ignore_fetch_failures_flag() {
    let cli = Cli::parse_from(["rtango", "wander", "--ignore-fetch-failures"]);
    match cli.command {
        Command::Wander {
            ignore_fetch_failures,
            ..
        } => assert!(ignore_fetch_failures),
        _ => panic!("expected wander command"),
    }
}

#[test]
fn global_sync_parses_spec_targets_and_options() {
    let cli = Cli::parse_from([
        "rtango",
        "global-sync",
        "--spec",
        "global.yaml",
        "claude-code",
        "--agent",
        "codex",
        "--force",
        "--prune",
    ]);
    match cli.command {
        Command::GlobalSync {
            spec,
            agents,
            agent_flags,
            force,
            prune,
            ..
        } => {
            assert_eq!(spec.unwrap().to_string_lossy(), "global.yaml");
            assert_eq!(agents, vec!["claude-code"]);
            assert_eq!(agent_flags, vec!["codex"]);
            assert!(force);
            assert!(prune);
        }
        _ => panic!("expected global-sync command"),
    }
}
