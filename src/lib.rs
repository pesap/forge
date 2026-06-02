pub mod blueprint;
pub mod cli;
pub mod commands;
pub mod errors;
mod install_source;
pub mod ui;

use anyhow::Result;
use clap::CommandFactory;

pub fn run(cli: cli::Cli) -> Result<()> {
    ui::configure(ui::UiOptions {
        color_mode: cli.color,
    });

    match cli.command {
        Some(cli::Commands::Blueprints(args)) => commands::blueprints::run(args),
        Some(cli::Commands::Components(args)) => commands::components::run(args),
        Some(cli::Commands::Completions(args)) => commands::completions::run(args),
        Some(cli::Commands::Init(args)) => commands::init::run(args),
        Some(cli::Commands::New(args)) => commands::new::run(args),
        Some(cli::Commands::Update(args)) => commands::update::run(args),
        Some(cli::Commands::SelfCommand(args)) => commands::self_update::run(args),
        Some(cli::Commands::Doctor(args)) => commands::doctor::run(args),
        None => {
            let mut command = cli::Cli::command();
            command.print_help()?;
            println!();
            Ok(())
        }
    }
}
