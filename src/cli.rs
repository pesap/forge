use std::path::PathBuf;

use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;

use crate::blueprint::BlueprintName;

#[derive(Debug, Parser)]
#[command(
    name = "forge",
    version,
    about = "Create and update project blueprints",
    after_help = "Quickstart:\n  forge blueprints\n  forge new --blueprint python-library --project-name my-lib --description \"My library\" --yes\n  forge update --path ./my-lib --check"
)]
pub struct Cli {
    /// Colorized terminal output policy
    #[arg(long, global = true, value_enum, default_value_t = ColorMode::Auto)]
    pub color: ColorMode,
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum ColorMode {
    #[default]
    Auto,
    Always,
    Never,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// List available project blueprints
    #[command(alias = "bp")]
    Blueprints(BlueprintsArgs),
    /// List reusable optional components
    Components(ComponentsArgs),
    /// Generate shell completion scripts
    Completions(CompletionsArgs),
    /// Initialize Forge-managed infrastructure in an existing repository
    #[command(
        after_help = "Examples:\n  forge init --blueprint python-library --project-name my-lib --description \"My library\" --yes\n  forge init --blueprint any-project --path . --project-name infra --description \"Shared repo infrastructure\" --dry-run"
    )]
    Init(InitArgs),
    /// Create a new project from a blueprint
    #[command(
        after_help = "Examples:\n  forge new --blueprint python-library --project-name my-lib --description \"My library\" --author-name \"Ada Lovelace\" --author-email ada@example.com --yes\n  forge new --blueprint python-library --project-name my-lib --description \"My library\" --author-name \"Ada Lovelace\" --author-email ada@example.com --yes --dry-run --diff\n  forge new --blueprint rust-library --project-name tools --package-name tools --description \"Internal tools\" --author-name \"Ada Lovelace\" --author-email ada@example.com --yes\n  forge new --blueprint any-project --project-name infra --description \"Shared repo infrastructure\" --yes"
    )]
    New(NewArgs),
    /// Update managed infrastructure files in an existing project
    #[command(
        after_help = "Examples:\n  forge update --path . --yes\n  forge update --path . --dry-run\n  forge update --path . --check\n  forge update --path . --set prettier=true --yes"
    )]
    Update(UpdateArgs),
    /// Commands for forge itself
    #[command(name = "self")]
    SelfCommand(SelfArgs),
    /// Diagnose local tool setup
    Doctor(DoctorArgs),
}

#[derive(Debug, Args)]
pub struct BlueprintsArgs {
    /// Emit machine-readable JSON
    #[arg(long, action = ArgAction::SetTrue)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct ComponentsArgs {
    /// Limit components to a specific blueprint
    #[arg(long, value_enum)]
    pub blueprint: Option<BlueprintName>,
    /// Emit machine-readable JSON
    #[arg(long, action = ArgAction::SetTrue)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct DoctorArgs {
    /// Limit required tool checks to one blueprint
    #[arg(long, value_enum)]
    pub blueprint: Option<BlueprintName>,
    /// Limit required tool checks to the Forge blueprint detected at this project path
    #[arg(long)]
    pub path: Option<PathBuf>,
    /// Emit machine-readable JSON
    #[arg(long, action = ArgAction::SetTrue)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct CompletionsArgs {
    /// Shell to generate completions for
    #[arg(value_enum)]
    pub shell: Shell,
}

#[derive(Clone, Debug, Args)]
pub struct InitArgs {
    /// Blueprint to initialize
    #[arg(long, value_enum)]
    pub blueprint: Option<BlueprintName>,
    /// Existing repository path to initialize
    #[arg(long, default_value = ".")]
    pub path: PathBuf,
    /// Project distribution name, for example my-library
    #[arg(long)]
    pub project_name: Option<String>,
    /// Python package or Rust crate name. Defaults from the project name for Python and Rust.
    #[arg(long)]
    pub package_name: Option<String>,
    /// Short project description
    #[arg(long)]
    pub description: Option<String>,
    /// Package author name
    #[arg(long)]
    pub author_name: Option<String>,
    /// Package author email
    #[arg(long)]
    pub author_email: Option<String>,
    /// SPDX license identifier. Defaults to BSD-3-Clause for library blueprints.
    #[arg(long)]
    pub license: Option<String>,
    /// Minimum supported Python version for python-library projects as major.minor. Defaults to 3.11.
    #[arg(long)]
    pub python_min: Option<String>,
    /// Generate MkDocs documentation. Accepts --docs, --docs=true, or --docs=false.
    #[arg(
        long,
        value_name = "BOOL",
        action = ArgAction::Set,
        num_args = 0..=1,
        default_missing_value = "true",
        default_value_t = true
    )]
    pub docs: bool,
    /// Enable Codecov upload for python-library CI. Accepts --codecov, --codecov=true, or --codecov=false.
    #[arg(
        long,
        value_name = "BOOL",
        action = ArgAction::Set,
        num_args = 0..=1,
        default_missing_value = "true"
    )]
    pub codecov: Option<bool>,
    /// Add trusted PyPI publishing workflow. Accepts --pypi-publish, --pypi-publish=true, or --pypi-publish=false.
    #[arg(
        long,
        value_name = "BOOL",
        action = ArgAction::Set,
        num_args = 0..=1,
        default_missing_value = "true"
    )]
    pub pypi_publish: Option<bool>,
    /// Add Prettier formatting for JSON, YAML, and Markdown files. Accepts --prettier, --prettier=true, or --prettier=false.
    #[arg(
        long,
        value_name = "BOOL",
        action = ArgAction::Set,
        num_args = 0..=1,
        default_missing_value = "true",
        default_value_t = false
    )]
    pub prettier: bool,
    /// Add EditorConfig baseline for cross-editor whitespace consistency. Accepts --editorconfig, --editorconfig=true, or --editorconfig=false.
    #[arg(
        long,
        value_name = "BOOL",
        action = ArgAction::Set,
        num_args = 0..=1,
        default_missing_value = "true",
        default_value_t = false
    )]
    pub editorconfig: bool,
    /// Add markdownlint checks for Markdown files. Accepts --markdownlint, --markdownlint=true, or --markdownlint=false.
    #[arg(
        long,
        value_name = "BOOL",
        action = ArgAction::Set,
        num_args = 0..=1,
        default_missing_value = "true",
        default_value_t = false
    )]
    pub markdownlint: bool,
    /// Emit a machine-readable JSON init report
    #[arg(long, action = ArgAction::SetTrue)]
    pub json: bool,
    /// Preview generated infrastructure without writing files
    #[arg(long, action = ArgAction::SetTrue)]
    pub dry_run: bool,
    /// Show a text diff for managed file changes in human output
    #[arg(long, action = ArgAction::SetTrue, conflicts_with = "json")]
    pub diff: bool,
    /// Overwrite existing files that are selected as Forge-managed infrastructure
    #[arg(long, action = ArgAction::SetTrue)]
    pub force: bool,
    /// Non-interactive mode
    #[arg(long, action = ArgAction::SetTrue)]
    pub yes: bool,
}

#[derive(Clone, Debug, Args)]
pub struct NewArgs {
    /// Blueprint to generate
    #[arg(long, value_enum)]
    pub blueprint: Option<BlueprintName>,
    /// Destination path for generated project
    #[arg(long)]
    pub path: Option<PathBuf>,
    /// Project distribution name, for example my-library
    #[arg(long)]
    pub project_name: Option<String>,
    /// Python package or Rust crate name. Defaults from the project name for Python and Rust.
    #[arg(long)]
    pub package_name: Option<String>,
    /// Short project description
    #[arg(long)]
    pub description: Option<String>,
    /// Package author name
    #[arg(long)]
    pub author_name: Option<String>,
    /// Package author email
    #[arg(long)]
    pub author_email: Option<String>,
    /// SPDX license identifier. Defaults to BSD-3-Clause for library blueprints.
    #[arg(long)]
    pub license: Option<String>,
    /// Minimum supported Python version for python-library projects as major.minor. Defaults to 3.11.
    #[arg(long)]
    pub python_min: Option<String>,
    /// Generate MkDocs documentation. Accepts --docs, --docs=true, or --docs=false.
    #[arg(
        long,
        value_name = "BOOL",
        action = ArgAction::Set,
        num_args = 0..=1,
        default_missing_value = "true",
        default_value_t = true
    )]
    pub docs: bool,
    /// Enable Codecov upload for python-library CI. Accepts --codecov, --codecov=true, or --codecov=false.
    #[arg(
        long,
        value_name = "BOOL",
        action = ArgAction::Set,
        num_args = 0..=1,
        default_missing_value = "true"
    )]
    pub codecov: Option<bool>,
    /// Add trusted PyPI publishing workflow. Accepts --pypi-publish, --pypi-publish=true, or --pypi-publish=false.
    #[arg(
        long,
        value_name = "BOOL",
        action = ArgAction::Set,
        num_args = 0..=1,
        default_missing_value = "true"
    )]
    pub pypi_publish: Option<bool>,
    /// Add Prettier formatting for JSON, YAML, and Markdown files. Accepts --prettier, --prettier=true, or --prettier=false.
    #[arg(
        long,
        value_name = "BOOL",
        action = ArgAction::Set,
        num_args = 0..=1,
        default_missing_value = "true",
        default_value_t = false
    )]
    pub prettier: bool,
    /// Add EditorConfig baseline for cross-editor whitespace consistency. Accepts --editorconfig, --editorconfig=true, or --editorconfig=false.
    #[arg(
        long,
        value_name = "BOOL",
        action = ArgAction::Set,
        num_args = 0..=1,
        default_missing_value = "true",
        default_value_t = false
    )]
    pub editorconfig: bool,
    /// Add markdownlint checks for Markdown files. Accepts --markdownlint, --markdownlint=true, or --markdownlint=false.
    #[arg(
        long,
        value_name = "BOOL",
        action = ArgAction::Set,
        num_args = 0..=1,
        default_missing_value = "true",
        default_value_t = false
    )]
    pub markdownlint: bool,
    /// Create and push the GitHub repository after generation
    #[arg(
        long,
        value_name = "BOOL",
        action = ArgAction::Set,
        num_args = 0..=1,
        default_missing_value = "true",
        default_value_t = false
    )]
    pub github: bool,
    /// GitHub owner (user or org). Defaults to authenticated user.
    #[arg(long)]
    pub github_owner: Option<String>,
    /// GitHub repository visibility. Defaults to public when --github is enabled.
    #[arg(long, value_enum)]
    pub github_visibility: Option<GithubVisibility>,
    /// Emit a machine-readable JSON creation report
    #[arg(long, action = ArgAction::SetTrue)]
    pub json: bool,
    /// Preview generated files without writing or initializing git
    #[arg(long, action = ArgAction::SetTrue)]
    pub dry_run: bool,
    /// Show a text diff for generated files in human dry-run output
    #[arg(long, action = ArgAction::SetTrue, conflicts_with = "json")]
    pub diff: bool,
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
pub struct UpdateArgs {
    /// Project path to update
    #[arg(long, default_value = ".")]
    pub path: PathBuf,
    /// Preview managed infrastructure changes without writing files
    #[arg(long, action = ArgAction::SetTrue)]
    pub dry_run: bool,
    /// Show a text diff for managed file changes in human output
    #[arg(long, action = ArgAction::SetTrue, conflicts_with = "json")]
    pub diff: bool,
    /// Fail if managed infrastructure is not up to date
    #[arg(long, action = ArgAction::SetTrue)]
    pub check: bool,
    /// Override a managed option, for example --set prettier=true
    #[arg(long = "set", value_name = "OPTION=BOOL")]
    pub set: Vec<String>,
    /// Emit a machine-readable JSON update report
    #[arg(long, action = ArgAction::SetTrue)]
    pub json: bool,
    /// Non-interactive mode
    #[arg(long, action = ArgAction::SetTrue)]
    pub yes: bool,
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
