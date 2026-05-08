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
