use std::collections::BTreeSet;
use std::fs;
use std::io::IsTerminal;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::blueprint::files::{
    GeneratedFile, GeneratedFiles, ManagedFileAction, ManagedFileConflict, count_changes,
    count_conflicts, plan_generated_files, write_generated_files,
};
use crate::blueprint::{
    BlueprintName, detect_blueprint_metadata_from_pyproject, forge_metadata_is_python_library,
    minimal_external_pyproject_metadata, python_library, rust_library,
};
use crate::cli::{GithubVisibility, InitArgs, NewArgs};
use crate::commands::diff;
use crate::commands::new::{self, ProjectRender, RenderScope};
use crate::errors::{ErrorCode, coded_error};
use crate::ui;

const PYPROJECT_TOML: &str = "pyproject.toml";
const FORGE_DEPENDENCY_GROUP: &str = "forge";

pub fn run(mut args: InitArgs) -> Result<()> {
    let stdin_is_terminal = std::io::stdin().is_terminal();
    new::ensure_interactive_setup_allowed(args.yes, args.json, args.dry_run, stdin_is_terminal)?;
    crate::commands::validate_diff_mode(args.diff, args.dry_run, false)?;
    ensure_existing_directory(&args.path)?;
    ensure_not_already_managed(&args.path)?;

    let blueprint = new::select_blueprint(args.blueprint, args.yes)?;
    apply_existing_project_defaults(&mut args, blueprint)?;
    let render_args = new_args_from_init_args(&args);
    new::validate_explicit_options(blueprint, &render_args)?;
    new::validate_required_fields_for_yes(blueprint, &render_args)?;
    let mut project =
        new::render_blueprint(&render_args, blueprint, RenderScope::ManagedInfrastructure)?;
    let adopted_existing_pyproject = if args.force {
        false
    } else {
        adopt_existing_pyproject(&args.path, &mut project.files)?
    };
    let infrastructure = new::managed_infrastructure_summary(&project.files);
    let apply_command =
        preview_init_command(&args, blueprint, &project, args.force, stdin_is_terminal);
    let mut actions = plan_generated_files(&args.path, &project.files);
    if !args.force {
        mark_existing_files_as_conflicts(&mut actions, adopted_existing_pyproject);
    }
    let changes = count_changes(&actions);
    let conflicts = count_conflicts(&actions);

    if args.json {
        print_json_report(
            &args,
            &args.path,
            blueprint,
            args.dry_run,
            args.force,
            &project,
            &actions,
        )?;
    } else {
        ui::section(if args.dry_run {
            "Repository initialization preview"
        } else {
            "Repository initialization"
        });
        ui::info("path", args.path.display());
        ui::info("blueprint", blueprint.as_str());
        ui::info("blueprint version", blueprint.version());
        ui::info("options", new::format_selected_options(&project.options));
        ui::info(
            "required tools",
            new::required_tools_summary_for_options(blueprint, &project.options),
        );
        ui::info("infrastructure", &infrastructure);
        ui::info("force", args.force);
        print_actions(&actions);
        if args.diff {
            diff::print_diffs(&args.path, &actions, &project.files)?;
        }
    }

    if conflicts > 0 {
        if !args.json {
            print_next_steps(&args, blueprint, conflicts, args.dry_run);
        }
        return Err(coded_error(
            ErrorCode::Conflict,
            "existing files would be overwritten; rerun with --force after reviewing the plan",
        ));
    }

    if !new::confirm_interactive_setup(
        args.yes,
        args.json,
        args.dry_run,
        new::SetupReview {
            section_title: "Repository setup review",
            path: &args.path,
            blueprint,
            options: &project.options,
            prompt: "Apply Forge-managed infrastructure to this repository?",
            context: init_setup_review_context(
                &args,
                blueprint,
                &project.options,
                &project.files,
                changes,
                conflicts,
                &apply_command,
            ),
        },
    )? {
        if !args.json {
            ui::section("Repository initialization canceled");
            ui::success("no files changed");
        }
        return Ok(());
    }

    if !args.dry_run {
        write_generated_files(&args.path, project.files)?;
    }

    if !args.json {
        ui::section(if args.dry_run {
            "Repository checked"
        } else {
            "Repository initialized"
        });
        if args.dry_run {
            ui::success("dry run complete; no files changed");
        } else {
            ui::success("managed infrastructure added");
        }
        ui::info("infrastructure", infrastructure);
        ui::info(
            "required tools",
            new::required_tools_summary_for_options(blueprint, &project.options),
        );
        ui::info("managed sync", "forge sync --path .");
        print_next_steps(&args, blueprint, conflicts, args.dry_run);
    }

    Ok(())
}

fn apply_existing_project_defaults(args: &mut InitArgs, blueprint: BlueprintName) -> Result<()> {
    match blueprint {
        BlueprintName::AnyProject | BlueprintName::PythonLibrary => {
            apply_pyproject_project_defaults(args, blueprint == BlueprintName::PythonLibrary)?;
        }
        BlueprintName::RustLibrary => {
            apply_cargo_package_defaults(args)?;
        }
    }
    Ok(())
}

fn apply_pyproject_project_defaults(args: &mut InitArgs, infer_python_min: bool) -> Result<()> {
    let pyproject_path = args.path.join(PYPROJECT_TOML);
    if !pyproject_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(&pyproject_path)
        .with_context(|| format!("failed to read {}", pyproject_path.display()))?;
    let Ok(parsed) = toml::from_str::<toml::Value>(&content) else {
        return Ok(());
    };
    let Some(project) = parsed.get("project").and_then(toml::Value::as_table) else {
        return Ok(());
    };

    fill_if_missing(&mut args.project_name, project_string(project, "name"));
    fill_if_missing(
        &mut args.description,
        project_string(project, "description"),
    );
    if infer_python_min
        && args.python_min.is_none()
        && let Some(requires_python) = project_string(project, "requires-python")
    {
        args.python_min = minimum_python_from_requires_python(&requires_python);
    }

    Ok(())
}

fn apply_cargo_package_defaults(args: &mut InitArgs) -> Result<()> {
    let cargo_path = args.path.join("Cargo.toml");
    if !cargo_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(&cargo_path)
        .with_context(|| format!("failed to read {}", cargo_path.display()))?;
    let Ok(parsed) = toml::from_str::<toml::Value>(&content) else {
        return Ok(());
    };
    let Some(package) = parsed.get("package").and_then(toml::Value::as_table) else {
        return Ok(());
    };

    fill_if_missing(&mut args.project_name, project_string(package, "name"));
    fill_if_missing(
        &mut args.description,
        project_string(package, "description"),
    );
    if args.package_name.is_none()
        && let Some(project_name) = args.project_name.as_deref()
    {
        args.package_name = Some(rust_library::default_crate_name(project_name));
    }

    Ok(())
}

fn project_string(table: &toml::Table, key: &str) -> Option<String> {
    table
        .get(key)
        .and_then(toml::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn fill_if_missing(target: &mut Option<String>, value: Option<String>) {
    if target.is_none() {
        *target = value;
    }
}

fn minimum_python_from_requires_python(requires_python: &str) -> Option<String> {
    requires_python
        .split(',')
        .map(str::trim)
        .find_map(|requirement| requirement.strip_prefix(">="))
        .map(str::trim)
        .filter(|version| python_library::is_valid_python_version(version))
        .map(str::to_string)
}

fn adopt_existing_pyproject(path: &Path, files: &mut GeneratedFiles) -> Result<bool> {
    let pyproject_path = path.join(PYPROJECT_TOML);
    if !pyproject_path.exists() {
        return Ok(false);
    }

    let Some(generated_pyproject) = files
        .get(Path::new(PYPROJECT_TOML))
        .and_then(GeneratedFile::as_text)
    else {
        return Ok(false);
    };
    let Some(forge_metadata) = forge_metadata_block(generated_pyproject) else {
        return Ok(false);
    };

    let existing = fs::read_to_string(&pyproject_path)
        .with_context(|| format!("failed to read {}", pyproject_path.display()))?;
    let adopted = append_forge_metadata(&existing, forge_metadata, generated_pyproject)?;
    toml::from_str::<toml::Value>(&adopted).with_context(|| {
        format!(
            "failed to merge Forge metadata into {}",
            pyproject_path.display()
        )
    })?;
    files.insert(
        Path::new(PYPROJECT_TOML).to_path_buf(),
        GeneratedFile::text(adopted),
    );
    Ok(true)
}

fn forge_metadata_block(pyproject: &str) -> Option<&str> {
    pyproject
        .find("[tool.forge]")
        .map(|start| &pyproject[start..])
}

fn append_forge_metadata(
    existing: &str,
    forge_metadata: &str,
    generated_pyproject: &str,
) -> Result<String> {
    let mut adopted = append_forge_dependency_group(existing, generated_pyproject)?;
    if !adopted.ends_with('\n') {
        adopted.push('\n');
    }
    if !adopted.ends_with("\n\n") {
        adopted.push('\n');
    }
    adopted.push_str(&external_pyproject_metadata(existing, forge_metadata));
    Ok(adopted)
}

fn append_forge_dependency_group(existing: &str, generated_pyproject: &str) -> Result<String> {
    let dependencies = forge_dependency_group_values(generated_pyproject)?;
    if dependencies.is_empty() || dependency_group_exists(existing, FORGE_DEPENDENCY_GROUP)? {
        return Ok(existing.to_string());
    }

    let group = render_dependency_group(FORGE_DEPENDENCY_GROUP, &dependencies);
    if let Some((start, end)) = table_range(existing, "dependency-groups") {
        let mut adopted = String::with_capacity(existing.len() + group.len() + 1);
        adopted.push_str(&existing[..end]);
        if !existing[start..end].ends_with('\n') {
            adopted.push('\n');
        }
        adopted.push_str(&group);
        adopted.push_str(&existing[end..]);
        return Ok(adopted);
    }

    let mut adopted = existing.to_string();
    if !adopted.ends_with('\n') {
        adopted.push('\n');
    }
    if !adopted.ends_with("\n\n") {
        adopted.push('\n');
    }
    adopted.push_str("[dependency-groups]\n");
    adopted.push_str(&group);
    Ok(adopted)
}

fn forge_dependency_group_values(generated_pyproject: &str) -> Result<Vec<String>> {
    let parsed: toml::Value =
        toml::from_str(generated_pyproject).context("failed to parse generated pyproject.toml")?;
    let Some(groups) = parsed
        .get("dependency-groups")
        .and_then(toml::Value::as_table)
    else {
        return Ok(Vec::new());
    };

    let mut seen = BTreeSet::new();
    let mut dependencies = Vec::new();
    for group in groups.values() {
        let Some(group_dependencies) = group.as_array() else {
            continue;
        };
        for dependency in group_dependencies {
            let Some(dependency) = dependency.as_str() else {
                continue;
            };
            if seen.insert(dependency.to_string()) {
                dependencies.push(dependency.to_string());
            }
        }
    }
    Ok(dependencies)
}

fn dependency_group_exists(pyproject: &str, group_name: &str) -> Result<bool> {
    let parsed: toml::Value =
        toml::from_str(pyproject).context("failed to parse pyproject.toml")?;
    Ok(parsed
        .get("dependency-groups")
        .and_then(toml::Value::as_table)
        .and_then(|groups| groups.get(group_name))
        .is_some())
}

fn render_dependency_group(name: &str, dependencies: &[String]) -> String {
    let mut group = format!("{name} = [\n");
    for dependency in dependencies {
        group.push_str("    ");
        group.push_str(&crate::blueprint::toml_value::string_literal(dependency));
        group.push_str(",\n");
    }
    group.push_str("]\n");
    group
}

fn table_range(content: &str, table_name: &str) -> Option<(usize, usize)> {
    let header = format!("[{table_name}]");
    let start = content.find(&header)?;
    let after_header = start + header.len();
    let relative_end = content[after_header..]
        .find("\n[")
        .map(|index| after_header + index + 1)
        .unwrap_or(content.len());
    Some((start, relative_end))
}

fn external_pyproject_metadata(existing: &str, forge_metadata: &str) -> String {
    if existing_has_project_table(existing) && forge_metadata_is_python_library(forge_metadata) {
        return minimal_external_pyproject_metadata(forge_metadata);
    }

    let marker = "\n[tool.forge.overrides]";
    if let Some(index) = forge_metadata.find(marker) {
        let (forge_table, overrides) = forge_metadata.split_at(index);
        format!("{forge_table}managed_pyproject = false\n{overrides}")
    } else {
        let mut metadata = forge_metadata.to_string();
        if !metadata.ends_with('\n') {
            metadata.push('\n');
        }
        metadata.push_str("managed_pyproject = false\n");
        metadata
    }
}

fn existing_has_project_table(existing: &str) -> bool {
    toml::from_str::<toml::Value>(existing)
        .ok()
        .and_then(|parsed| parsed.get("project").cloned())
        .is_some()
}

fn new_args_from_init_args(args: &InitArgs) -> NewArgs {
    NewArgs {
        blueprint: args.blueprint,
        path: Some(args.path.clone()),
        project_name: args.project_name.clone(),
        package_name: args.package_name.clone(),
        description: args.description.clone(),
        author_name: args.author_name.clone(),
        author_email: args.author_email.clone(),
        license: args.license.clone(),
        python_min: args.python_min.clone(),
        gitignore_profile: args.gitignore_profile.clone(),
        docs: args.docs,
        codecov: args.codecov,
        pypi_publish: args.pypi_publish,
        prettier: args.prettier,
        editorconfig: args.editorconfig,
        markdownlint: args.markdownlint,
        no_git_history: false,
        github: false,
        github_owner: None,
        github_visibility: None::<GithubVisibility>,
        json: args.json,
        dry_run: args.dry_run,
        diff: args.diff,
        yes: args.yes,
    }
}

fn ensure_existing_directory(path: &Path) -> Result<()> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(coded_error(
                ErrorCode::Env,
                format!(
                    "repository path does not exist: {}; create it first or use `forge new --path {}`",
                    path.display(),
                    ui::shell_arg(path.display().to_string())
                ),
            ));
        }
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to read repository path {}", path.display()));
        }
    };
    if !metadata.is_dir() {
        return Err(coded_error(
            ErrorCode::Env,
            format!(
                "repository path is not a directory: {}; choose an existing repository directory or use `forge new --path {}`",
                path.display(),
                ui::shell_arg(path.display().to_string())
            ),
        ));
    }
    Ok(())
}

fn ensure_not_already_managed(path: &Path) -> Result<()> {
    let pyproject_path = path.join("pyproject.toml");
    if !pyproject_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(&pyproject_path)
        .with_context(|| format!("failed to read {}", pyproject_path.display()))?;
    let Ok(parsed) = toml::from_str::<toml::Value>(&content) else {
        return Ok(());
    };
    let has_forge_metadata = parsed
        .get("tool")
        .and_then(toml::Value::as_table)
        .and_then(|tool| tool.get("forge"))
        .is_some();
    if !has_forge_metadata {
        return Ok(());
    }

    let blueprint = match detect_blueprint_metadata_from_pyproject(&content) {
        Ok(metadata) => format!(" blueprint '{}'", metadata.name.as_str()),
        Err(error) => {
            let message = error.to_string();
            if message.contains("newer than this forge supports")
                || message.contains("blueprint_version")
            {
                return Err(error);
            }
            String::new()
        }
    };
    Err(coded_error(
        ErrorCode::Conflict,
        format!(
            "repository is already managed by forge{blueprint}; use `forge sync --path {}`",
            ui::shell_arg(path.display().to_string())
        ),
    ))
}

fn mark_existing_files_as_conflicts(
    actions: &mut [ManagedFileAction],
    adopted_existing_pyproject: bool,
) {
    for action in actions {
        if adopted_existing_pyproject
            && matches!(action, ManagedFileAction::Update(path) if path == Path::new("pyproject.toml"))
        {
            continue;
        }
        if matches!(
            action,
            ManagedFileAction::Update(_) | ManagedFileAction::Relink(_)
        ) {
            let path = action.path().to_path_buf();
            *action = ManagedFileAction::Conflict {
                path,
                reason: ManagedFileConflict::ExistingFile,
            };
        }
    }
}

fn print_actions(actions: &[ManagedFileAction]) {
    for action in actions {
        if let Some(reason) = action.reason() {
            ui::action(
                action.label(),
                format!("{} ({reason})", action.path().display()),
            );
        } else {
            ui::action(action.label(), action.path().display());
        }
    }
    ui::info("changes", count_changes(actions));
    ui::info("conflicts", count_conflicts(actions));
}

fn print_next_steps(args: &InitArgs, blueprint: BlueprintName, conflicts: usize, dry_run: bool) {
    let next_steps = next_steps_for_report(args, blueprint, dry_run, conflicts);
    if next_steps.is_empty() {
        return;
    }

    ui::section("Next steps");
    for step in next_steps {
        ui::next_step(&step);
    }
}

fn print_json_report(
    args: &InitArgs,
    path: &Path,
    blueprint: BlueprintName,
    dry_run: bool,
    force: bool,
    project: &ProjectRender,
    actions: &[ManagedFileAction],
) -> Result<()> {
    let conflicts = count_conflicts(actions);
    let report = InitReport {
        path: path.display().to_string(),
        project_name: project.project_name.as_str(),
        blueprint: blueprint.as_str(),
        blueprint_version: blueprint.version(),
        status_code: init_status_code(dry_run, conflicts),
        dry_run,
        force,
        managed_sync: "forge sync --path .",
        infrastructure: new::managed_infrastructure_summary(&project.files),
        required_tools: new::required_tools_summary_for_options(blueprint, &project.options),
        options: &project.options,
        next_steps: next_steps_for_report(args, blueprint, dry_run, conflicts),
        changes: count_changes(actions),
        conflicts,
        actions: actions
            .iter()
            .map(|action| InitAction {
                action: action.label(),
                path: action.path().display().to_string(),
                reason_code: action.reason_code(),
                reason: action.reason(),
                changes_filesystem: action.changes_filesystem(),
            })
            .collect(),
    };

    ui::json(report)
}

fn next_steps_for_report(
    args: &InitArgs,
    blueprint: BlueprintName,
    dry_run: bool,
    conflicts: usize,
) -> Vec<String> {
    if conflicts > 0 {
        vec![force_init_command(args, blueprint)]
    } else if dry_run {
        vec![init_command(args, blueprint, args.force)]
    } else {
        vec![
            format!("cd {}", ui::shell_arg(args.path.display().to_string())),
            "uv sync --all-groups".to_string(),
            "just verify".to_string(),
        ]
    }
}

fn force_init_command(args: &InitArgs, blueprint: BlueprintName) -> String {
    init_command(args, blueprint, true)
}

fn init_setup_review_context(
    args: &InitArgs,
    blueprint: BlueprintName,
    options: &[new::SelectedOption],
    files: &GeneratedFiles,
    changes: usize,
    conflicts: usize,
    apply_command: &str,
) -> Vec<new::SetupReviewItem> {
    vec![
        new::SetupReviewItem::new("force", args.force.to_string()),
        new::SetupReviewItem::new("changes", changes.to_string()),
        new::SetupReviewItem::new("conflicts", conflicts.to_string()),
        new::SetupReviewItem::new("infrastructure", new::managed_infrastructure_summary(files)),
        new::SetupReviewItem::new(
            "required tools",
            new::required_tools_summary_for_options(blueprint, options),
        ),
        new::SetupReviewItem::new("apply", apply_command),
    ]
}

fn preview_init_command(
    args: &InitArgs,
    blueprint: BlueprintName,
    project: &ProjectRender,
    force: bool,
    stdin_is_terminal: bool,
) -> String {
    if args.yes || !stdin_is_terminal {
        return init_command(args, blueprint, force);
    }

    let render_args = new_args_from_init_args(args);
    let Some(resolved_new_args) =
        new::resolved_new_args_from_rendered_pyproject(&render_args, project)
    else {
        return init_command(args, blueprint, force);
    };

    let mut resolved = args.clone();
    resolved.project_name = resolved_new_args.project_name;
    resolved.package_name = resolved_new_args.package_name;
    resolved.description = resolved_new_args.description;
    resolved.author_name = resolved_new_args.author_name;
    resolved.author_email = resolved_new_args.author_email;
    resolved.license = resolved_new_args.license;
    resolved.python_min = resolved_new_args.python_min;
    resolved.gitignore_profile = resolved_new_args.gitignore_profile;
    resolved.docs = resolved_new_args.docs;
    resolved.codecov = resolved_new_args.codecov;
    resolved.pypi_publish = resolved_new_args.pypi_publish;
    resolved.prettier = resolved_new_args.prettier;
    resolved.editorconfig = resolved_new_args.editorconfig;
    resolved.markdownlint = resolved_new_args.markdownlint;

    init_command(&resolved, blueprint, force)
}

fn init_command(args: &InitArgs, blueprint: BlueprintName, force: bool) -> String {
    let mut parts = vec![
        "forge".to_string(),
        "init".to_string(),
        "--path".to_string(),
        ui::shell_arg(args.path.display().to_string()),
        "--blueprint".to_string(),
        blueprint.as_str().to_string(),
    ];

    new::push_option(&mut parts, "--project-name", args.project_name.as_deref());
    new::push_option(&mut parts, "--package-name", args.package_name.as_deref());
    new::push_option(&mut parts, "--description", args.description.as_deref());
    new::push_option(&mut parts, "--author-name", args.author_name.as_deref());
    new::push_option(&mut parts, "--author-email", args.author_email.as_deref());
    new::push_option(&mut parts, "--license", args.license.as_deref());
    new::push_option(&mut parts, "--python-min", args.python_min.as_deref());
    new::push_option(
        &mut parts,
        "--gitignore-profile",
        args.gitignore_profile.as_deref(),
    );

    new::push_managed_option_flags(
        &mut parts,
        new::ManagedOptionFlags {
            docs: args.docs,
            codecov: args.codecov,
            pypi_publish: args.pypi_publish,
            prettier: args.prettier,
            editorconfig: args.editorconfig,
            markdownlint: args.markdownlint,
        },
    );

    if force {
        parts.push("--force".to_string());
    }
    if args.yes {
        parts.push("--yes".to_string());
    }

    parts.join(" ")
}

fn init_status_code(dry_run: bool, conflicts: usize) -> &'static str {
    if conflicts > 0 {
        "conflicts"
    } else if dry_run {
        "dry_run"
    } else {
        "initialized"
    }
}

#[derive(Serialize)]
struct InitReport<'a> {
    path: String,
    project_name: &'a str,
    blueprint: &'a str,
    blueprint_version: &'a str,
    status_code: &'static str,
    dry_run: bool,
    force: bool,
    managed_sync: &'a str,
    infrastructure: String,
    required_tools: String,
    options: &'a [new::SelectedOption],
    next_steps: Vec<String>,
    changes: usize,
    conflicts: usize,
    actions: Vec<InitAction<'a>>,
}

#[derive(Serialize)]
struct InitAction<'a> {
    action: &'a str,
    path: String,
    reason_code: Option<&'a str>,
    reason: Option<&'a str>,
    changes_filesystem: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn mark_existing_files_as_conflicts_preserves_creates_and_keeps() {
        let mut actions = vec![
            ManagedFileAction::Create(PathBuf::from("pyproject.toml")),
            ManagedFileAction::Keep(PathBuf::from("justfile")),
            ManagedFileAction::Update(PathBuf::from("README.md")),
            ManagedFileAction::Relink(PathBuf::from("CLAUDE.md")),
        ];

        mark_existing_files_as_conflicts(&mut actions, false);

        assert_eq!(
            actions,
            vec![
                ManagedFileAction::Create(PathBuf::from("pyproject.toml")),
                ManagedFileAction::Keep(PathBuf::from("justfile")),
                ManagedFileAction::Conflict {
                    path: PathBuf::from("README.md"),
                    reason: ManagedFileConflict::ExistingFile,
                },
                ManagedFileAction::Conflict {
                    path: PathBuf::from("CLAUDE.md"),
                    reason: ManagedFileConflict::ExistingFile,
                },
            ]
        );
    }

    #[test]
    fn force_init_command_preserves_setup_flags() {
        let args = InitArgs {
            blueprint: Some(BlueprintName::PythonLibrary),
            path: PathBuf::from("/tmp/my repo"),
            project_name: Some("grid-tools".to_string()),
            package_name: Some("grid_tools".to_string()),
            description: Some("Grid toolchain".to_string()),
            author_name: Some("Ada Lovelace".to_string()),
            author_email: Some("ada@example.com".to_string()),
            license: Some("MIT".to_string()),
            python_min: Some("3.12".to_string()),
            gitignore_profile: Some("python,macos,visualstudiocode,jetbrains,node".to_string()),
            docs: false,
            codecov: Some(false),
            pypi_publish: Some(true),
            prettier: true,
            editorconfig: true,
            markdownlint: false,
            json: true,
            dry_run: true,
            diff: true,
            force: false,
            yes: true,
        };

        let command = force_init_command(&args, BlueprintName::PythonLibrary);

        assert_eq!(
            command,
            "forge init --path '/tmp/my repo' --blueprint python-library --project-name grid-tools --package-name grid_tools --description 'Grid toolchain' --author-name 'Ada Lovelace' --author-email 'ada@example.com' --license MIT --python-min 3.12 --gitignore-profile 'python,macos,visualstudiocode,jetbrains,node' --docs=false --codecov=false --pypi-publish=true --prettier --editorconfig --force --yes"
        );
        assert!(!command.contains("--json"));
        assert!(!command.contains("--dry-run"));
        assert!(!command.contains("--diff"));
    }

    #[test]
    fn init_status_code_covers_json_outcomes() {
        assert_eq!(init_status_code(false, 2), "conflicts");
        assert_eq!(init_status_code(true, 0), "dry_run");
        assert_eq!(init_status_code(false, 0), "initialized");
    }

    #[test]
    fn init_command_drops_preview_flags_and_preserves_force_state() {
        let args = InitArgs {
            blueprint: Some(BlueprintName::AnyProject),
            path: PathBuf::from("/tmp/repo"),
            project_name: Some("repo-infra".to_string()),
            package_name: None,
            description: Some("Shared infra".to_string()),
            author_name: None,
            author_email: None,
            license: None,
            python_min: None,
            gitignore_profile: None,
            docs: true,
            codecov: None,
            pypi_publish: None,
            prettier: false,
            editorconfig: false,
            markdownlint: false,
            json: true,
            dry_run: true,
            diff: true,
            force: false,
            yes: true,
        };

        let command = init_command(&args, BlueprintName::AnyProject, false);

        assert_eq!(
            command,
            "forge init --path /tmp/repo --blueprint any-project --project-name repo-infra --description 'Shared infra' --yes"
        );
        assert!(!command.contains("--force"));
        assert!(!command.contains("--json"));
        assert!(!command.contains("--dry-run"));
        assert!(!command.contains("--diff"));
    }

    #[test]
    fn setup_review_context_includes_apply_command_and_counts() {
        let args = InitArgs {
            blueprint: Some(BlueprintName::AnyProject),
            path: PathBuf::from("/tmp/repo"),
            project_name: Some("repo-infra".to_string()),
            package_name: None,
            description: Some("Shared infra".to_string()),
            author_name: None,
            author_email: None,
            license: None,
            python_min: None,
            gitignore_profile: None,
            docs: true,
            codecov: None,
            pypi_publish: None,
            prettier: false,
            editorconfig: false,
            markdownlint: false,
            json: false,
            dry_run: false,
            diff: false,
            force: true,
            yes: true,
        };

        let files = GeneratedFiles::from([
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
        ]);

        let options = vec![
            new::SelectedOption {
                name: "docs",
                enabled: true,
            },
            new::SelectedOption {
                name: "markdownlint",
                enabled: true,
            },
        ];

        let context = init_setup_review_context(
            &args,
            BlueprintName::AnyProject,
            &options,
            &files,
            12,
            0,
            "forge init --path /tmp/repo --blueprint any-project --project-name repo-infra --description 'Shared infra' --force --yes",
        );

        assert!(
            context
                .iter()
                .any(|item| item.label == "force" && item.value == "true")
        );
        assert!(
            context
                .iter()
                .any(|item| item.label == "changes" && item.value == "12")
        );
        assert!(
            context
                .iter()
                .any(|item| item.label == "conflicts" && item.value == "0")
        );
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
        assert_eq!(required_tools.value, "uv, just, npx");
        let apply = context
            .iter()
            .find(|item| item.label == "apply")
            .expect("apply command should be present");
        assert_eq!(
            apply.value,
            "forge init --path /tmp/repo --blueprint any-project --project-name repo-infra --description 'Shared infra' --force --yes"
        );
    }

    #[test]
    fn preview_init_command_prefers_rendered_metadata_in_interactive_mode() {
        let args = InitArgs {
            blueprint: Some(BlueprintName::PythonLibrary),
            path: PathBuf::from("/tmp/repo"),
            project_name: None,
            package_name: None,
            description: None,
            author_name: None,
            author_email: None,
            license: None,
            python_min: None,
            gitignore_profile: None,
            docs: true,
            codecov: None,
            pypi_publish: None,
            prettier: false,
            editorconfig: false,
            markdownlint: false,
            json: false,
            dry_run: true,
            diff: false,
            force: false,
            yes: false,
        };
        let project = new::ProjectRender {
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

        let command =
            preview_init_command(&args, BlueprintName::PythonLibrary, &project, false, true);

        assert!(command.contains("--project-name grid-tools"));
        assert!(command.contains("--package-name grid_tools"));
        assert!(command.contains("--description 'Grid toolchain'"));
        assert!(command.contains("--author-name 'Ada Lovelace'"));
        assert!(command.contains("--author-email 'ada@example.com'"));
        assert!(command.contains("--license MIT"));
        assert!(command.contains("--python-min 3.12"));
        assert!(command.contains("--docs=false"));
        assert!(command.contains("--codecov=false"));
        assert!(command.contains("--pypi-publish=true"));
        assert!(command.contains("--prettier"));
        assert!(command.contains("--editorconfig"));
        assert!(command.contains("--markdownlint"));
    }
}
