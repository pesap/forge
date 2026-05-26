use std::fs;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use dialoguer::{Confirm, Input, MultiSelect, Select};
use serde::Serialize;
use toml::Value;

use crate::blueprint::any_project;
use crate::blueprint::components::{ComponentSelection, ManagedComponent};
use crate::blueprint::files::{
    GeneratedFiles, managed_file_path, plan_generated_files, write_generated_file,
};
use crate::blueprint::python_library;
use crate::blueprint::rust_library;
use crate::blueprint::{BlueprintName, ManagedOption, ManagedOptionValues, managed_option_enabled};
use crate::cli::{GithubVisibility, NewArgs};
use crate::commands::diff;
use crate::errors::{ErrorCode, coded_error};
use crate::ui;

const DEFAULT_LICENSE: &str = "BSD-3-Clause";
const DEFAULT_PYTHON_MIN: &str = "3.11";

pub fn run(args: NewArgs) -> Result<()> {
    let stdin_is_terminal = std::io::stdin().is_terminal();
    ensure_interactive_setup_allowed(args.yes, args.json, args.dry_run, stdin_is_terminal)?;
    validate_diff_mode(args.diff, args.dry_run)?;
    let blueprint = select_blueprint(args.blueprint, args.yes)?;
    validate_explicit_options(blueprint, &args)?;
    validate_required_fields_for_yes(blueprint, &args)?;
    let project = render_blueprint(&args, blueprint, RenderScope::Project)?;
    let infrastructure = managed_infrastructure_summary(&project.files);

    let destination = destination_path(&project.project_name, &args.path)?;
    let replay_command =
        preview_new_command(&args, blueprint, &destination, &project, stdin_is_terminal);
    validate_destination_is_available(&destination)?;
    let review_context = new_setup_review_context(&args, blueprint, &project, &replay_command);
    if !confirm_interactive_setup(
        args.yes,
        args.json,
        args.dry_run,
        SetupReview {
            section_title: "Project setup review",
            path: &destination,
            blueprint,
            options: &project.options,
            prompt: "Create this project?",
            context: review_context,
        },
    )? {
        ui::section("Project creation canceled");
        ui::success("no files changed");
        return Ok(());
    }

    if args.dry_run {
        if args.json {
            print_json_report(NewReportInput {
                destination: &destination,
                project_name: &project.project_name,
                blueprint,
                github: args.github,
                github_visibility: github_visibility_for_report(
                    args.github,
                    args.github_visibility,
                ),
                dry_run: args.dry_run,
                infrastructure: &infrastructure,
                options: &project.options,
                files: project
                    .files
                    .keys()
                    .map(|path| path.display().to_string())
                    .collect(),
                dry_run_command: &replay_command,
            })?;
            return Ok(());
        }

        ui::section("Project creation preview");
        ui::info("path", destination.display());
        ui::info("blueprint", blueprint.as_str());
        ui::info("blueprint version", blueprint.version());
        ui::info("options", format_selected_options(&project.options));
        ui::info(
            "required tools",
            required_tools_summary_for_options(blueprint, &project.options),
        );
        ui::info("infrastructure", &infrastructure);
        ui::info(
            "github",
            github_preview(args.github, args.github_visibility),
        );
        if pypi_publish_notice(&project.options) {
            ui::info("pypi", python_library::PYPI_PUBLISH_NOTICE);
        }
        ui::info("files", project.files.len());
        for relative_path in project.files.keys() {
            ui::action("create", relative_path.display());
        }
        if args.diff {
            let actions = plan_generated_files(&destination, &project.files);
            diff::print_diffs(&destination, &actions, &project.files)?;
        }
        ui::section("Next steps");
        ui::next_step(&replay_command);
        return Ok(());
    }

    ensure_destination_directory_exists(&destination)?;

    let mut file_paths: Vec<_> = project
        .files
        .keys()
        .map(|path| path.display().to_string())
        .collect();
    write_project_files(&destination, project.files)?;
    if lock_dependencies_before_push(&destination, args.github, args.json)? {
        file_paths.push("uv.lock".to_string());
        file_paths.sort();
    }
    initialize_git_repository(&destination, args.github, args.json)?;

    if args.github {
        create_github_repo(
            &destination,
            &project.project_name,
            args.github_owner.clone(),
            args.github_visibility.unwrap_or(GithubVisibility::Public),
            args.yes,
            args.json,
        )?;
    }

    if args.json {
        print_json_report(NewReportInput {
            destination: &destination,
            project_name: &project.project_name,
            blueprint,
            github: args.github,
            github_visibility: github_visibility_for_report(args.github, args.github_visibility),
            dry_run: args.dry_run,
            infrastructure: &infrastructure,
            options: &project.options,
            files: file_paths,
            dry_run_command: &replay_command,
        })?;
        return Ok(());
    }

    ui::section("Project created");
    ui::success(format!("generated {}", project.project_name));
    ui::info("path", destination.display());
    ui::info("blueprint", blueprint.as_str());
    ui::info("blueprint version", blueprint.version());
    ui::info("options", format_selected_options(&project.options));
    ui::info(
        "required tools",
        required_tools_summary_for_options(blueprint, &project.options),
    );
    ui::info("infrastructure", infrastructure);
    if pypi_publish_notice(&project.options) {
        ui::info("pypi", python_library::PYPI_PUBLISH_NOTICE);
    }
    ui::info("managed update", "forge update --path .");
    ui::section("Next steps");
    ui::next_step(&format!(
        "cd {}",
        ui::shell_arg(destination.display().to_string())
    ));
    ui::next_step("uv sync --all-groups");
    ui::next_step("just verify");
    Ok(())
}

pub(crate) fn confirm_interactive_setup(
    assume_yes: bool,
    json: bool,
    dry_run: bool,
    review: SetupReview<'_>,
) -> Result<bool> {
    if !should_confirm_interactive_setup(assume_yes, json, dry_run) {
        return Ok(true);
    }

    ui::section(review.section_title);
    ui::info("path", review.path.display());
    ui::info("blueprint", review.blueprint.as_str());
    ui::info("blueprint version", review.blueprint.version());
    ui::info("options", format_selected_options(review.options));
    for item in &review.context {
        ui::info(item.label, &item.value);
    }
    Ok(Confirm::new()
        .with_prompt(review.prompt)
        .default(true)
        .interact()?)
}

pub(crate) struct SetupReview<'a> {
    pub(crate) section_title: &'a str,
    pub(crate) path: &'a Path,
    pub(crate) blueprint: BlueprintName,
    pub(crate) options: &'a [SelectedOption],
    pub(crate) prompt: &'a str,
    pub(crate) context: Vec<SetupReviewItem>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SetupReviewItem {
    pub(crate) label: &'static str,
    pub(crate) value: String,
}

impl SetupReviewItem {
    pub(crate) fn new(label: &'static str, value: impl Into<String>) -> Self {
        Self {
            label,
            value: value.into(),
        }
    }
}

pub(crate) fn should_confirm_interactive_setup(
    assume_yes: bool,
    json: bool,
    dry_run: bool,
) -> bool {
    !assume_yes && !json && !dry_run
}

fn validate_diff_mode(diff: bool, dry_run: bool) -> Result<()> {
    if diff && !dry_run {
        return Err(coded_error(ErrorCode::Input, "--diff requires --dry-run"));
    }

    Ok(())
}

pub(crate) fn render_blueprint(
    args: &NewArgs,
    blueprint: BlueprintName,
    scope: RenderScope,
) -> Result<ProjectRender> {
    match blueprint {
        BlueprintName::AnyProject => {
            let config = gather_any_project_config(args)?;
            config.validate()?;
            let options = any_project_managed_option_values(&config);
            let files = match scope {
                RenderScope::Project => any_project::render_project_files(&config),
                RenderScope::ManagedInfrastructure => any_project::render_managed_files(&config),
            };
            Ok(ProjectRender {
                project_name: config.project_name.clone(),
                options: selected_options_from_values(blueprint, &options)?,
                files,
            })
        }
        BlueprintName::PythonLibrary => {
            let config = gather_python_library_config(args)?;
            config.validate()?;
            let options = python_library_managed_option_values(&config);
            let files = match scope {
                RenderScope::Project => python_library::render_project_files(&config),
                RenderScope::ManagedInfrastructure => python_library::render_managed_files(&config),
            };
            Ok(ProjectRender {
                project_name: config.project_name.clone(),
                options: selected_options_from_values(blueprint, &options)?,
                files,
            })
        }
        BlueprintName::RustLibrary => {
            let config = gather_rust_library_config(args)?;
            config.validate()?;
            let options = rust_library_managed_option_values(&config);
            let files = match scope {
                RenderScope::Project => rust_library::render_project_files(&config),
                RenderScope::ManagedInfrastructure => rust_library::render_managed_files(&config),
            };
            Ok(ProjectRender {
                project_name: config.project_name.clone(),
                options: selected_options_from_values(blueprint, &options)?,
                files,
            })
        }
    }
}

fn selected_options_from_values(
    blueprint: BlueprintName,
    options: &ManagedOptionValues,
) -> Result<Vec<SelectedOption>> {
    blueprint
        .supported_options()
        .iter()
        .map(|option| {
            let enabled = managed_option_enabled(options, *option)?;
            Ok(SelectedOption::new(*option, enabled))
        })
        .collect()
}

fn any_project_managed_option_values(config: &any_project::ProjectConfig) -> ManagedOptionValues {
    let mut values = ManagedOptionValues::new();
    values.insert(ManagedOption::Docs, config.docs);
    values.insert(
        ManagedOption::Prettier,
        config.components.is_enabled(ManagedComponent::Prettier),
    );
    values.insert(
        ManagedOption::Editorconfig,
        config.components.is_enabled(ManagedComponent::Editorconfig),
    );
    values.insert(
        ManagedOption::Markdownlint,
        config.components.is_enabled(ManagedComponent::Markdownlint),
    );
    values
}

fn python_library_managed_option_values(
    config: &python_library::ProjectConfig,
) -> ManagedOptionValues {
    let mut values = ManagedOptionValues::new();
    values.insert(ManagedOption::Docs, config.docs);
    values.insert(ManagedOption::Codecov, config.codecov);
    values.insert(ManagedOption::PypiPublish, config.pypi_publish);
    values.insert(ManagedOption::PythonRules, config.python_rules);
    values.insert(
        ManagedOption::Prettier,
        config.components.is_enabled(ManagedComponent::Prettier),
    );
    values.insert(
        ManagedOption::Editorconfig,
        config.components.is_enabled(ManagedComponent::Editorconfig),
    );
    values.insert(
        ManagedOption::Markdownlint,
        config.components.is_enabled(ManagedComponent::Markdownlint),
    );
    values
}

fn rust_library_managed_option_values(config: &rust_library::ProjectConfig) -> ManagedOptionValues {
    let mut values = ManagedOptionValues::new();
    values.insert(ManagedOption::Docs, config.docs);
    values.insert(ManagedOption::RustRules, config.rust_rules);
    values.insert(
        ManagedOption::Prettier,
        config.components.is_enabled(ManagedComponent::Prettier),
    );
    values.insert(
        ManagedOption::Editorconfig,
        config.components.is_enabled(ManagedComponent::Editorconfig),
    );
    values.insert(
        ManagedOption::Markdownlint,
        config.components.is_enabled(ManagedComponent::Markdownlint),
    );
    values
}

fn print_json_report(input: NewReportInput<'_>) -> Result<()> {
    let report = NewReport {
        project_name: input.project_name,
        path: input.destination.display().to_string(),
        blueprint: input.blueprint.as_str(),
        blueprint_version: input.blueprint.version(),
        status_code: new_status_code(input.dry_run),
        github: input.github,
        github_visibility: input.github_visibility.map(github_visibility_label),
        dry_run: input.dry_run,
        managed_update: "forge update --path .",
        infrastructure: input.infrastructure,
        required_tools: required_tools_summary_for_options(input.blueprint, input.options),
        options: input.options,
        files: input.files,
        next_steps: next_steps_for_report(input.destination, input.dry_run_command, input.dry_run),
    };

    ui::json(report)
}

struct NewReportInput<'a> {
    destination: &'a Path,
    project_name: &'a str,
    blueprint: BlueprintName,
    github: bool,
    github_visibility: Option<GithubVisibility>,
    dry_run: bool,
    infrastructure: &'a str,
    options: &'a [SelectedOption],
    files: Vec<String>,
    dry_run_command: &'a str,
}

fn next_steps_for_report(destination: &Path, dry_run_command: &str, dry_run: bool) -> Vec<String> {
    if dry_run {
        vec![dry_run_command.to_string()]
    } else {
        vec![
            format!("cd {}", ui::shell_arg(destination.display().to_string())),
            "uv sync --all-groups".to_string(),
            "just verify".to_string(),
        ]
    }
}

fn new_setup_review_context(
    args: &NewArgs,
    blueprint: BlueprintName,
    project: &ProjectRender,
    replay_command: &str,
) -> Vec<SetupReviewItem> {
    let mut context = vec![
        SetupReviewItem::new("files", project.files.len().to_string()),
        SetupReviewItem::new(
            "infrastructure",
            managed_infrastructure_summary(&project.files),
        ),
        SetupReviewItem::new(
            "required tools",
            required_tools_summary_for_options(blueprint, &project.options),
        ),
        SetupReviewItem::new(
            "github",
            github_preview(args.github, args.github_visibility),
        ),
        SetupReviewItem::new("apply", replay_command),
    ];

    if pypi_publish_notice(&project.options) {
        context.push(SetupReviewItem::new(
            "pypi",
            python_library::PYPI_PUBLISH_NOTICE,
        ));
    }

    context
}

fn pypi_publish_notice(options: &[SelectedOption]) -> bool {
    options
        .iter()
        .any(|option| option.name == ManagedOption::PypiPublish.as_str() && option.enabled)
}

fn preview_new_command(
    args: &NewArgs,
    blueprint: BlueprintName,
    destination: &Path,
    project: &ProjectRender,
    stdin_is_terminal: bool,
) -> String {
    if args.yes || !stdin_is_terminal {
        return new_command(args, blueprint, destination);
    }

    resolved_new_args_from_rendered_pyproject(args, project)
        .map(|resolved| new_command(&resolved, blueprint, destination))
        .unwrap_or_else(|| new_command(args, blueprint, destination))
}

pub(crate) fn resolved_new_args_from_rendered_pyproject(
    args: &NewArgs,
    project: &ProjectRender,
) -> Option<NewArgs> {
    let pyproject = project
        .files
        .get(Path::new("pyproject.toml"))?
        .as_text()
        .map(str::to_string)?;
    let parsed: Value = toml::from_str(&pyproject).ok()?;
    let forge = parsed
        .get("tool")
        .and_then(Value::as_table)?
        .get("forge")
        .and_then(Value::as_table)?;
    let empty_options = toml::Table::new();
    let options = forge
        .get("overrides")
        .or_else(|| forge.get("options"))
        .and_then(Value::as_table)
        .unwrap_or(&empty_options);

    let mut resolved = args.clone();
    resolved.project_name = forge
        .get("project_name")
        .and_then(Value::as_str)
        .map(str::to_string);
    resolved.package_name = forge
        .get("package_name")
        .and_then(Value::as_str)
        .map(str::to_string);
    resolved.description = forge
        .get("description")
        .and_then(Value::as_str)
        .map(str::to_string);
    resolved.author_name = forge
        .get("author_name")
        .and_then(Value::as_str)
        .map(str::to_string);
    resolved.author_email = forge
        .get("author_email")
        .and_then(Value::as_str)
        .map(str::to_string);
    resolved.license = forge
        .get("license")
        .and_then(Value::as_str)
        .map(str::to_string);
    resolved.python_min = forge
        .get("python_min")
        .and_then(Value::as_str)
        .map(str::to_string);

    resolved.docs = option_flag(options, "docs").unwrap_or(resolved.docs);
    resolved.codecov = option_flag(options, "codecov");
    resolved.pypi_publish = option_flag(options, "pypi-publish");
    resolved.prettier = option_flag(options, "prettier").unwrap_or(resolved.prettier);
    resolved.editorconfig = option_flag(options, "editorconfig").unwrap_or(resolved.editorconfig);
    resolved.markdownlint = option_flag(options, "markdownlint").unwrap_or(resolved.markdownlint);

    Some(resolved)
}

fn option_flag(options: &toml::Table, key: &str) -> Option<bool> {
    options.get(key).and_then(Value::as_bool)
}

fn new_command(args: &NewArgs, blueprint: BlueprintName, destination: &Path) -> String {
    let mut parts = vec![
        "forge".to_string(),
        "new".to_string(),
        "--path".to_string(),
        ui::shell_arg(destination.display().to_string()),
        "--blueprint".to_string(),
        blueprint.as_str().to_string(),
    ];

    push_option(&mut parts, "--project-name", args.project_name.as_deref());
    push_option(&mut parts, "--package-name", args.package_name.as_deref());
    push_option(&mut parts, "--description", args.description.as_deref());
    push_option(&mut parts, "--author-name", args.author_name.as_deref());
    push_option(&mut parts, "--author-email", args.author_email.as_deref());
    push_option(&mut parts, "--license", args.license.as_deref());
    push_option(&mut parts, "--python-min", args.python_min.as_deref());
    push_managed_option_flags(
        &mut parts,
        ManagedOptionFlags {
            docs: args.docs,
            codecov: args.codecov,
            pypi_publish: args.pypi_publish,
            prettier: args.prettier,
            editorconfig: args.editorconfig,
            markdownlint: args.markdownlint,
        },
    );
    if args.github {
        parts.push("--github".to_string());
    }
    push_option(&mut parts, "--github-owner", args.github_owner.as_deref());
    if let Some(visibility) = args.github_visibility {
        parts.push("--github-visibility".to_string());
        parts.push(github_visibility_label(visibility).to_string());
    }
    if args.yes {
        parts.push("--yes".to_string());
    }

    parts.join(" ")
}

pub(crate) fn managed_infrastructure_summary(files: &GeneratedFiles) -> String {
    let mut parts = Vec::new();

    if files.contains_key(Path::new("pyproject.toml")) {
        parts.push("pyproject.toml".to_string());
    }
    if files.contains_key(Path::new("justfile")) {
        parts.push("justfile".to_string());
    }
    if files.contains_key(Path::new(".pre-commit-config.yaml")) {
        parts.push("prek hooks".to_string());
    }
    if files.contains_key(Path::new("AGENTS.md")) {
        parts.push("AGENTS.md".to_string());
    }
    if files.contains_key(Path::new("CLAUDE.md")) {
        parts.push("CLAUDE.md link".to_string());
    }
    if files.contains_key(Path::new("docs/package.json")) {
        parts.push("docs".to_string());
    }

    let workflow_count = files
        .keys()
        .filter(|path| path.starts_with(Path::new(".github/workflows")))
        .count();
    if workflow_count > 0 {
        parts.push(format!("github actions ({workflow_count})"));
    }

    if parts.is_empty() {
        "managed files".to_string()
    } else {
        parts.join(", ")
    }
}

pub(crate) fn required_tools_summary_for_options(
    blueprint: BlueprintName,
    options: &[SelectedOption],
) -> String {
    let mut tools = blueprint.definition().required_tools.to_vec();

    for component in ManagedComponent::ALL {
        if !blueprint.supports_option(component.option()) {
            continue;
        }
        if !selected_option_enabled(options, component.option_name()) {
            continue;
        }

        for required_tool in component.required_tools() {
            if !tools.contains(required_tool) {
                tools.push(required_tool);
            }
        }
    }

    tools.join(", ")
}

fn selected_option_enabled(options: &[SelectedOption], option_name: &str) -> bool {
    options
        .iter()
        .find(|option| option.name == option_name)
        .is_some_and(|option| option.enabled)
}

fn push_option(parts: &mut Vec<String>, name: &str, value: Option<&str>) {
    let Some(value) = value else {
        return;
    };
    parts.push(name.to_string());
    parts.push(ui::shell_arg(value));
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) struct ManagedOptionFlags {
    pub(crate) docs: bool,
    pub(crate) codecov: Option<bool>,
    pub(crate) pypi_publish: Option<bool>,
    pub(crate) prettier: bool,
    pub(crate) editorconfig: bool,
    pub(crate) markdownlint: bool,
}

pub(crate) fn push_managed_option_flags(parts: &mut Vec<String>, flags: ManagedOptionFlags) {
    if !flags.docs {
        parts.push("--docs=false".to_string());
    }
    if let Some(codecov) = flags.codecov {
        parts.push(format!("--codecov={codecov}"));
    }
    if let Some(pypi_publish) = flags.pypi_publish {
        parts.push(format!("--pypi-publish={pypi_publish}"));
    }
    if flags.prettier {
        parts.push("--prettier".to_string());
    }
    if flags.editorconfig {
        parts.push("--editorconfig".to_string());
    }
    if flags.markdownlint {
        parts.push("--markdownlint".to_string());
    }
}

#[derive(Serialize)]
struct NewReport<'a> {
    project_name: &'a str,
    path: String,
    blueprint: &'a str,
    blueprint_version: &'a str,
    status_code: &'static str,
    github: bool,
    github_visibility: Option<&'static str>,
    dry_run: bool,
    managed_update: &'a str,
    infrastructure: &'a str,
    required_tools: String,
    options: &'a [SelectedOption],
    files: Vec<String>,
    next_steps: Vec<String>,
}

fn new_status_code(dry_run: bool) -> &'static str {
    if dry_run { "dry_run" } else { "created" }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct SelectedOption {
    pub(crate) name: &'static str,
    pub(crate) enabled: bool,
}

impl SelectedOption {
    fn new(option: ManagedOption, enabled: bool) -> Self {
        Self {
            name: option.as_str(),
            enabled,
        }
    }
}

pub(crate) fn format_selected_options(options: &[SelectedOption]) -> String {
    let enabled = options
        .iter()
        .filter(|option| option.enabled)
        .map(|option| option.name)
        .collect::<Vec<_>>();
    let disabled = options
        .iter()
        .filter(|option| !option.enabled)
        .map(|option| option.name)
        .collect::<Vec<_>>();

    let enabled_summary = if enabled.is_empty() {
        "none".to_string()
    } else {
        enabled.join(", ")
    };
    let disabled_summary = if disabled.is_empty() {
        "none".to_string()
    } else {
        disabled.join(", ")
    };

    format!("enabled: {enabled_summary}; disabled: {disabled_summary}")
}

fn github_visibility_for_report(
    github: bool,
    visibility: Option<GithubVisibility>,
) -> Option<GithubVisibility> {
    github.then_some(visibility.unwrap_or(GithubVisibility::Public))
}

fn github_preview(github: bool, visibility: Option<GithubVisibility>) -> String {
    match github_visibility_for_report(github, visibility) {
        Some(visibility) => format!("create {} repository", github_visibility_label(visibility)),
        None => "disabled".to_string(),
    }
}

fn github_visibility_label(visibility: GithubVisibility) -> &'static str {
    match visibility {
        GithubVisibility::Public => "public",
        GithubVisibility::Private => "private",
    }
}

pub(crate) fn select_blueprint(
    blueprint: Option<BlueprintName>,
    assume_yes: bool,
) -> Result<BlueprintName> {
    match blueprint {
        Some(blueprint) => Ok(blueprint),
        None if assume_yes => Err(coded_error(
            ErrorCode::Input,
            "--blueprint is required when --yes is used",
        )),
        None if !std::io::stdin().is_terminal() => Err(coded_error(
            ErrorCode::Input,
            "--blueprint is required when interactive setup is unavailable",
        )),
        None => prompt_blueprint(),
    }
}

pub(crate) fn ensure_interactive_setup_allowed(
    assume_yes: bool,
    json: bool,
    dry_run: bool,
    stdin_is_terminal: bool,
) -> Result<()> {
    if assume_yes || json || dry_run || stdin_is_terminal {
        return Ok(());
    }

    Err(coded_error(
        ErrorCode::Input,
        "interactive confirmation requires a terminal; pass --yes, --json, or --dry-run",
    ))
}

fn prompt_blueprint() -> Result<BlueprintName> {
    let labels = BlueprintName::ALL
        .into_iter()
        .map(BlueprintName::description)
        .collect::<Vec<_>>();
    let default_index = BlueprintName::ALL
        .iter()
        .position(|blueprint| *blueprint == BlueprintName::PythonLibrary)
        .unwrap_or(0);

    let selected = Select::new()
        .with_prompt("Blueprint")
        .items(&labels)
        .default(default_index)
        .interact()?;

    Ok(BlueprintName::ALL[selected])
}

pub(crate) struct ProjectRender {
    pub(crate) project_name: String,
    pub(crate) options: Vec<SelectedOption>,
    pub(crate) files: GeneratedFiles,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum RenderScope {
    Project,
    ManagedInfrastructure,
}

fn gather_any_project_config(args: &NewArgs) -> Result<any_project::ProjectConfig> {
    let mut project_name = args.project_name.clone();
    let mut description = args.description.clone();
    let mut docs = docs_enabled(args);
    let mut components = component_selection_from_args(args);

    if !args.yes && std::io::stdin().is_terminal() {
        prompt_if_missing_validated(
            &mut project_name,
            "Project name",
            any_project::is_valid_project_name,
            "invalid project name",
        )?;
        prompt_if_missing_validated(
            &mut description,
            "Description",
            is_non_empty_text,
            "description cannot be empty",
        )?;
        docs = prompt_bool("Generate Starlight documentation?", docs)?;
        prompt_supported_components(&mut components, BlueprintName::AnyProject)?;
    }

    Ok(any_project::ProjectConfig {
        project_name: require_field("project-name", project_name)?,
        description: require_field("description", description)?,
        docs,
        components,
    })
}

fn gather_python_library_config(args: &NewArgs) -> Result<python_library::ProjectConfig> {
    let mut project_name = args.project_name.clone();
    let mut package_name = args.package_name.clone();
    let mut description = args.description.clone();
    let author_name = args.author_name.clone();
    let author_email = args.author_email.clone();
    let mut license = args.license.clone();
    let mut python_min = args.python_min.clone();
    let mut docs = docs_enabled(args);
    let mut codecov = codecov_enabled(args);
    let mut pypi_publish = pypi_publish_enabled(args);
    let mut components = component_selection_from_args(args);

    if !args.yes && std::io::stdin().is_terminal() {
        prompt_if_missing_validated(
            &mut project_name,
            "Project name",
            python_library::is_valid_project_name,
            "invalid project name",
        )?;
        if package_name.is_none() {
            let project_name = project_name
                .as_deref()
                .context("project name should be set before package prompt")?;
            let default = default_python_package_name(project_name)?;
            prompt_if_missing_with_default_validated(
                &mut package_name,
                "Package name",
                &default,
                python_library::is_valid_package_name,
                "invalid package name",
            )?;
        }
        prompt_if_missing_validated(
            &mut description,
            "Description",
            is_non_empty_text,
            "description cannot be empty",
        )?;
        prompt_if_missing_with_default_validated(
            &mut license,
            "License",
            DEFAULT_LICENSE,
            is_supported_license,
            "license must be BSD-3-Clause, MIT, or Apache-2.0",
        )?;
        prompt_if_missing_with_default_validated(
            &mut python_min,
            "Minimum Python version",
            DEFAULT_PYTHON_MIN,
            python_library::is_valid_python_version,
            "python-min must be between 3.8 and 3.14 as major.minor",
        )?;
        docs = prompt_bool("Generate Starlight documentation?", docs)?;
        codecov = prompt_bool("Enable Codecov upload in CI?", codecov)?;
        pypi_publish = prompt_bool("Add trusted PyPI publish workflow?", pypi_publish)?;
        prompt_supported_components(&mut components, BlueprintName::PythonLibrary)?;
    }

    let project_name = require_field("project-name", project_name)?;
    let package_name = match package_name {
        Some(package_name) => package_name,
        None => default_python_package_name(&project_name)?,
    };

    Ok(python_library::ProjectConfig {
        project_name,
        package_name,
        description: require_field("description", description)?,
        author_name,
        author_email,
        license: license.unwrap_or_else(|| DEFAULT_LICENSE.to_string()),
        python_min: python_min.unwrap_or_else(|| DEFAULT_PYTHON_MIN.to_string()),
        docs,
        codecov,
        pypi_publish,
        python_rules: true,
        components,
    })
}

fn gather_rust_library_config(args: &NewArgs) -> Result<rust_library::ProjectConfig> {
    let mut project_name = args.project_name.clone();
    let mut crate_name = args.package_name.clone();
    let mut description = args.description.clone();
    let author_name = args.author_name.clone();
    let author_email = args.author_email.clone();
    let mut license = args.license.clone();
    let mut docs = docs_enabled(args);
    let mut components = component_selection_from_args(args);

    if !args.yes && std::io::stdin().is_terminal() {
        prompt_if_missing_validated(
            &mut project_name,
            "Project name",
            rust_library::is_valid_package_name,
            "invalid Rust package name",
        )?;
        if crate_name.is_none() {
            let project_name = project_name
                .as_deref()
                .context("project name should be set before crate prompt")?;
            let default = rust_library::default_crate_name(project_name);
            prompt_if_missing_with_default_validated(
                &mut crate_name,
                "Crate name",
                &default,
                rust_library::is_valid_crate_name,
                "invalid Rust crate name",
            )?;
        }
        prompt_if_missing_validated(
            &mut description,
            "Description",
            is_non_empty_text,
            "description cannot be empty",
        )?;
        prompt_if_missing_with_default_validated(
            &mut license,
            "License",
            DEFAULT_LICENSE,
            is_supported_license,
            "license must be BSD-3-Clause, MIT, or Apache-2.0",
        )?;
        docs = prompt_bool("Generate Starlight documentation?", docs)?;
        prompt_supported_components(&mut components, BlueprintName::RustLibrary)?;
    }

    let project_name = require_field("project-name", project_name)?;
    let crate_name = crate_name.unwrap_or_else(|| rust_library::default_crate_name(&project_name));

    Ok(rust_library::ProjectConfig {
        project_name,
        crate_name,
        description: require_field("description", description)?,
        author_name,
        author_email,
        license: license.unwrap_or_else(|| DEFAULT_LICENSE.to_string()),
        rust_edition: "2024".to_string(),
        docs,
        rust_rules: true,
        components,
    })
}

fn prompt_if_missing_validated(
    value: &mut Option<String>,
    prompt: &str,
    validate: fn(&str) -> bool,
    error_message: &'static str,
) -> Result<()> {
    if value.is_none() {
        *value = Some(
            Input::new()
                .with_prompt(prompt)
                .validate_with(move |input: &String| {
                    if validate(input) {
                        Ok(())
                    } else {
                        Err(error_message)
                    }
                })
                .interact_text()?,
        );
    }
    Ok(())
}

fn prompt_if_missing_with_default_validated(
    value: &mut Option<String>,
    prompt: &str,
    default: &str,
    validate: fn(&str) -> bool,
    error_message: &'static str,
) -> Result<()> {
    if value.is_none() {
        *value = Some(
            Input::new()
                .with_prompt(prompt)
                .default(default.to_string())
                .validate_with(move |input: &String| {
                    if validate(input) {
                        Ok(())
                    } else {
                        Err(error_message)
                    }
                })
                .interact_text()?,
        );
    }
    Ok(())
}

fn require_field(name: &str, value: Option<String>) -> Result<String> {
    value.ok_or_else(|| {
        coded_error(
            ErrorCode::Input,
            format!("--{} is required (or run without --yes)", name),
        )
    })
}

fn is_non_empty_text(value: &str) -> bool {
    !value.trim().is_empty()
}

fn is_supported_license(value: &str) -> bool {
    matches!(value, "BSD-3-Clause" | "MIT" | "Apache-2.0")
}

fn default_python_package_name(project_name: &str) -> Result<String> {
    let package_name = python_library::default_package_name(project_name);
    if !python_library::is_valid_package_name(&package_name) {
        return Err(coded_error(
            ErrorCode::Input,
            format!(
                "derived package name '{package_name}' is invalid; pass --package-name explicitly"
            ),
        ));
    }

    Ok(package_name)
}

fn prompt_bool(prompt: &str, default: bool) -> Result<bool> {
    Ok(Confirm::new()
        .with_prompt(prompt)
        .default(default)
        .interact()?)
}

fn component_selection_from_args(args: &NewArgs) -> ComponentSelection {
    ComponentSelection::from_flags(args.prettier, args.editorconfig, args.markdownlint)
}

fn prompt_supported_components(
    components: &mut ComponentSelection,
    blueprint: BlueprintName,
) -> Result<()> {
    let prompt_options = supported_component_prompt_options(components, blueprint);
    if prompt_options.is_empty() {
        return Ok(());
    }

    let labels = prompt_options
        .iter()
        .map(|option| option.label.clone())
        .collect::<Vec<_>>();
    let defaults = prompt_options
        .iter()
        .map(|option| option.default_enabled)
        .collect::<Vec<_>>();

    let selected = MultiSelect::new()
        .with_prompt("Optional managed components (space to toggle)")
        .items(&labels)
        .defaults(&defaults)
        .interact()?;
    apply_component_prompt_selection(components, &prompt_options, &selected);

    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ComponentPromptOption {
    component: ManagedComponent,
    label: String,
    default_enabled: bool,
}

fn supported_component_prompt_options(
    components: &ComponentSelection,
    blueprint: BlueprintName,
) -> Vec<ComponentPromptOption> {
    ManagedComponent::ALL
        .into_iter()
        .filter(|component| blueprint.supports_option(component.option()))
        .map(|component| ComponentPromptOption {
            component,
            label: format!("{} ({})", component.option_name(), component.description()),
            default_enabled: components.is_enabled(component),
        })
        .collect()
}

fn apply_component_prompt_selection(
    components: &mut ComponentSelection,
    prompt_options: &[ComponentPromptOption],
    selected_indices: &[usize],
) {
    for (index, option) in prompt_options.iter().enumerate() {
        components.set_enabled(option.component, selected_indices.contains(&index));
    }
}

fn docs_enabled(args: &NewArgs) -> bool {
    args.docs
}

fn codecov_enabled(args: &NewArgs) -> bool {
    args.codecov.unwrap_or(true)
}

fn pypi_publish_enabled(args: &NewArgs) -> bool {
    args.pypi_publish.unwrap_or(false)
}

pub(crate) fn validate_explicit_options(blueprint: BlueprintName, args: &NewArgs) -> Result<()> {
    if !args.github {
        reject_if_requires("github-owner", args.github_owner.is_some(), "--github")?;
        reject_if_requires(
            "github-visibility",
            args.github_visibility.is_some(),
            "--github",
        )?;
    }

    if blueprint == BlueprintName::AnyProject {
        reject_if_present(blueprint, "package-name", args.package_name.is_some())?;
        reject_if_present(blueprint, "author-name", args.author_name.is_some())?;
        reject_if_present(blueprint, "author-email", args.author_email.is_some())?;
        reject_if_present(blueprint, "license", args.license.is_some())?;
    }

    if blueprint != BlueprintName::PythonLibrary {
        reject_if_present(blueprint, "python-min", args.python_min.is_some())?;
    }

    if !blueprint.supports_option(ManagedOption::Codecov) && args.codecov.is_some() {
        return Err(coded_error(
            ErrorCode::Input,
            format!(
                "option '{}' is not supported by {}",
                ManagedOption::Codecov.as_str(),
                blueprint.as_str()
            ),
        ));
    }

    if !blueprint.supports_option(ManagedOption::PypiPublish) && args.pypi_publish.is_some() {
        return Err(coded_error(
            ErrorCode::Input,
            format!(
                "option '{}' is not supported by {}",
                ManagedOption::PypiPublish.as_str(),
                blueprint.as_str()
            ),
        ));
    }

    Ok(())
}

pub(crate) fn validate_required_fields_for_yes(
    blueprint: BlueprintName,
    args: &NewArgs,
) -> Result<()> {
    if !args.yes {
        return Ok(());
    }

    let missing_fields = blueprint
        .creation_fields()
        .iter()
        .filter(|field| field.required && !creation_field_is_present(field.name, args))
        .map(|field| format!("--{}", field.name))
        .collect::<Vec<_>>();

    if missing_fields.is_empty() {
        return Ok(());
    }

    Err(coded_error(
        ErrorCode::Input,
        format!(
            "missing required options for --yes: {} (or run without --yes)",
            missing_fields.join(", ")
        ),
    ))
}

fn creation_field_is_present(field: &str, args: &NewArgs) -> bool {
    match field {
        "project-name" => args.project_name.is_some(),
        "package-name" => args.package_name.is_some(),
        "description" => args.description.is_some(),
        "author-name" => args.author_name.is_some(),
        "author-email" => args.author_email.is_some(),
        "license" => args.license.is_some(),
        "python-min" => args.python_min.is_some(),
        _ => false,
    }
}

fn reject_if_requires(option: &str, present: bool, required_option: &str) -> Result<()> {
    if present {
        return Err(coded_error(
            ErrorCode::Input,
            format!("option '{option}' requires {required_option}"),
        ));
    }

    Ok(())
}

fn reject_if_present(blueprint: BlueprintName, option: &str, present: bool) -> Result<()> {
    if present {
        return Err(coded_error(
            ErrorCode::Input,
            format!(
                "option '{option}' is not supported by {}",
                blueprint.as_str()
            ),
        ));
    }

    Ok(())
}

fn destination_path(project_name: &str, path: &Option<PathBuf>) -> Result<PathBuf> {
    match path {
        Some(path) => Ok(path.clone()),
        None => Ok(std::env::current_dir()?.join(project_name)),
    }
}

fn validate_destination_is_available(path: &Path) -> Result<()> {
    if path.exists() {
        let metadata = fs::metadata(path)
            .with_context(|| format!("failed to read destination {}", path.display()))?;
        if !metadata.is_dir() {
            return Err(coded_error(
                ErrorCode::Input,
                format!(
                    "destination path is not a directory: {}; choose a directory path",
                    path.display()
                ),
            ));
        }

        let mut entries = fs::read_dir(path)
            .with_context(|| format!("failed to read destination {}", path.display()))?;
        if entries.next().is_some() {
            return Err(coded_error(
                ErrorCode::Conflict,
                format!(
                    "destination already exists and is not empty: {}",
                    path.display()
                ),
            ));
        }
    }
    Ok(())
}

fn ensure_destination_directory_exists(path: &Path) -> Result<()> {
    fs::create_dir_all(path)
        .with_context(|| format!("failed to create destination {}", path.display()))?;
    Ok(())
}

fn write_project_files(destination: &Path, files: GeneratedFiles) -> Result<()> {
    for (relative_path, generated_file) in files {
        let path = managed_file_path(destination, &relative_path)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        write_generated_file(&path, &generated_file)?;
    }
    Ok(())
}

fn lock_dependencies_before_push(
    destination: &Path,
    github_requested: bool,
    quiet: bool,
) -> Result<bool> {
    if !github_requested {
        return Ok(false);
    }

    run_command(destination, "uv", &["lock"], quiet).with_context(|| {
        format!(
            "dependency locking failed after local project generation; run `cd {}` and retry `uv lock` before creating the GitHub repository",
            ui::shell_arg(destination.display().to_string())
        )
    })?;
    Ok(true)
}

fn initialize_git_repository(
    destination: &Path,
    github_requested: bool,
    quiet: bool,
) -> Result<()> {
    run_command(destination, "git", &["init", "-b", "main"], quiet)?;
    run_command(destination, "git", &["add", "."], quiet)?;

    if github_requested {
        commit_initial_files(destination, quiet)?;
    } else {
        let _ = commit_initial_files(destination, quiet);
    }

    Ok(())
}

fn commit_initial_files(destination: &Path, quiet: bool) -> Result<()> {
    run_command(
        destination,
        "git",
        &["commit", "-m", "chore: initialize project with forge"],
        quiet,
    )
}

fn create_github_repo(
    destination: &Path,
    project_name: &str,
    github_owner: Option<String>,
    visibility: GithubVisibility,
    assume_yes: bool,
    quiet: bool,
) -> Result<()> {
    ensure_gh_ready(assume_yes)?;

    let owner = match github_owner {
        Some(owner) => owner,
        None => detect_github_owner()?,
    };
    let repo = format!("{owner}/{project_name}");
    let visibility_flag = match visibility {
        GithubVisibility::Public => "--public",
        GithubVisibility::Private => "--private",
    };

    run_command(
        destination,
        "gh",
        &[
            "repo",
            "create",
            &repo,
            "--source",
            ".",
            "--remote",
            "origin",
            visibility_flag,
            "--push",
        ],
        quiet,
    )
    .with_context(|| {
        format!(
            "GitHub repository creation failed after local project generation; run `cd {}` and retry `gh repo create {repo} --source . --remote origin {visibility_flag} --push`",
            ui::shell_arg(destination.display().to_string())
        )
    })?;

    if !quiet {
        ui::success(format!("created and pushed GitHub repo {repo}"));
    }
    Ok(())
}

fn ensure_gh_ready(assume_yes: bool) -> Result<()> {
    ensure_gh_installed(assume_yes)?;
    ensure_gh_authenticated(assume_yes)?;
    Ok(())
}

fn ensure_gh_installed(assume_yes: bool) -> Result<()> {
    if command_exists("gh") {
        return Ok(());
    }

    if assume_yes {
        return Err(coded_error(
            ErrorCode::Env,
            "GitHub CLI is not installed. Install gh first: https://cli.github.com/",
        ));
    }

    let should_install = Confirm::new()
        .with_prompt("GitHub CLI (gh) is missing. Install it now?")
        .default(true)
        .interact()?;

    if !should_install {
        return Err(coded_error(
            ErrorCode::Input,
            "gh is required for GitHub repo creation",
        ));
    }

    try_install_gh()
}

fn ensure_gh_authenticated(assume_yes: bool) -> Result<()> {
    if gh_is_authenticated()? {
        return Ok(());
    }

    if assume_yes {
        return Err(coded_error(
            ErrorCode::Env,
            "GitHub CLI is not authenticated. Run `gh auth login` and retry.",
        ));
    }

    let should_login = Confirm::new()
        .with_prompt("Run `gh auth login` now?")
        .default(true)
        .interact()?;

    if !should_login {
        return Err(coded_error(
            ErrorCode::Input,
            "gh authentication is required for GitHub repo creation",
        ));
    }

    let status = Command::new("gh").args(["auth", "login"]).status()?;
    if !status.success() {
        return Err(coded_error(ErrorCode::Env, "`gh auth login` failed"));
    }

    Ok(())
}

fn try_install_gh() -> Result<()> {
    if command_exists("brew") {
        run_command(Path::new("."), "brew", &["install", "gh"], false)?;
        return Ok(());
    }
    if command_exists("apt-get") {
        run_command(Path::new("."), "sudo", &["apt-get", "update"], false)?;
        run_command(
            Path::new("."),
            "sudo",
            &["apt-get", "install", "-y", "gh"],
            false,
        )?;
        return Ok(());
    }
    if command_exists("dnf") {
        run_command(
            Path::new("."),
            "sudo",
            &["dnf", "install", "-y", "gh"],
            false,
        )?;
        return Ok(());
    }

    Err(coded_error(
        ErrorCode::Env,
        "automatic gh installation is unsupported on this OS. Install manually: https://cli.github.com/",
    ))
}

fn detect_github_owner() -> Result<String> {
    let output = Command::new("gh")
        .args(["api", "user", "-q", ".login"])
        .output()?;

    if !output.status.success() {
        return Err(coded_error(
            ErrorCode::Env,
            "failed to detect GitHub owner using `gh api user`",
        ));
    }

    let login = String::from_utf8(output.stdout)?.trim().to_string();
    if login.is_empty() {
        return Err(coded_error(ErrorCode::Env, "detected empty GitHub owner"));
    }
    Ok(login)
}

fn command_exists(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

fn gh_is_authenticated() -> Result<bool> {
    let output = Command::new("gh").args(["auth", "status"]).output()?;
    Ok(output.status.success())
}

fn run_command(cwd: &Path, program: &str, args: &[&str], quiet: bool) -> Result<()> {
    let mut command = Command::new(program);
    command.current_dir(cwd).args(args).stdin(Stdio::inherit());
    if quiet {
        command.stdout(Stdio::null()).stderr(Stdio::inherit());
    } else {
        command.stdout(Stdio::inherit()).stderr(Stdio::inherit());
    }

    let status = command.status().map_err(|error| {
        coded_error(
            ErrorCode::Env,
            format!("failed to execute {program}: {error}"),
        )
    })?;

    if !status.success() {
        return Err(coded_error(
            ErrorCode::Env,
            format!("command failed: {} {}", program, args.join(" ")),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::{CodedError, ErrorCode};
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn new_status_code_covers_json_outcomes() {
        assert_eq!(new_status_code(false), "created");
        assert_eq!(new_status_code(true), "dry_run");
    }

    #[test]
    fn destination_path_uses_provided_path() {
        let provided = Some(PathBuf::from("/custom/path"));
        let result = destination_path("my-project", &provided).unwrap();
        assert_eq!(result, PathBuf::from("/custom/path"));
    }

    #[test]
    fn destination_path_defaults_to_project_name() {
        let result = destination_path("my-cool-project", &None).unwrap();
        assert!(result.ends_with("my-cool-project"));
    }

    #[test]
    fn destination_validation_rejects_nonempty_directory() {
        let temp = TempDir::new().expect("temp dir should create");
        let destination = temp.path().join("existing");
        fs::create_dir_all(&destination).expect("destination dir should create");
        fs::write(destination.join("README.md"), "existing").expect("file should write");

        let error = validate_destination_is_available(&destination)
            .expect_err("non-empty destination should be rejected");

        assert!(
            error
                .to_string()
                .contains("destination already exists and is not empty")
        );
    }

    #[test]
    fn ensure_destination_directory_exists_creates_missing_directory() {
        let temp = TempDir::new().expect("temp dir should create");
        let destination = temp.path().join("new-project");

        ensure_destination_directory_exists(&destination)
            .expect("missing destination should be created");

        assert!(destination.is_dir());
    }

    #[test]
    fn select_blueprint_uses_explicit_value() {
        let blueprint = select_blueprint(Some(BlueprintName::RustLibrary), true).unwrap();
        assert_eq!(blueprint, BlueprintName::RustLibrary);
    }

    #[test]
    fn select_blueprint_requires_explicit_value_in_yes_mode() {
        let error = select_blueprint(None, true).expect_err("missing blueprint should fail");
        assert!(error.to_string().contains("--blueprint"));
    }

    #[test]
    fn interactive_setup_requires_terminal_without_yes() {
        let error = ensure_interactive_setup_allowed(false, false, false, false)
            .expect_err("non-interactive prompt should fail");
        assert!(
            error
                .to_string()
                .contains("interactive confirmation requires a terminal")
        );
        assert!(error.to_string().contains("--dry-run"));
        assert!(error.to_string().contains("--json"));
    }

    #[test]
    fn interactive_setup_allows_terminal_or_yes_mode() {
        ensure_interactive_setup_allowed(false, false, false, true)
            .expect("terminal setup should be allowed");
        ensure_interactive_setup_allowed(true, false, false, false)
            .expect("--yes should bypass prompts");
        ensure_interactive_setup_allowed(false, true, false, false)
            .expect("--json should bypass interactive confirmation");
        ensure_interactive_setup_allowed(false, false, true, false)
            .expect("--dry-run should bypass interactive confirmation");
    }

    #[test]
    fn default_python_package_name_rejects_invalid_derived_name() {
        let error = default_python_package_name("123-tools").unwrap_err();
        assert!(error.to_string().contains("pass --package-name"));
    }

    #[test]
    fn interactive_prompt_validators_match_supported_metadata_rules() {
        assert!(is_non_empty_text("project"));
        assert!(!is_non_empty_text("   "));

        assert!(is_supported_license("BSD-3-Clause"));
        assert!(is_supported_license("MIT"));
        assert!(is_supported_license("Apache-2.0"));
        assert!(!is_supported_license("GPL-3.0-only"));
    }

    #[test]
    fn new_command_preserves_setup_flags_and_drops_preview_flags() {
        let args = NewArgs {
            blueprint: Some(BlueprintName::PythonLibrary),
            path: Some(PathBuf::from("/tmp/grid tools")),
            project_name: Some("grid-tools".to_string()),
            package_name: Some("grid_tools".to_string()),
            description: Some("Grid toolchain".to_string()),
            author_name: Some("Ada Lovelace".to_string()),
            author_email: Some("ada@example.com".to_string()),
            license: Some("MIT".to_string()),
            python_min: Some("3.12".to_string()),
            docs: false,
            codecov: Some(false),
            pypi_publish: Some(true),
            prettier: true,
            editorconfig: true,
            markdownlint: false,
            github: true,
            github_owner: Some("example-org".to_string()),
            github_visibility: Some(GithubVisibility::Private),
            json: true,
            dry_run: true,
            diff: true,
            yes: true,
        };

        let command = new_command(
            &args,
            BlueprintName::PythonLibrary,
            Path::new("/tmp/grid tools"),
        );

        assert_eq!(
            command,
            "forge new --path '/tmp/grid tools' --blueprint python-library --project-name grid-tools --package-name grid_tools --description 'Grid toolchain' --author-name 'Ada Lovelace' --author-email 'ada@example.com' --license MIT --python-min 3.12 --docs=false --codecov=false --pypi-publish=true --prettier --editorconfig --github --github-owner example-org --github-visibility private --yes"
        );
        assert!(!command.contains("--json"));
        assert!(!command.contains("--dry-run"));
        assert!(!command.contains("--diff"));
    }

    #[test]
    fn managed_option_flags_use_canonical_boolean_forms() {
        let mut parts = vec![];

        push_managed_option_flags(
            &mut parts,
            ManagedOptionFlags {
                docs: false,
                codecov: Some(false),
                pypi_publish: Some(true),
                prettier: true,
                editorconfig: true,
                markdownlint: false,
            },
        );

        assert_eq!(
            parts,
            vec![
                "--docs=false".to_string(),
                "--codecov=false".to_string(),
                "--pypi-publish=true".to_string(),
                "--prettier".to_string(),
                "--editorconfig".to_string(),
            ]
        );
        assert!(!parts.iter().any(|part| part == "--no-docs"));
        assert!(!parts.iter().any(|part| part == "--no-codecov"));
    }

    #[test]
    fn managed_option_flags_skip_defaults() {
        let mut parts = vec![];

        push_managed_option_flags(
            &mut parts,
            ManagedOptionFlags {
                docs: true,
                codecov: None,
                pypi_publish: None,
                prettier: false,
                editorconfig: false,
                markdownlint: false,
            },
        );

        assert!(parts.is_empty());
    }

    #[test]
    fn component_selection_from_args_reflects_component_flags() {
        let args = NewArgs {
            blueprint: Some(BlueprintName::AnyProject),
            path: None,
            project_name: Some("repo-infra".to_string()),
            package_name: None,
            description: Some("Shared infrastructure".to_string()),
            author_name: None,
            author_email: None,
            license: None,
            python_min: None,
            docs: true,
            codecov: None,
            pypi_publish: None,
            prettier: true,
            editorconfig: false,
            markdownlint: false,
            github: false,
            github_owner: None,
            github_visibility: None,
            json: false,
            dry_run: false,
            diff: false,
            yes: true,
        };

        let selection = component_selection_from_args(&args);

        assert!(selection.is_enabled(ManagedComponent::Prettier));
        assert!(!selection.is_enabled(ManagedComponent::Editorconfig));
    }

    #[test]
    fn component_prompt_options_keep_component_order_and_defaults() {
        let selection = ComponentSelection::from_flags(true, false, true);

        let options = supported_component_prompt_options(&selection, BlueprintName::AnyProject);

        assert_eq!(options.len(), 3);
        assert_eq!(options[0].component, ManagedComponent::Prettier);
        assert_eq!(options[1].component, ManagedComponent::Editorconfig);
        assert_eq!(options[2].component, ManagedComponent::Markdownlint);
        assert_eq!(
            options
                .iter()
                .map(|option| option.default_enabled)
                .collect::<Vec<_>>(),
            vec![true, false, true]
        );
        assert!(options[0].label.contains("prettier"));
        assert!(options[1].label.contains("editorconfig"));
        assert!(options[2].label.contains("markdownlint"));
    }

    #[test]
    fn applying_component_prompt_selection_replaces_supported_component_state() {
        let mut selection = ComponentSelection::from_flags(true, true, true);
        let prompt_options =
            supported_component_prompt_options(&selection, BlueprintName::PythonLibrary);

        apply_component_prompt_selection(&mut selection, &prompt_options, &[1]);

        assert!(!selection.is_enabled(ManagedComponent::Prettier));
        assert!(selection.is_enabled(ManagedComponent::Editorconfig));
        assert!(!selection.is_enabled(ManagedComponent::Markdownlint));
    }

    #[test]
    fn format_selected_options_groups_enabled_and_disabled() {
        let options = vec![
            SelectedOption::new(ManagedOption::Docs, true),
            SelectedOption::new(ManagedOption::Prettier, false),
            SelectedOption::new(ManagedOption::Editorconfig, true),
        ];

        assert_eq!(
            format_selected_options(&options),
            "enabled: docs, editorconfig; disabled: prettier"
        );
    }

    #[test]
    fn format_selected_options_handles_empty_groups() {
        let options = vec![SelectedOption::new(ManagedOption::Docs, true)];
        assert_eq!(
            format_selected_options(&options),
            "enabled: docs; disabled: none"
        );

        let options = vec![SelectedOption::new(ManagedOption::Docs, false)];
        assert_eq!(
            format_selected_options(&options),
            "enabled: none; disabled: docs"
        );
    }

    #[test]
    fn render_blueprint_reports_supported_option_set_for_all_blueprints() {
        let any_args = NewArgs {
            blueprint: Some(BlueprintName::AnyProject),
            path: None,
            project_name: Some("repo-infra".to_string()),
            package_name: None,
            description: Some("Shared infrastructure".to_string()),
            author_name: None,
            author_email: None,
            license: None,
            python_min: None,
            docs: true,
            codecov: None,
            pypi_publish: None,
            prettier: false,
            editorconfig: false,
            markdownlint: false,
            github: false,
            github_owner: None,
            github_visibility: None,
            json: false,
            dry_run: false,
            diff: false,
            yes: true,
        };
        let any_render =
            render_blueprint(&any_args, BlueprintName::AnyProject, RenderScope::Project)
                .expect("any-project should render");
        assert_eq!(
            any_render
                .options
                .iter()
                .map(|option| option.name)
                .collect::<Vec<_>>(),
            BlueprintName::AnyProject
                .supported_options()
                .iter()
                .map(|option| option.as_str())
                .collect::<Vec<_>>()
        );

        let python_args = NewArgs {
            blueprint: Some(BlueprintName::PythonLibrary),
            path: None,
            project_name: Some("grid-tools".to_string()),
            package_name: Some("grid_tools".to_string()),
            description: Some("Grid toolchain".to_string()),
            author_name: Some("Ada Lovelace".to_string()),
            author_email: Some("ada@example.com".to_string()),
            license: Some("MIT".to_string()),
            python_min: Some("3.12".to_string()),
            docs: true,
            codecov: Some(true),
            pypi_publish: Some(false),
            prettier: true,
            editorconfig: true,
            markdownlint: false,
            github: false,
            github_owner: None,
            github_visibility: None,
            json: false,
            dry_run: false,
            diff: false,
            yes: true,
        };
        let python_render = render_blueprint(
            &python_args,
            BlueprintName::PythonLibrary,
            RenderScope::Project,
        )
        .expect("python-library should render");
        assert_eq!(
            python_render
                .options
                .iter()
                .map(|option| option.name)
                .collect::<Vec<_>>(),
            BlueprintName::PythonLibrary
                .supported_options()
                .iter()
                .map(|option| option.as_str())
                .collect::<Vec<_>>()
        );

        let rust_args = NewArgs {
            blueprint: Some(BlueprintName::RustLibrary),
            path: None,
            project_name: Some("grid-rs".to_string()),
            package_name: Some("grid_rs".to_string()),
            description: Some("Grid utilities for Rust".to_string()),
            author_name: Some("Ferris Engineer".to_string()),
            author_email: Some("ferris@example.com".to_string()),
            license: Some("MIT".to_string()),
            python_min: None,
            docs: true,
            codecov: None,
            pypi_publish: None,
            prettier: false,
            editorconfig: false,
            markdownlint: false,
            github: false,
            github_owner: None,
            github_visibility: None,
            json: false,
            dry_run: false,
            diff: false,
            yes: true,
        };
        let rust_render =
            render_blueprint(&rust_args, BlueprintName::RustLibrary, RenderScope::Project)
                .expect("rust-library should render");
        assert_eq!(
            rust_render
                .options
                .iter()
                .map(|option| option.name)
                .collect::<Vec<_>>(),
            BlueprintName::RustLibrary
                .supported_options()
                .iter()
                .map(|option| option.as_str())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn command_exists_finds_common_commands() {
        // These should exist on most systems
        assert!(command_exists("cargo"));
        assert!(command_exists("rustc"));
    }

    #[test]
    fn command_exists_returns_false_for_fake() {
        assert!(!command_exists("definitely-not-a-real-command-12345"));
    }

    #[test]
    fn run_command_reports_env_code_for_non_zero_exit() {
        let error = run_command(Path::new("."), "sh", &["-c", "exit 7"], true)
            .expect_err("non-zero command should fail");
        let coded = error
            .downcast_ref::<CodedError>()
            .expect("run_command errors should be typed");
        assert_eq!(coded.code(), ErrorCode::Env);
    }

    #[test]
    fn interactive_confirmation_gate_respects_yes_and_json_modes() {
        assert!(!should_confirm_interactive_setup(true, false, false));
        assert!(!should_confirm_interactive_setup(true, true, false));
        assert!(!should_confirm_interactive_setup(false, true, false));
        assert!(!should_confirm_interactive_setup(false, false, true));
        assert!(should_confirm_interactive_setup(false, false, false));
    }

    #[test]
    fn confirm_interactive_setup_returns_true_when_confirmation_is_bypassed() {
        let options = [SelectedOption::new(ManagedOption::Docs, true)];
        let confirmed = confirm_interactive_setup(
            false,
            false,
            true,
            SetupReview {
                section_title: "Project setup review",
                path: Path::new("/tmp/repo"),
                blueprint: BlueprintName::AnyProject,
                options: &options,
                prompt: "Create this project?",
                context: vec![SetupReviewItem::new("apply", "forge new --yes ...")],
            },
        )
        .expect("dry-run confirmation should be bypassed");

        assert!(confirmed);
    }

    #[test]
    fn setup_review_context_includes_apply_command() {
        let args = NewArgs {
            blueprint: Some(BlueprintName::AnyProject),
            path: Some(PathBuf::from("/tmp/repo")),
            project_name: Some("repo-infra".to_string()),
            package_name: None,
            description: Some("Shared infrastructure".to_string()),
            author_name: None,
            author_email: None,
            license: None,
            python_min: None,
            docs: true,
            codecov: None,
            pypi_publish: None,
            prettier: false,
            editorconfig: false,
            markdownlint: false,
            github: false,
            github_owner: None,
            github_visibility: None,
            json: false,
            dry_run: false,
            diff: false,
            yes: true,
        };
        let project = ProjectRender {
            project_name: "repo-infra".to_string(),
            options: vec![SelectedOption::new(ManagedOption::Docs, true)],
            files: GeneratedFiles::from([
                (
                    PathBuf::from("pyproject.toml"),
                    crate::blueprint::files::GeneratedFile::text("[tool.forge]\n".to_string()),
                ),
                (
                    PathBuf::from(".pre-commit-config.yaml"),
                    crate::blueprint::files::GeneratedFile::text("repos: []\n".to_string()),
                ),
                (
                    PathBuf::from("AGENTS.md"),
                    crate::blueprint::files::GeneratedFile::text("# Agents\n".to_string()),
                ),
                (
                    PathBuf::from("CLAUDE.md"),
                    crate::blueprint::files::GeneratedFile::symlink("AGENTS.md"),
                ),
                (
                    PathBuf::from("docs/package.json"),
                    crate::blueprint::files::GeneratedFile::text("site_name: Repo\n".to_string()),
                ),
                (
                    PathBuf::from(".github/workflows/ci.yaml"),
                    crate::blueprint::files::GeneratedFile::text("name: CI\n".to_string()),
                ),
            ]),
        };

        let context = new_setup_review_context(
            &args,
            BlueprintName::AnyProject,
            &project,
            "forge new --path /tmp/repo --blueprint any-project --project-name repo-infra --description 'Shared infrastructure' --yes",
        );

        assert!(context.iter().any(|item| item.label == "files"));
        let infrastructure = context
            .iter()
            .find(|item| item.label == "infrastructure")
            .expect("infrastructure summary should be present");
        assert!(infrastructure.value.contains("pyproject.toml"));
        assert!(infrastructure.value.contains("prek hooks"));
        assert!(infrastructure.value.contains("AGENTS.md"));
        assert!(infrastructure.value.contains("CLAUDE.md link"));
        assert!(infrastructure.value.contains("docs"));
        assert!(infrastructure.value.contains("github actions (1)"));
        let required_tools = context
            .iter()
            .find(|item| item.label == "required tools")
            .expect("required tools should be present");
        assert_eq!(required_tools.value, "uv, just");
        assert!(context.iter().any(|item| item.label == "github"));
        let apply = context
            .iter()
            .find(|item| item.label == "apply")
            .expect("apply command should be present");
        assert_eq!(
            apply.value,
            "forge new --path /tmp/repo --blueprint any-project --project-name repo-infra --description 'Shared infrastructure' --yes"
        );
    }

    #[test]
    fn required_tools_summary_includes_component_tools_for_enabled_options() {
        let options = vec![
            SelectedOption::new(ManagedOption::Docs, true),
            SelectedOption::new(ManagedOption::Prettier, true),
            SelectedOption::new(ManagedOption::Markdownlint, true),
        ];

        let summary = required_tools_summary_for_options(BlueprintName::AnyProject, &options);

        assert_eq!(summary, "uv, just, npx");
    }

    #[test]
    fn resolved_new_args_from_rendered_pyproject_prefers_rendered_metadata() {
        let args = NewArgs {
            blueprint: Some(BlueprintName::PythonLibrary),
            path: Some(PathBuf::from("/tmp/grid-tools")),
            project_name: None,
            package_name: None,
            description: None,
            author_name: None,
            author_email: None,
            license: None,
            python_min: None,
            docs: true,
            codecov: None,
            pypi_publish: None,
            prettier: false,
            editorconfig: false,
            markdownlint: false,
            github: false,
            github_owner: None,
            github_visibility: None,
            json: false,
            dry_run: true,
            diff: false,
            yes: false,
        };
        let project = ProjectRender {
            project_name: "grid-tools".to_string(),
            options: vec![],
            files: GeneratedFiles::from([(
                PathBuf::from("pyproject.toml"),
                crate::blueprint::files::GeneratedFile::text(
                    r#"[tool.forge]
blueprint = "python-library"
project_name = "grid-tools"
package_name = "grid_tools"
description = "Grid toolchain"
author_name = "Ada Lovelace"
author_email = "ada@example.com"
license = "MIT"
python_min = "3.12"

[tool.forge.overrides]
docs = false
codecov = false
pypi-publish = true
prettier = true
editorconfig = true
markdownlint = true
"#,
                ),
            )]),
        };

        let resolved = resolved_new_args_from_rendered_pyproject(&args, &project)
            .expect("rendered pyproject should resolve args");

        assert_eq!(resolved.project_name.as_deref(), Some("grid-tools"));
        assert_eq!(resolved.package_name.as_deref(), Some("grid_tools"));
        assert_eq!(resolved.description.as_deref(), Some("Grid toolchain"));
        assert_eq!(resolved.author_name.as_deref(), Some("Ada Lovelace"));
        assert_eq!(resolved.author_email.as_deref(), Some("ada@example.com"));
        assert_eq!(resolved.license.as_deref(), Some("MIT"));
        assert_eq!(resolved.python_min.as_deref(), Some("3.12"));
        assert!(!resolved.docs);
        assert_eq!(resolved.codecov, Some(false));
        assert_eq!(resolved.pypi_publish, Some(true));
        assert!(resolved.prettier);
        assert!(resolved.editorconfig);
        assert!(resolved.markdownlint);
    }
}
