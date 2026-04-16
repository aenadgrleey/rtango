mod cmd;
mod engine;
mod error;
mod spec;

use clap::Parser;
use cmd::Cli;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    cmd::run(cli)
}
