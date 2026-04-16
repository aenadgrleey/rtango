mod cmd;

use clap::Parser;
use cmd::Cli;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    cmd::run(cli)
}
