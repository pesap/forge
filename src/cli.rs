use std::path::PathBuf;

use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "forge",
    version,
    about = "Create and upgrade project blueprints"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Create a new project from a blueprint
    New(NewArgs),
    /// Upgrade managed infrastructure files in an existing project
    Upgrade(UpgradeArgs),
    /// Commands for forge itself
    #[command(name = "self")]
    SelfCommand(SelfArgs),
    /// Diagnose local tool setup
    Doctor,
}

#[derive(Debug, Args)]
pub struct NewArgs {
    /// Destination path for generated project
    #[arg(long)]
    pub path: Option<PathBuf>,
    #[arg(long)]
    pub project_name: Option<String>,
    #[arg(long)]
    pub package_name: Option<String>,
    #[arg(long)]
    pub description: Option<String>,
    #[arg(long)]
    pub author_name: Option<String>,
    #[arg(long)]
    pub author_email: Option<String>,
    #[arg(long, default_value = "BSD-3-Clause")]
    pub license: String,
    #[arg(long, default_value = "3.11")]
    pub python_min: String,
    #[arg(long, action = ArgAction::Set, default_value_t = true)]
    pub docs: bool,
    #[arg(long, action = ArgAction::Set, default_value_t = true)]
    pub codecov: bool,
    #[arg(long, action = ArgAction::Set, default_value_t = false)]
    pub pypi_publish: bool,
    /// Create and push the GitHub repository after generation
    #[arg(long, action = ArgAction::Set, default_value_t = false)]
    pub github: bool,
    /// GitHub owner (user or org). Defaults to authenticated user.
    #[arg(long)]
    pub github_owner: Option<String>,
    #[arg(long, value_enum, default_value_t = GithubVisibility::Public)]
    pub github_visibility: GithubVisibility,
    /// Non-interactive mode
    #[arg(long, action = ArgAction::SetTrue)]
    pub yes: bool,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
pub enum GithubVisibility {
    Public,
    Private,
}

#[derive(Debug, Args)]
pub struct UpgradeArgs {
    /// Project path to upgrade
    #[arg(long, default_value = ".")]
    pub path: PathBuf,
}

#[derive(Debug, Args)]
pub struct SelfArgs {
    #[command(subcommand)]
    pub command: SelfCommand,
}

#[derive(Debug, Subcommand)]
pub enum SelfCommand {
    /// Update forge itself
    Update,
}
