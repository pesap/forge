use anyhow::Result;
use serde::Serialize;

use crate::blueprint::{BLUEPRINT_REGISTRY, BlueprintField, BlueprintName};
use crate::cli::BlueprintsArgs;
use crate::ui;

pub fn run(args: BlueprintsArgs) -> Result<()> {
    if args.json {
        print_json()?;
        return Ok(());
    }

    ui::section("Available blueprints");
    for blueprint in &BLUEPRINT_REGISTRY {
        ui::action(blueprint.name, blueprint.summary);
        ui::info(
            "fields",
            blueprint
                .fields
                .iter()
                .map(format_field)
                .collect::<Vec<_>>()
                .join(", "),
        );
        ui::info(
            "options",
            blueprint
                .options
                .iter()
                .map(|option| {
                    format!(
                        "{}={} ({})",
                        option.as_str(),
                        blueprint.id.option_default_enabled(*option),
                        option.description()
                    )
                })
                .collect::<Vec<_>>()
                .join(", "),
        );
        ui::info("managed", managed_highlights(blueprint.id).join(", "));
        ui::info("required tools", blueprint.required_tools.join(", "));
        ui::info("create", create_command(blueprint.name));
        ui::info("init", init_command(blueprint.name));
        ui::info("check", sync_check_command());
    }
    ui::section("Next steps");
    ui::next_step(&create_command("python-library"));
    ui::next_step(&init_command("python-library"));
    ui::next_step(sync_check_command());
    Ok(())
}

fn print_json() -> Result<()> {
    let blueprints = BLUEPRINT_REGISTRY
        .iter()
        .map(|blueprint| BlueprintInfo {
            name: blueprint.name,
            version: blueprint.version,
            summary: blueprint.summary,
            description: blueprint.description,
            create_command: create_command(blueprint.name),
            init_command: init_command(blueprint.name),
            sync_check_command: sync_check_command(),
            fields: blueprint.fields.to_vec(),
            required_tools: blueprint.required_tools.to_vec(),
            managed_highlights: managed_highlights(blueprint.id).to_vec(),
            options: blueprint
                .options
                .iter()
                .map(|option| BlueprintOption {
                    name: option.as_str(),
                    default_enabled: blueprint.id.option_default_enabled(*option),
                    description: option.description(),
                })
                .collect(),
        })
        .collect::<Vec<_>>();

    ui::json(BlueprintRegistryReport {
        status_code: "ok",
        blueprints,
    })
}

fn format_field(field: &BlueprintField) -> String {
    match field.default {
        Some(default) => format!("{} (default: {})", field.name, default),
        None if field.required => format!("{} (required)", field.name),
        None => format!("{} (optional)", field.name),
    }
}

fn create_command(blueprint_name: &str) -> String {
    format!("forge new --blueprint {blueprint_name} --yes ...")
}

fn init_command(blueprint_name: &str) -> String {
    format!("forge init --path . --blueprint {blueprint_name} --yes ...")
}

fn sync_check_command() -> &'static str {
    "forge sync --path . --check"
}

fn managed_highlights(blueprint: BlueprintName) -> &'static [&'static str] {
    match blueprint {
        BlueprintName::AnyProject => &[
            "pyproject.toml metadata",
            "justfile",
            "prek hooks",
            "AGENTS.md",
            "CLAUDE.md link",
            "docs (optional)",
            "github actions",
            "repository infrastructure only",
        ],
        BlueprintName::PythonLibrary => &[
            "pyproject.toml metadata",
            "justfile",
            "prek hooks",
            "AGENTS.md",
            "CLAUDE.md link",
            "docs (optional)",
            "github actions",
            "python package scaffolding",
        ],
        BlueprintName::RustLibrary => &[
            "pyproject.toml metadata",
            "justfile",
            "prek hooks",
            "AGENTS.md",
            "CLAUDE.md link",
            "docs (optional)",
            "github actions",
            "cargo package scaffolding",
        ],
    }
}

#[derive(Serialize)]
struct BlueprintRegistryReport<'a> {
    status_code: &'static str,
    blueprints: Vec<BlueprintInfo<'a>>,
}

#[derive(Serialize)]
struct BlueprintInfo<'a> {
    name: &'a str,
    version: &'a str,
    summary: &'a str,
    description: &'a str,
    create_command: String,
    init_command: String,
    sync_check_command: &'a str,
    fields: Vec<BlueprintField>,
    required_tools: Vec<&'a str>,
    managed_highlights: Vec<&'a str>,
    options: Vec<BlueprintOption<'a>>,
}

#[derive(Serialize)]
struct BlueprintOption<'a> {
    name: &'a str,
    default_enabled: bool,
    description: &'a str,
}
