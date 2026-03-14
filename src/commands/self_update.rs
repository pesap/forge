use anyhow::Result;

use crate::cli::{SelfArgs, SelfCommand};

pub fn run(args: SelfArgs) -> Result<()> {
    match args.command {
        SelfCommand::Update => {
            let executable = std::env::current_exe()?;
            let location = executable.display().to_string();

            if location.contains("homebrew") || location.contains("linuxbrew") {
                println!("forge appears to be installed with Homebrew. Run: brew upgrade forge");
                return Ok(());
            }

            println!("Self-update for this install method is not automated yet.");
            println!("Install the latest release manually, then replace: {location}");
        }
    }

    Ok(())
}
