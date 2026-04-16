use clap::Parser;
use rtango::cmd::{Cli, run};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    run(cli)
}
