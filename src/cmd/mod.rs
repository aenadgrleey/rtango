pub mod init;
pub mod status;
pub mod sync;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "rtango", version, about = "Package manager for agent skills and configuration files")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Scan project and create .rtango.yaml + .rtango.lock
    Init {
        /// Overwrite existing spec
        #[arg(short, long)]
        force: bool,

        /// Explicitly specify agents (repeatable)
        #[arg(short, long, value_name = "NAME")]
        agent: Vec<String>,

        /// Skip auto-detection, create minimal empty spec
        #[arg(short, long)]
        no_detect: bool,
    },

    /// Bring target files in sync with the spec
    Sync {
        /// Dry-run: exit 1 if out of sync (for CI)
        #[arg(short, long)]
        check: bool,

        /// Ignore on_target_modified: fail
        #[arg(short, long)]
        force: bool,

        /// Only process a single rule
        #[arg(short, long, value_name = "ID")]
        rule: Option<String>,

        /// Adopt existing target files on first sync
        #[arg(short, long)]
        adopt: bool,
    },

    /// Show sync plan without writing anything
    Status {
        /// Only show a single rule
        #[arg(short, long, value_name = "ID")]
        rule: Option<String>,

        /// Show up-to-date items too
        #[arg(short, long)]
        verbose: bool,
    },
}

pub fn run(cli: Cli) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    match cli.command {
        Command::Init { force, agent, no_detect } => init::exec(&root, force, agent, no_detect),
        Command::Sync { check, force, rule, adopt } => sync::exec(&root, check, force, rule, adopt),
        Command::Status { rule, verbose } => status::exec(&root, rule, verbose),
    }
}
