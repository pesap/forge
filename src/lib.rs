pub mod blueprint;
pub mod cli;
pub mod commands;

use anyhow::Result;

pub fn run(cli: cli::Cli) -> Result<()> {
    match cli.command {
        cli::Commands::New(args) => commands::new::run(args),
        cli::Commands::Upgrade(args) => commands::upgrade::run(args),
        cli::Commands::SelfCommand(args) => commands::self_update::run(args),
        cli::Commands::Doctor => commands::doctor::run(),
    }
}
