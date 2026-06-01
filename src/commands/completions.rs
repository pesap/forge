use anyhow::Result;
use clap::CommandFactory;
use clap_complete::aot::generate;

use crate::cli::{Cli, CompletionsArgs};

pub fn run(args: CompletionsArgs) -> Result<()> {
    let mut command = Cli::command();
    generate(args.shell, &mut command, "forge", &mut std::io::stdout());
    Ok(())
}
