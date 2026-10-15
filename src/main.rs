mod cli;

use clap::Parser;

fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    eprintln!("{cli:#?}");
    Ok(())
}
