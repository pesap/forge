use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::blueprint::components::ManagedComponent;
use crate::blueprint::{
    BLUEPRINT_REGISTRY, BlueprintName, detect_blueprint_metadata_from_pyproject,
    managed_option_enabled, validate_managed_options_from_metadata,
};
use crate::cli::DoctorArgs;
use crate::errors::{ErrorCode, coded_error};
use crate::ui;

pub fn run(args: DoctorArgs) -> Result<()> {
    let scope = DoctorScope::resolve(args.blueprint, args.path)?;
    let report = build_report(&scope);

    if args.json {
        ui::json(&report)?;
    } else {
        print_human_report(&report);
    }

    if report.ok {
        Ok(())
    } else {
        Err(coded_error(
            ErrorCode::Env,
            format!(
                "required tools are missing: {}",
                report.missing_required.join(", ")
            ),
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DoctorScope {
    blueprint: Option<BlueprintName>,
    blueprint_version: Option<String>,
    path: Option<PathBuf>,
    enabled_components: Vec<ManagedComponent>,
}

impl DoctorScope {
    fn resolve(blueprint: Option<BlueprintName>, path: Option<PathBuf>) -> Result<Self> {
        if blueprint.is_some() && path.is_some() {
            return Err(coded_error(
                ErrorCode::Input,
                "--blueprint cannot be used with --path",
            ));
        }

        let Some(path) = path else {
            return Ok(Self {
                blueprint,
                blueprint_version: blueprint.map(|name| name.version().to_string()),
                path: None,
                enabled_components: ManagedComponent::ALL.to_vec(),
            });
        };
        let root = path.canonicalize().unwrap_or(path);
        ensure_path_scope_is_directory(&root)?;
        let pyproject = read_pyproject_for_path_scope(&root)?;
        ensure_forge_metadata_for_path_scope(&root, &pyproject)?;
        let metadata = detect_blueprint_metadata_from_pyproject(&pyproject)
            .with_context(|| format!("failed to detect Forge blueprint at {}", root.display()))?;
        metadata
            .name
            .render_managed_files_from_pyproject(&pyproject)
            .with_context(|| format!("failed to validate Forge metadata at {}", root.display()))?;
        let enabled_components = enabled_components_from_pyproject(metadata.name, &pyproject)
            .with_context(|| format!("failed to validate Forge metadata at {}", root.display()))?;
        let blueprint_version = Some(
            metadata
                .version
                .expect("blueprint version metadata is required"),
        );

        Ok(Self {
            blueprint: Some(metadata.name),
            blueprint_version,
            path: Some(root),
            enabled_components,
        })
    }

    fn path_display(&self) -> Option<String> {
        self.path.as_ref().map(|path| path.display().to_string())
    }

    fn rerun_command(&self) -> String {
        if let Some(path) = self.path_display() {
            format!("forge doctor --path {}", ui::shell_arg(path))
        } else if let Some(blueprint) = self.blueprint {
            format!("forge doctor --blueprint {}", blueprint.as_str())
        } else {
            "forge doctor".to_string()
        }
    }

    fn scope_code(&self) -> DoctorScopeCode {
        if self.path.is_some() {
            DoctorScopeCode::Path
        } else if self.blueprint.is_some() {
            DoctorScopeCode::Blueprint
        } else {
            DoctorScopeCode::Global
        }
    }

    fn scope_label(&self) -> &'static str {
        if self.path.is_some() {
            "path"
        } else if let Some(blueprint) = self.blueprint {
            blueprint.as_str()
        } else {
            "global"
        }
    }
}

fn ensure_path_scope_is_directory(root: &Path) -> Result<()> {
    let metadata = fs::metadata(root)
        .with_context(|| format!("failed to read repository path {}", root.display()))?;
    if metadata.is_dir() {
        return Ok(());
    }

    Err(coded_error(
        ErrorCode::Env,
        format!(
            "repository path is not a directory: {}; choose an existing Forge-managed repository directory",
            root.display()
        ),
    ))
}

fn read_pyproject_for_path_scope(root: &Path) -> Result<String> {
    let pyproject_path = root.join("pyproject.toml");
    match fs::read_to_string(&pyproject_path) {
        Ok(pyproject) => Ok(pyproject),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Err(coded_error(
            ErrorCode::Env,
            format!(
                "missing Forge metadata at {}; use `forge init --path {}` to adopt this repository or `forge new --path {}` to create a new project",
                pyproject_path.display(),
                ui::shell_arg(root.display().to_string()),
                ui::shell_arg(root.display().to_string())
            ),
        )),
        Err(error) => {
            Err(error).with_context(|| format!("failed to read {}", pyproject_path.display()))
        }
    }
}

fn ensure_forge_metadata_for_path_scope(root: &Path, pyproject: &str) -> Result<()> {
    let Ok(parsed) = toml::from_str::<toml::Value>(pyproject) else {
        return Ok(());
    };

    let has_forge_metadata = parsed
        .get("tool")
        .and_then(toml::Value::as_table)
        .and_then(|tool| tool.get("forge"))
        .is_some();
    if !has_forge_metadata {
        return Err(coded_error(
            ErrorCode::Env,
            format!(
                "missing [tool.forge] metadata; use `forge init --path {}` to adopt this repository before running doctor",
                ui::shell_arg(root.display().to_string())
            ),
        ));
    }

    Ok(())
}

fn build_report(scope: &DoctorScope) -> DoctorReport {
    let mut tools = required_tool_requirements(scope.blueprint)
        .into_iter()
        .map(|tool| check_required_tool(tool, required_tool_purpose(tool)))
        .collect::<Vec<_>>();
    tools.push(check_optional_tool(
        "gh",
        "create and push GitHub repositories",
    ));
    tools.extend(
        component_tool_requirements(scope)
            .into_iter()
            .map(|tool| check_optional_tool(tool, component_tool_purpose(tool))),
    );
    let missing_required = tools
        .iter()
        .filter(|tool| tool.required && !tool.installed)
        .map(|tool| tool.name)
        .collect::<Vec<_>>();

    DoctorReport {
        scope_code: scope.scope_code(),
        scope: scope.scope_label(),
        blueprint: scope.blueprint.map(BlueprintName::as_str),
        blueprint_version: scope.blueprint_version.clone(),
        path: scope.path_display(),
        status_code: DoctorStatusCode::from_missing_required(&missing_required),
        ok: missing_required.is_empty(),
        next_steps: next_steps_for_missing_required(&missing_required, scope),
        missing_required,
        tools,
    }
}

fn print_human_report(report: &DoctorReport) {
    ui::section("Forge doctor");
    if let Some(blueprint) = report.blueprint {
        ui::info("scope", blueprint);
    }
    if let Some(blueprint_version) = &report.blueprint_version {
        ui::info("blueprint version", blueprint_version);
    }
    if let Some(path) = &report.path {
        ui::info("path", path);
    }

    for tool in &report.tools {
        if tool.installed {
            ui::success(format!("{} installed", tool.label));
            if let Some(version) = &tool.version {
                ui::info("version", version);
            }
        } else if tool.required {
            ui::info(tool.label, format!("missing; needed to {}", tool.purpose));
        } else {
            ui::info(
                tool.label,
                format!("missing (optional unless you need to {})", tool.purpose),
            );
        }
    }

    if !report.next_steps.is_empty() {
        ui::section("Next steps");
        for step in &report.next_steps {
            ui::next_step(step);
        }
    }
}

fn next_steps_for_missing_required(missing_required: &[&str], scope: &DoctorScope) -> Vec<String> {
    if missing_required.is_empty() {
        Vec::new()
    } else {
        vec![
            format!(
                "install missing required tools: {}",
                missing_required.join(", ")
            ),
            scope.rerun_command(),
        ]
    }
}

fn required_tool_requirements(blueprint: Option<BlueprintName>) -> Vec<&'static str> {
    let mut tools = vec!["git"];
    match blueprint {
        Some(blueprint) => tools.extend(blueprint.definition().required_tools.iter().copied()),
        None => tools.extend(
            BLUEPRINT_REGISTRY
                .iter()
                .flat_map(|blueprint| blueprint.required_tools.iter().copied()),
        ),
    }
    tools.sort_unstable();
    tools.dedup();
    tools
}

fn required_tool_purpose(tool: &str) -> &'static str {
    match tool {
        "cargo" => "build and install forge, and run Rust blueprint tasks",
        "git" => "initialize generated repositories",
        "just" => "run generated project tasks",
        "uv" => "sync generated project dependencies",
        _ => "run generated project tasks",
    }
}

fn check_required_tool(name: &'static str, purpose: &'static str) -> ToolStatus {
    let version = command_version(name);
    let installed = version.is_some();
    ToolStatus {
        name,
        label: name,
        required: true,
        installed,
        status_code: ToolStatusCode::from_state(true, installed),
        version,
        purpose,
    }
}

fn check_optional_tool(name: &'static str, purpose: &'static str) -> ToolStatus {
    let version = command_version(name);
    let installed = version.is_some();
    ToolStatus {
        name,
        label: if name == "gh" { "gh cli" } else { name },
        required: false,
        installed,
        status_code: ToolStatusCode::from_state(false, installed),
        version,
        purpose,
    }
}

fn component_tool_requirements(scope: &DoctorScope) -> Vec<&'static str> {
    scope
        .enabled_components
        .iter()
        .copied()
        .flat_map(|component| component.required_tools().iter().copied())
        .collect()
}

fn component_tool_purpose(tool: &str) -> &'static str {
    match tool {
        "npx" => "run Prettier component hooks for JSON, YAML, and Markdown",
        _ => "run optional managed component hooks",
    }
}

#[derive(Serialize)]
struct DoctorReport {
    scope_code: DoctorScopeCode,
    scope: &'static str,
    blueprint: Option<&'static str>,
    blueprint_version: Option<String>,
    path: Option<String>,
    status_code: DoctorStatusCode,
    ok: bool,
    next_steps: Vec<String>,
    missing_required: Vec<&'static str>,
    tools: Vec<ToolStatus>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum DoctorScopeCode {
    Global,
    Blueprint,
    Path,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum DoctorStatusCode {
    Ok,
    MissingRequired,
}

impl DoctorStatusCode {
    fn from_missing_required(missing_required: &[&'static str]) -> Self {
        if missing_required.is_empty() {
            Self::Ok
        } else {
            Self::MissingRequired
        }
    }
}

#[derive(Serialize)]
struct ToolStatus {
    name: &'static str,
    label: &'static str,
    required: bool,
    installed: bool,
    status_code: ToolStatusCode,
    version: Option<String>,
    purpose: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ToolStatusCode {
    Installed,
    MissingRequired,
    MissingOptional,
}

impl ToolStatusCode {
    fn from_state(required: bool, installed: bool) -> Self {
        match (required, installed) {
            (_, true) => Self::Installed,
            (true, false) => Self::MissingRequired,
            (false, false) => Self::MissingOptional,
        }
    }
}

fn command_version(command: &str) -> Option<String> {
    Command::new(command)
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| {
            String::from_utf8(output.stdout)
                .ok()
                .and_then(|stdout| stdout.lines().next().map(str::to_string))
        })
        .filter(|version| !version.trim().is_empty())
}

fn enabled_components_from_pyproject(
    blueprint: BlueprintName,
    pyproject: &str,
) -> Result<Vec<ManagedComponent>> {
    let parsed: toml::Value =
        toml::from_str(pyproject).context("failed to parse pyproject.toml")?;
    let options = parsed
        .get("tool")
        .and_then(toml::Value::as_table)
        .and_then(|tool| tool.get("forge"))
        .and_then(toml::Value::as_table)
        .and_then(|forge| forge.get("options"))
        .and_then(toml::Value::as_table);
    let mut values = std::collections::BTreeMap::new();
    if let Some(options) = options {
        for (name, value) in options {
            let enabled = value
                .as_bool()
                .with_context(|| format!("tool.forge.options.{name} must be a boolean"))?;
            values.insert(name.clone(), enabled);
        }
    }
    let options = validate_managed_options_from_metadata(blueprint, values)?;

    let mut enabled_components = Vec::new();
    for component in ManagedComponent::ALL {
        if blueprint.supports_option(component.option())
            && managed_option_enabled(&options, component.option())?
        {
            enabled_components.push(component);
        }
    }

    Ok(enabled_components)
}

#[cfg(test)]
mod tests {
    use crate::blueprint::BLUEPRINT_REGISTRY;
    use crate::blueprint::BlueprintName;
    use crate::blueprint::components::ManagedComponent;
    use crate::commands::doctor::{DoctorScope, required_tool_requirements};
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn required_tools_include_core_and_blueprint_requirements() {
        let tools = required_tool_requirements(None);

        assert!(tools.contains(&"cargo"));
        assert!(tools.contains(&"git"));
        for blueprint in &BLUEPRINT_REGISTRY {
            for required_tool in blueprint.required_tools {
                assert!(tools.contains(required_tool));
            }
        }
    }

    #[test]
    fn required_tools_can_be_scoped_to_one_blueprint() {
        let tools = required_tool_requirements(Some(BlueprintName::PythonLibrary));

        assert_eq!(tools, vec!["git", "just", "uv"]);
        assert!(!tools.contains(&"cargo"));
    }

    #[test]
    fn doctor_scope_rejects_blueprint_and_path() {
        let error =
            DoctorScope::resolve(Some(BlueprintName::PythonLibrary), Some(PathBuf::from(".")))
                .expect_err("scope should reject ambiguous input");

        assert!(error.to_string().contains("--blueprint cannot be used"));
    }

    #[test]
    fn doctor_scope_detects_blueprint_from_project_path() {
        let temp = TempDir::new().expect("temp dir should create");
        std::fs::write(
            temp.path().join("pyproject.toml"),
            python_forge_metadata(false),
        )
        .expect("pyproject should write");

        let scope = DoctorScope::resolve(None, Some(temp.path().to_path_buf()))
            .expect("scope should resolve");

        assert_eq!(scope.blueprint, Some(BlueprintName::PythonLibrary));
        assert_eq!(scope.blueprint_version.as_deref(), Some("0.1.0"));
        assert_eq!(
            scope.path.expect("path should be present"),
            temp.path()
                .canonicalize()
                .expect("temp path should resolve")
        );
    }

    #[test]
    fn doctor_scope_detects_enabled_components_from_project_path() {
        let temp = TempDir::new().expect("temp dir should create");
        std::fs::write(
            temp.path().join("pyproject.toml"),
            python_forge_metadata(true),
        )
        .expect("pyproject should write");

        let scope = DoctorScope::resolve(None, Some(temp.path().to_path_buf()))
            .expect("scope should resolve");

        assert_eq!(scope.blueprint_version.as_deref(), Some("0.1.0"));
        assert_eq!(scope.enabled_components, vec![ManagedComponent::Prettier]);
    }

    #[test]
    fn doctor_scope_rejects_missing_options_table() {
        let temp = TempDir::new().expect("temp dir should create");
        std::fs::write(
            temp.path().join("pyproject.toml"),
            python_forge_metadata_without_options(),
        )
        .expect("pyproject should write");

        let error = DoctorScope::resolve(None, Some(temp.path().to_path_buf()))
            .expect_err("scope should fail");
        assert!(
            error
                .to_string()
                .contains("failed to validate Forge metadata")
        );
    }

    fn python_forge_metadata(prettier: bool) -> String {
        format!(
            r#"[tool.forge]
blueprint = "python-library"
blueprint_version = "0.1.0"
project_name = "ops-tools"
package_name = "ops_tools"
description = "Ops toolchain"
author_name = "Grace Hopper"
author_email = "grace@example.com"
license = "MIT"
python_min = "3.12"

[tool.forge.options]
docs = true
codecov = true
pypi-publish = false
python-rules = true
prettier = {prettier}
editorconfig = false
markdownlint = false
"#
        )
    }

    fn python_forge_metadata_without_options() -> &'static str {
        r#"[tool.forge]
blueprint = "python-library"
blueprint_version = "0.1.0"
project_name = "ops-tools"
package_name = "ops_tools"
description = "Ops toolchain"
author_name = "Grace Hopper"
author_email = "grace@example.com"
license = "MIT"
python_min = "3.12"
"#
    }

    #[test]
    fn doctor_scope_rerun_command_preserves_scope() {
        assert_eq!(
            DoctorScope {
                blueprint: Some(BlueprintName::PythonLibrary),
                blueprint_version: Some("0.1.0".to_string()),
                path: None,
                enabled_components: ManagedComponent::ALL.to_vec(),
            }
            .rerun_command(),
            "forge doctor --blueprint python-library"
        );

        assert_eq!(
            DoctorScope {
                blueprint: Some(BlueprintName::PythonLibrary),
                blueprint_version: Some("0.1.0".to_string()),
                path: Some(PathBuf::from("/tmp/project")),
                enabled_components: Vec::new(),
            }
            .rerun_command(),
            "forge doctor --path /tmp/project"
        );
    }
}
