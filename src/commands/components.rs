use anyhow::Result;
use serde::Serialize;

use crate::blueprint::components::{COMPONENT_REGISTRY, ManagedComponent};
use crate::blueprint::{BLUEPRINT_REGISTRY, BlueprintDefinition};
use crate::cli::ComponentsArgs;
use crate::ui;

pub fn run(args: ComponentsArgs) -> Result<()> {
    let components = filtered_components(args.blueprint);

    if args.json {
        print_json(&components)?;
        return Ok(());
    }

    ui::section("Available components");
    if components.is_empty() {
        ui::info(
            "status",
            "no optional components available for selected blueprint",
        );
        return Ok(());
    }

    for component in &components {
        let managed_component = component.id;
        ui::action(component.option_name, component.description);
        ui::info("option", component.option_name);
        ui::info(
            "managed files",
            managed_component.cleanup_paths().join(", "),
        );
        ui::info(
            "pre-commit hook",
            managed_component.pre_commit_hook().is_some(),
        );
        if let Some(format_command) = managed_component.format_command() {
            ui::info("format command", format_command);
        }
        if let Some(check_command) = managed_component.check_command() {
            ui::info("check command", check_command);
        }
        ui::info(
            "required tools",
            managed_component.required_tools().join(", "),
        );
        ui::info(
            "supported blueprints",
            supported_blueprints(managed_component)
                .into_iter()
                .map(|blueprint| blueprint.name)
                .collect::<Vec<_>>()
                .join(", "),
        );
        ui::info("enable", enable_command(component.option_name));
        ui::info("disable", disable_command(component.option_name));
    }
    ui::section("Enable a component");
    for component in &components {
        ui::next_step(&enable_command(component.option_name));
    }
    Ok(())
}

fn print_json(
    components: &[&'static crate::blueprint::components::ComponentDefinition],
) -> Result<()> {
    let components = components
        .iter()
        .map(|component| {
            let managed_component = component.id;
            ComponentInfo {
                name: component.option_name,
                option: component.option_name,
                description: component.description,
                managed_files: managed_component.cleanup_paths().to_vec(),
                required_tools: managed_component.required_tools().to_vec(),
                pre_commit_hook: managed_component.pre_commit_hook().is_some(),
                format_command: managed_component.format_command(),
                check_command: managed_component.check_command(),
                supported_blueprints: supported_blueprints(managed_component)
                    .into_iter()
                    .map(|blueprint| blueprint.name)
                    .collect(),
                enable_command: enable_command(component.option_name),
                disable_command: disable_command(component.option_name),
            }
        })
        .collect::<Vec<_>>();

    ui::json(ComponentRegistryReport {
        status_code: "ok",
        components,
    })
}

fn filtered_components(
    blueprint: Option<crate::blueprint::BlueprintName>,
) -> Vec<&'static crate::blueprint::components::ComponentDefinition> {
    COMPONENT_REGISTRY
        .iter()
        .filter(|component| match blueprint {
            Some(name) => name.supports_option(component.id.option()),
            None => true,
        })
        .collect()
}

fn supported_blueprints(component: ManagedComponent) -> Vec<&'static BlueprintDefinition> {
    BLUEPRINT_REGISTRY
        .iter()
        .filter(|blueprint| blueprint.id.supports_option(component.option()))
        .collect()
}

fn enable_command(option: &str) -> String {
    format!("forge sync --path . --set {option}=true")
}

fn disable_command(option: &str) -> String {
    format!("forge sync --path . --set {option}=false")
}

#[derive(Serialize)]
struct ComponentRegistryReport<'a> {
    status_code: &'static str,
    components: Vec<ComponentInfo<'a>>,
}

#[derive(Serialize)]
struct ComponentInfo<'a> {
    name: &'a str,
    option: &'a str,
    description: &'a str,
    managed_files: Vec<&'a str>,
    required_tools: Vec<&'a str>,
    pre_commit_hook: bool,
    format_command: Option<&'a str>,
    check_command: Option<&'a str>,
    supported_blueprints: Vec<&'a str>,
    enable_command: String,
    disable_command: String,
}
