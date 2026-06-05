use std::fs;
use std::io::IsTerminal;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::blueprint::files::{
    GeneratedFile, GeneratedFiles, ManagedFileAction, count_changes, count_conflicts,
    plan_generated_files, write_generated_files,
};
use crate::blueprint::{
    BlueprintName, detect_blueprint_metadata_from_pyproject, forge_metadata_is_python_library,
    minimal_external_pyproject_metadata, python_library, rust_library,
};
use crate::cli::{InitArgs, NewArgs};
use crate::commands::dependency_groups::sync_dependency_groups;
use crate::commands::diff;
use crate::commands::new::{self, ProjectRender, RenderScope};
use crate::commands::pyproject_sections::{sync_build_system, sync_pytest_sections};
use crate::errors::{ErrorCode, coded_error};
use crate::ui;

const PYPROJECT_TOML: &str = "pyproject.toml";

pub fn run(mut args: InitArgs) -> Result<()> {
    if let Some(path) = args.path_flag.take() {
        args.path = path;
    }
    if should_create_project(&args.path)? {
        return new::run(new_args_from_init_args(&args));
    }
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
    let adopted_existing_pyproject = adopt_existing_pyproject(&args.path, &mut project.files)?;
    let infrastructure = new::managed_infrastructure_summary(&project.files);
    let apply_command = preview_init_command(&args, blueprint, &project, stdin_is_terminal);
    let actions = plan_generated_files(&args.path, &project.files);
    let overwrites = count_overwrites(&actions, adopted_existing_pyproject);
    let changes = count_changes(&actions);
    let conflicts = count_conflicts(&actions);

    if args.json {
        print_json_report(
            &args,
            &args.path,
            blueprint,
            args.dry_run,
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
        print_detected_package_name(&args);
        ui::info("options", new::format_selected_options(&project.options));
        ui::info(
            "required tools",
            new::required_tools_summary_for_options(blueprint, &project.options),
        );
        ui::info("infrastructure", &infrastructure);
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
            "managed paths cannot be updated safely; resolve conflicts and retry",
        ));
    }

    let mut overwrites_confirmed = false;
    if overwrites > 0 && !args.dry_run && !args.json && !args.yes {
        ui::section("Destination overwrites");
        ui::info("overwrites", overwrites);
        let overwrite_actions = overwrite_actions(&actions, adopted_existing_pyproject);
        diff::print_diffs(&args.path, &overwrite_actions, &project.files)?;
        if !new::confirm_yes_no(
            "Apply Forge-managed infrastructure and overwrite conflicting managed files?",
            false,
        )? {
            ui::section("Repository initialization canceled");
            ui::success("no files changed");
            return Ok(());
        }
        overwrites_confirmed = true;
    }

    if should_confirm_setup(&args, overwrites_confirmed)
        && !new::confirm_interactive_setup(
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
                    blueprint,
                    &project.options,
                    &project.files,
                    changes,
                    conflicts,
                    &apply_command,
                    overwrites,
                ),
            },
        )?
    {
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

fn should_create_project(path: &Path) -> Result<bool> {
    match fs::metadata(path) {
        Ok(metadata) if !metadata.is_dir() => Ok(false),
        Ok(_) => {
            let mut entries = fs::read_dir(path)
                .with_context(|| format!("failed to read project path {}", path.display()))?;
            Ok(entries.next().is_none())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Err(error) => {
            Err(error).with_context(|| format!("failed to read project path {}", path.display()))
        }
    }
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
    apply_existing_path_fallbacks(args, blueprint);
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
    if infer_python_min {
        fill_if_missing(
            &mut args.package_name,
            python_package_name(&parsed, &args.path),
        );
    }
    fill_if_missing(
        &mut args.description,
        project_string(project, "description"),
    );
    fill_if_missing(&mut args.license, project_license(project));
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
    fill_if_missing(&mut args.license, project_string(package, "license"));
    if args.package_name.is_none()
        && let Some(project_name) = args.project_name.as_deref()
    {
        args.package_name = Some(rust_library::default_crate_name(project_name));
    }

    Ok(())
}

fn apply_existing_path_fallbacks(args: &mut InitArgs, blueprint: BlueprintName) {
    let fallback_name = args
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("project")
        .to_string();
    fill_if_missing(&mut args.project_name, Some(fallback_name.clone()));
    fill_if_missing(
        &mut args.description,
        Some(format!("Existing {fallback_name} project")),
    );

    match blueprint {
        BlueprintName::AnyProject => {}
        BlueprintName::PythonLibrary => {
            fill_if_missing(&mut args.license, existing_license(&args.path));
            if args.package_name.is_none() {
                args.package_name = Some(python_library::default_package_name(&fallback_name));
            }
            fill_if_missing(&mut args.python_min, Some("3.11".to_string()));
        }
        BlueprintName::RustLibrary => {
            fill_if_missing(&mut args.license, existing_license(&args.path));
            if args.package_name.is_none() {
                args.package_name = Some(rust_library::default_crate_name(&fallback_name));
            }
        }
    }
}

fn python_package_name(parsed: &toml::Value, path: &Path) -> Option<String> {
    uv_build_backend_module_name(parsed).or_else(|| package_name_from_src_layout(path))
}

fn uv_build_backend_module_name(parsed: &toml::Value) -> Option<String> {
    parsed
        .get("tool")
        .and_then(|tool| tool.get("uv"))
        .and_then(|uv| uv.get("build-backend"))
        .and_then(|build_backend| project_string(build_backend.as_table()?, "module-name"))
        .filter(|name| python_library::is_valid_package_name(name))
}

fn package_name_from_src_layout(path: &Path) -> Option<String> {
    let src_path = path.join("src");
    let entries = fs::read_dir(src_path).ok()?;
    let packages = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().join("__init__.py").is_file())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| python_library::is_valid_package_name(name))
        .collect::<Vec<_>>();

    match packages.as_slice() {
        [package_name] => Some(package_name.clone()),
        _ => None,
    }
}

fn project_license(project: &toml::Table) -> Option<String> {
    project
        .get("license")
        .and_then(|license| {
            license.as_str().map(str::to_string).or_else(|| {
                license
                    .as_table()
                    .and_then(|license| project_string(license, "text"))
            })
        })
        .filter(|license| matches!(license.as_str(), "BSD-3-Clause" | "MIT" | "Apache-2.0"))
}

fn existing_license(path: &Path) -> Option<String> {
    for filename in ["LICENSE", "LICENSE.txt", "LICENCE", "LICENCE.txt"] {
        let Ok(content) = fs::read_to_string(path.join(filename)) else {
            continue;
        };
        if let Some(license) = detect_supported_license(&content) {
            return Some(license.to_string());
        }
        return Some("BSD-3-Clause".to_string());
    }
    None
}

fn detect_supported_license(content: &str) -> Option<&'static str> {
    if content.contains("MIT License") {
        return Some("MIT");
    }
    if content.contains("Apache License") && content.contains("Version 2.0") {
        return Some("Apache-2.0");
    }
    if content.contains("Redistribution and use in source and binary forms") {
        return Some("BSD-3-Clause");
    }
    None
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
    let start = pyproject.find("[tool.forge]")?;
    let metadata = &pyproject[start..];
    let end = metadata
        .match_indices("\n[")
        .find_map(|(index, _)| {
            let header = metadata[index + 1..].lines().next()?.trim();
            let table = header.strip_prefix('[')?.strip_suffix(']')?;
            (!table.starts_with("tool.forge.")).then_some(index)
        })
        .unwrap_or(metadata.len());
    Some(&metadata[..end])
}

fn append_forge_metadata(
    existing: &str,
    forge_metadata: &str,
    generated_pyproject: &str,
) -> Result<String> {
    let adopted = sync_dependency_groups(existing, generated_pyproject)?;
    let adopted = sync_build_system(&adopted, generated_pyproject)?;
    let mut adopted = sync_pytest_sections(&adopted, generated_pyproject)?;
    if !adopted.ends_with('\n') {
        adopted.push('\n');
    }
    if !adopted.ends_with("\n\n") {
        adopted.push('\n');
    }
    adopted.push_str(&external_pyproject_metadata(existing, forge_metadata));
    Ok(adopted)
}

fn external_pyproject_metadata(existing: &str, forge_metadata: &str) -> String {
    if existing_has_project_table(existing) && forge_metadata_is_python_library(forge_metadata) {
        return minimal_external_pyproject_metadata(forge_metadata);
    }

    forge_metadata.to_string()
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
        ignored_files: args.ignored_files.clone(),
        no_git_history: args.no_git_history,
        github: args.github,
        github_owner: args.github_owner.clone(),
        github_visibility: args.github_visibility,
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
                    "repository path does not exist: {}; create it first or use `forge init --path {}`",
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
                "repository path is not a directory: {}; choose an existing repository directory or use `forge init --path {}`",
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

fn should_confirm_setup(args: &InitArgs, overwrites_confirmed: bool) -> bool {
    !overwrites_confirmed && !args.yes && !args.json && !args.dry_run
}

fn count_overwrites(actions: &[ManagedFileAction], adopted_existing_pyproject: bool) -> usize {
    actions
        .iter()
        .filter(|action| should_review_overwrite(action, adopted_existing_pyproject))
        .count()
}

fn overwrite_actions(
    actions: &[ManagedFileAction],
    adopted_existing_pyproject: bool,
) -> Vec<ManagedFileAction> {
    actions
        .iter()
        .filter(|action| should_review_overwrite(action, adopted_existing_pyproject))
        .cloned()
        .collect()
}

fn should_review_overwrite(action: &ManagedFileAction, adopted_existing_pyproject: bool) -> bool {
    if adopted_existing_pyproject && is_pyproject_update(action) {
        return false;
    }
    matches!(
        action,
        ManagedFileAction::Update(_) | ManagedFileAction::Relink(_)
    )
}

fn is_pyproject_update(action: &ManagedFileAction) -> bool {
    matches!(action, ManagedFileAction::Update(path) if path == Path::new(PYPROJECT_TOML))
}

fn print_detected_package_name(args: &InitArgs) {
    if let Some(package_name) = args.package_name.as_deref() {
        ui::info("detected package", package_name);
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
        Vec::new()
    } else if dry_run {
        vec![init_command(args, blueprint)]
    } else {
        vec![
            format!("cd {}", ui::shell_arg(args.path.display().to_string())),
            "uv sync --all-groups".to_string(),
            "just verify".to_string(),
        ]
    }
}

fn init_setup_review_context(
    blueprint: BlueprintName,
    options: &[new::SelectedOption],
    files: &GeneratedFiles,
    changes: usize,
    conflicts: usize,
    apply_command: &str,
    overwrites: usize,
) -> Vec<new::SetupReviewItem> {
    vec![
        new::SetupReviewItem::new("changes", changes.to_string()),
        new::SetupReviewItem::new("overwrites", overwrites.to_string()),
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
    stdin_is_terminal: bool,
) -> String {
    if args.yes || !stdin_is_terminal {
        return init_command(args, blueprint);
    }

    let render_args = new_args_from_init_args(args);
    let Some(resolved_new_args) =
        new::resolved_new_args_from_rendered_pyproject(&render_args, project)
    else {
        return init_command(args, blueprint);
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

    init_command(&resolved, blueprint)
}

fn init_command(args: &InitArgs, blueprint: BlueprintName) -> String {
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
    use tempfile::TempDir;

    #[test]
    fn overwrite_count_preserves_creates_keeps_and_adopted_pyproject() {
        let actions = vec![
            ManagedFileAction::Create(PathBuf::from("pyproject.toml")),
            ManagedFileAction::Keep(PathBuf::from("justfile")),
            ManagedFileAction::Update(PathBuf::from("README.md")),
            ManagedFileAction::Relink(PathBuf::from("CLAUDE.md")),
            ManagedFileAction::Update(PathBuf::from("pyproject.toml")),
        ];

        assert_eq!(count_overwrites(&actions, false), 3);
        assert_eq!(count_overwrites(&actions, true), 2);
    }

    #[test]
    fn overwrite_confirmation_skips_second_setup_confirmation() {
        let mut args = InitArgs {
            blueprint: Some(BlueprintName::AnyProject),
            path: PathBuf::from("/tmp/repo"),
            path_flag: None,
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
            ignored_files: Vec::new(),
            no_git_history: false,
            github: false,
            github_owner: None,
            github_visibility: None,
            json: false,
            dry_run: false,
            diff: false,
            yes: false,
        };

        assert!(should_confirm_setup(&args, false));
        assert!(!should_confirm_setup(&args, true));
        args.yes = true;
        assert!(!should_confirm_setup(&args, false));
    }

    #[test]
    fn init_command_drops_preview_flags_and_uses_yes_for_noninteractive_apply() {
        let args = InitArgs {
            blueprint: Some(BlueprintName::AnyProject),
            path: PathBuf::from("/tmp/repo"),
            path_flag: None,
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
            ignored_files: Vec::new(),
            no_git_history: false,
            github: false,
            github_owner: None,
            github_visibility: None,
            json: true,
            dry_run: true,
            diff: true,
            yes: true,
        };

        let command = init_command(&args, BlueprintName::AnyProject);

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
    fn init_status_code_covers_json_outcomes() {
        assert_eq!(init_status_code(false, 2), "conflicts");
        assert_eq!(init_status_code(true, 0), "dry_run");
        assert_eq!(init_status_code(false, 0), "initialized");
    }

    #[test]
    fn setup_review_context_includes_apply_command_and_counts() {
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
            BlueprintName::AnyProject,
            &options,
            &files,
            12,
            0,
            "forge init --path /tmp/repo --blueprint any-project --project-name repo-infra --description 'Shared infra' --yes",
            3,
        );

        assert!(
            context
                .iter()
                .any(|item| item.label == "overwrites" && item.value == "3")
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
            "forge init --path /tmp/repo --blueprint any-project --project-name repo-infra --description 'Shared infra' --yes"
        );
    }

    #[test]
    fn existing_path_defaults_prevent_interactive_metadata_prompts_without_pyproject() {
        let temp = TempDir::new().expect("temp dir should create");
        let project_path = temp.path().join("grid-tools");
        fs::create_dir(&project_path).expect("project dir should create");
        fs::write(project_path.join("LICENSE"), "MIT License\n")
            .expect("license file should write");
        let mut args = InitArgs {
            blueprint: Some(BlueprintName::PythonLibrary),
            path: project_path,
            path_flag: None,
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
            ignored_files: Vec::new(),
            no_git_history: false,
            github: false,
            github_owner: None,
            github_visibility: None,
            json: false,
            dry_run: false,
            diff: false,
            yes: false,
        };

        apply_existing_project_defaults(&mut args, BlueprintName::PythonLibrary)
            .expect("existing path defaults should apply");

        assert_eq!(args.project_name.as_deref(), Some("grid-tools"));
        assert_eq!(args.package_name.as_deref(), Some("grid_tools"));
        assert_eq!(
            args.description.as_deref(),
            Some("Existing grid-tools project")
        );
        assert_eq!(args.python_min.as_deref(), Some("3.11"));
        assert_eq!(args.license.as_deref(), Some("MIT"));
    }

    #[test]
    fn preview_init_command_prefers_rendered_metadata_in_interactive_mode() {
        let args = InitArgs {
            blueprint: Some(BlueprintName::PythonLibrary),
            path: PathBuf::from("/tmp/repo"),
            path_flag: None,
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
            ignored_files: Vec::new(),
            no_git_history: false,
            github: false,
            github_owner: None,
            github_visibility: None,
            json: false,
            dry_run: true,
            diff: false,
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

        let command = preview_init_command(&args, BlueprintName::PythonLibrary, &project, true);

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
