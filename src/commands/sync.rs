use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use dialoguer::Confirm;
use serde::Serialize;
use toml::Value;

use crate::blueprint::files::{
    GeneratedFile, GeneratedFiles, ManagedFileAction, ManagedFileConflict, count_changes,
    count_conflicts, managed_file_path, plan_generated_files, remove_managed_file_if_exists,
    write_generated_files,
};
use crate::blueprint::{
    BlueprintMetadata, BlueprintName, ManagedOption, ManagedOptionValues,
    detect_blueprint_metadata_from_pyproject, forge_metadata_is_python_library,
    managed_option_enabled, minimal_external_pyproject_metadata,
    validate_managed_overrides_from_metadata,
};
use crate::cli::SyncArgs;
use crate::commands::dependency_groups::sync_dependency_groups;
use crate::commands::diff;
use crate::commands::new::managed_infrastructure_summary;
use crate::commands::pyproject_sections::{sync_build_system, sync_pytest_sections};
use crate::errors::{ErrorCode, coded_error};
use crate::ui;

pub fn run(args: SyncArgs) -> Result<()> {
    let stdin_is_terminal = std::io::stdin().is_terminal();
    crate::commands::validate_diff_mode(args.diff, args.dry_run, args.check)?;
    let root = args
        .path
        .canonicalize()
        .unwrap_or_else(|_| args.path.clone());
    ensure_update_path_is_directory(&root)?;
    let pyproject_path = root.join("pyproject.toml");
    let pyproject = read_pyproject_for_update(&root, &pyproject_path)?;
    ensure_forge_metadata_for_update(&root, &pyproject)?;
    let metadata = detect_blueprint_metadata_from_pyproject(&pyproject).with_context(|| {
        format!(
            "failed to validate Forge metadata at {}; ensure [tool.forge] includes blueprint_version and valid [tool.forge.overrides] keys",
            root.display()
        )
    })?;
    let blueprint = metadata.name;
    let blueprint_version = metadata
        .version
        .as_deref()
        .expect("blueprint version metadata is required");

    let pyproject = apply_option_overrides(&pyproject, blueprint, &args.set)?;
    let options = selected_options_from_pyproject(&pyproject, blueprint).with_context(|| {
        format!(
            "failed to validate Forge metadata at {}; ensure [tool.forge] includes blueprint_version and valid [tool.forge.overrides] keys",
            root.display()
        )
    })?;
    let mut managed_files = blueprint
        .render_managed_files_from_pyproject(&pyproject)
        .map_err(|error| {
            coded_error(
                ErrorCode::Env,
                format!(
                    "failed to validate Forge metadata at {}: {error:#}",
                    root.display()
                ),
            )
        })?;
    if uses_external_pyproject(&pyproject)? {
        let generated_pyproject = managed_files
            .get(Path::new("pyproject.toml"))
            .and_then(GeneratedFile::as_text)
            .context("generated managed files are missing pyproject.toml")?;
        managed_files.insert(
            PathBuf::from("pyproject.toml"),
            GeneratedFile::text(sync_external_pyproject(&pyproject, generated_pyproject)?),
        );
    } else {
        preserve_pyproject_format_if_equivalent(&pyproject, &mut managed_files)?;
    }
    let infrastructure = managed_infrastructure_summary(&managed_files);

    let mut actions = plan_generated_files(&root, &managed_files);
    actions.extend(cleanup_actions_for_blueprint(blueprint, &root, &pyproject)?);
    let changes = count_changes(&actions);
    let conflicts = count_conflicts(&actions);
    let read_only = args.dry_run || args.check;

    if !args.json {
        ui::section(if read_only {
            "Managed changes preview"
        } else {
            "Managed changes"
        });
        print_actions(&actions);
        if args.diff {
            diff::print_diffs(&root, &actions, &managed_files)?;
        }
    }

    if conflicts > 0 {
        if args.json {
            print_json_report(SyncJsonReportInput {
                root: &root,
                metadata: &metadata,
                blueprint_version,
                dry_run: args.dry_run,
                check: args.check,
                infrastructure: &infrastructure,
                option_overrides: &args.set,
                options: &options,
                actions: &actions,
            })?;
        } else {
            print_sync_context(
                &root,
                blueprint,
                blueprint_version,
                &options,
                &infrastructure,
            );
            print_next_steps(&root, args.dry_run, args.check, &args.set, &actions);
        }
        return Err(coded_error(
            ErrorCode::Conflict,
            "managed infrastructure has conflicts; resolve conflicted paths and rerun sync",
        ));
    }

    if should_confirm_sync(
        args.yes,
        args.json,
        args.dry_run,
        args.check,
        stdin_is_terminal,
        changes,
    ) {
        print_sync_review(
            &root,
            blueprint,
            blueprint_version,
            &options,
            &infrastructure,
            changes,
            sync_command(&root, &args.set),
        );
        if !confirm_sync_apply()? {
            ui::section("Sync canceled");
            ui::success("no files changed");
            return Ok(());
        }
    } else {
        ensure_noninteractive_apply_allowed(NonInteractiveApplyGuardInput {
            assume_yes: args.yes,
            json: args.json,
            dry_run: args.dry_run,
            check: args.check,
            stdin_is_terminal,
            changes,
            root: &root,
            option_overrides: &args.set,
        })?;
    }

    if !read_only {
        write_generated_files(&root, managed_files)?;
        clean_optional_files_for_blueprint(blueprint, &root, &pyproject)?;
    }

    if args.json {
        print_json_report(SyncJsonReportInput {
            root: &root,
            metadata: &metadata,
            blueprint_version,
            dry_run: args.dry_run,
            check: args.check,
            infrastructure: &infrastructure,
            option_overrides: &args.set,
            options: &options,
            actions: &actions,
        })?;
        if args.check && changes > 0 {
            return Err(coded_error(
                ErrorCode::Conflict,
                "managed infrastructure is out of date",
            ));
        }
        return Ok(());
    }

    ui::section(sync_result_section_title(read_only, changes));
    if args.check && changes > 0 {
        print_sync_context(
            &root,
            blueprint,
            blueprint_version,
            &options,
            &infrastructure,
        );
        print_next_steps(&root, args.dry_run, args.check, &args.set, &actions);
        return Err(coded_error(
            ErrorCode::Conflict,
            format!(
                "managed infrastructure is out of date; run `forge sync --path {}`",
                root.display()
            ),
        ));
    } else if args.check {
        ui::success("managed infrastructure is current");
    } else if args.dry_run {
        ui::success("dry run complete; no files changed");
    } else if changes == 0 {
        ui::success("managed infrastructure is already current");
    } else {
        ui::success("managed infrastructure refreshed");
    }
    print_sync_context(
        &root,
        blueprint,
        blueprint_version,
        &options,
        &infrastructure,
    );
    print_next_steps(&root, args.dry_run, args.check, &args.set, &actions);
    Ok(())
}

fn ensure_update_path_is_directory(root: &Path) -> Result<()> {
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

fn read_pyproject_for_update(root: &Path, pyproject_path: &Path) -> Result<String> {
    match fs::read_to_string(pyproject_path) {
        Ok(pyproject) => Ok(pyproject),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(coded_error(
            ErrorCode::Env,
            format!(
                "missing Forge metadata at {}; use `forge init --path {}` for an existing repository or `forge init --path {}` to create a new project",
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

fn ensure_forge_metadata_for_update(root: &Path, pyproject: &str) -> Result<()> {
    let Ok(parsed) = toml::from_str::<Value>(pyproject) else {
        return Ok(());
    };

    let has_forge_metadata = parsed
        .get("tool")
        .and_then(Value::as_table)
        .and_then(|tool| tool.get("forge"))
        .is_some();
    if !has_forge_metadata {
        return Err(coded_error(
            ErrorCode::Env,
            format!(
                "missing [tool.forge] metadata; use `forge init --path {}` to adopt this repository before running sync",
                ui::shell_arg(root.display().to_string())
            ),
        ));
    }

    Ok(())
}

fn should_confirm_sync(
    assume_yes: bool,
    json: bool,
    dry_run: bool,
    check: bool,
    stdin_is_terminal: bool,
    changes: usize,
) -> bool {
    !assume_yes && !json && !dry_run && !check && stdin_is_terminal && changes > 0
}

struct NonInteractiveApplyGuardInput<'a> {
    assume_yes: bool,
    json: bool,
    dry_run: bool,
    check: bool,
    stdin_is_terminal: bool,
    changes: usize,
    root: &'a Path,
    option_overrides: &'a [String],
}

fn ensure_noninteractive_apply_allowed(input: NonInteractiveApplyGuardInput<'_>) -> Result<()> {
    if !input.assume_yes
        && !input.json
        && !input.dry_run
        && !input.check
        && !input.stdin_is_terminal
        && input.changes > 0
    {
        let apply_command = sync_command(input.root, input.option_overrides);
        return Err(coded_error(
            ErrorCode::Input,
            format!(
                "interactive confirmation requires a terminal; rerun with `{apply_command}` or pass --json, --dry-run, or --check"
            ),
        ));
    }

    Ok(())
}

fn confirm_sync_apply() -> Result<bool> {
    Ok(Confirm::new()
        .with_prompt("Apply managed infrastructure changes?")
        .default(true)
        .interact()?)
}

fn print_sync_review(
    root: &Path,
    blueprint: BlueprintName,
    blueprint_version: &str,
    options: &[SelectedOption],
    infrastructure: &str,
    changes: usize,
    apply_command: String,
) {
    let summary = sync_review_summary(
        root,
        blueprint,
        blueprint_version,
        options,
        infrastructure,
        changes,
        apply_command,
    );

    ui::section("Sync review");
    ui::info("path", &summary.path);
    ui::info("blueprint", summary.blueprint);
    ui::info("blueprint version", &summary.blueprint_version);
    ui::info("options", &summary.options);
    ui::info("required tools", &summary.required_tools);
    ui::info("infrastructure", summary.infrastructure);
    ui::info("changes", summary.changes);
    ui::info("apply", &summary.apply);
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SyncReviewSummary {
    path: String,
    blueprint: &'static str,
    blueprint_version: String,
    options: String,
    required_tools: String,
    infrastructure: String,
    changes: usize,
    apply: String,
}

fn sync_review_summary(
    root: &Path,
    blueprint: BlueprintName,
    blueprint_version: &str,
    options: &[SelectedOption],
    infrastructure: &str,
    changes: usize,
    apply_command: String,
) -> SyncReviewSummary {
    SyncReviewSummary {
        path: root.display().to_string(),
        blueprint: blueprint.as_str(),
        blueprint_version: blueprint_version.to_string(),
        options: format_selected_options(options),
        required_tools: required_tools_summary_for_options(blueprint, options),
        infrastructure: infrastructure.to_string(),
        changes,
        apply: apply_command,
    }
}

fn apply_option_overrides(
    pyproject: &str,
    blueprint: BlueprintName,
    overrides: &[String],
) -> Result<String> {
    if overrides.is_empty() {
        return Ok(pyproject.to_string());
    }

    let parsed: Value = toml::from_str(pyproject).context("failed to parse pyproject.toml")?;
    let options = forge_option_values(&parsed, blueprint)?;
    let mut seen_options = BTreeSet::new();
    let mut parsed_overrides = Vec::new();

    for override_value in overrides {
        let (key, value) = parse_option_override(override_value)?;
        let option = ManagedOption::parse(key)?;
        if !seen_options.insert(option.as_str()) {
            return Err(coded_error(
                ErrorCode::Input,
                format!("option '{}' was set more than once", option.as_str()),
            ));
        }
        if !blueprint.supports_option(option) {
            return Err(coded_error(
                ErrorCode::Input,
                format!(
                    "option '{}' is not supported by {}",
                    option.as_str(),
                    blueprint.as_str()
                ),
            ));
        }
        managed_option_enabled(&options, option)?;
        parsed_overrides.push((option, value));
    }

    apply_option_overrides_to_text(pyproject, &parsed_overrides)
}

fn uses_external_pyproject(pyproject: &str) -> Result<bool> {
    let parsed: Value = toml::from_str(pyproject).context("failed to parse pyproject.toml")?;
    Ok(parsed
        .get("tool")
        .and_then(Value::as_table)
        .and_then(|tool| tool.get("forge"))
        .is_some())
}

fn sync_external_pyproject(pyproject: &str, generated_pyproject: &str) -> Result<String> {
    let forge_metadata = forge_metadata_block(generated_pyproject)
        .context("generated pyproject.toml is missing [tool.forge]")?;
    let synced = sync_dependency_groups(pyproject, generated_pyproject)?;
    let synced = sync_build_system(&synced, generated_pyproject)?;
    let synced = sync_pytest_sections(&synced, generated_pyproject)?;
    let synced = sync_forge_metadata(&synced, &external_pyproject_metadata(forge_metadata));
    toml::from_str::<Value>(&synced).context("failed to parse synced external pyproject.toml")?;
    Ok(synced)
}

fn sync_forge_metadata(pyproject: &str, forge_metadata: &str) -> String {
    let mut lines: Vec<String> = pyproject
        .split_inclusive('\n')
        .map(str::to_string)
        .collect();
    let replacement = forge_metadata.split_inclusive('\n').map(str::to_string);
    if let Some((start, end)) = forge_section_range(pyproject) {
        lines.splice(start..end, replacement);
        return lines.concat();
    }

    let mut output = pyproject.to_string();
    if !output.ends_with('\n') {
        output.push('\n');
    }
    output.push('\n');
    output.push_str(forge_metadata);
    output
}

fn forge_section_range(content: &str) -> Option<(usize, usize)> {
    let lines: Vec<&str> = content.split_inclusive('\n').collect();
    let start = lines
        .iter()
        .position(|line| table_header_name(line) == Some("tool.forge"))?;
    let end = lines[start + 1..]
        .iter()
        .position(|line| {
            table_header_name(line)
                .is_some_and(|name| name != "tool.forge" && !name.starts_with("tool.forge."))
        })
        .map_or(lines.len(), |offset| start + 1 + offset);
    Some((start, end))
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

fn external_pyproject_metadata(forge_metadata: &str) -> String {
    if forge_metadata_is_python_library(forge_metadata) {
        return minimal_external_pyproject_metadata(forge_metadata);
    }

    forge_metadata.to_string()
}

fn preserve_pyproject_format_if_equivalent(
    pyproject: &str,
    managed_files: &mut GeneratedFiles,
) -> Result<()> {
    let Some(generated_pyproject) = managed_files
        .get(Path::new("pyproject.toml"))
        .and_then(GeneratedFile::as_text)
    else {
        return Ok(());
    };

    let current_value: Value =
        toml::from_str(pyproject).context("failed to parse updated pyproject.toml")?;
    let generated_value: Value =
        toml::from_str(generated_pyproject).context("failed to parse generated pyproject.toml")?;
    if current_value == generated_value {
        managed_files.insert(
            PathBuf::from("pyproject.toml"),
            GeneratedFile::text(pyproject.to_string()),
        );
    }

    Ok(())
}

fn forge_overrides_table(value: &Value) -> Result<Option<&toml::Table>> {
    let root = value
        .as_table()
        .context("pyproject.toml root must be a table")?;
    let tool = root
        .get("tool")
        .context("missing [tool] table")?
        .as_table()
        .context("pyproject.toml [tool] must be a table")?;
    let forge = tool
        .get("forge")
        .context("missing [tool.forge] metadata")?
        .as_table()
        .context("pyproject.toml [tool.forge] must be a table")?;

    let overrides = forge.get("overrides").or_else(|| forge.get("options"));

    match overrides {
        Some(overrides) => overrides
            .as_table()
            .context("pyproject.toml [tool.forge.overrides] must be a table")
            .map(Some),
        None => Ok(None),
    }
}

fn forge_option_values(value: &Value, blueprint: BlueprintName) -> Result<ManagedOptionValues> {
    let overrides = forge_overrides_table(value)?;
    let mut values = BTreeMap::new();

    if let Some(overrides) = overrides {
        for (name, value) in overrides {
            let enabled = value
                .as_bool()
                .with_context(|| format!("tool.forge.overrides.{name} must be a boolean"))?;
            values.insert(name.clone(), enabled);
        }
    }

    validate_managed_overrides_from_metadata(blueprint, values)
}

fn apply_option_overrides_to_text(
    pyproject: &str,
    overrides: &[(ManagedOption, bool)],
) -> Result<String> {
    let mut lines: Vec<String> = pyproject
        .split_inclusive('\n')
        .map(str::to_string)
        .collect();
    let mut remaining_overrides = Vec::new();
    for (option, value) in overrides {
        if update_forge_ignore_option(&mut lines, option.as_str(), *value) {
            continue;
        }
        remaining_overrides.push((*option, *value));
    }
    let overrides = remaining_overrides;
    let table_name = if table_range(&lines.concat(), "tool.forge.overrides").is_some() {
        "tool.forge.overrides"
    } else {
        "tool.forge.options"
    };
    let current_pyproject = lines.concat();
    let Some((table_start, mut table_end)) = table_range(&current_pyproject, table_name) else {
        return append_option_table(&current_pyproject, &overrides);
    };

    for (option, value) in overrides {
        let option_name = option.as_str();
        let mut replaced = false;
        for line in &mut lines[table_start + 1..table_end] {
            if option_assignment_key(line) == Some(option_name) {
                *line = replace_boolean_assignment_value(line, value);
                replaced = true;
                break;
            }
        }
        if !replaced {
            lines.insert(table_end, format!("{option_name} = {value}\n"));
            table_end += 1;
        }
    }

    Ok(lines.concat())
}

fn update_forge_ignore_option(lines: &mut [String], option_name: &str, enabled: bool) -> bool {
    let Some((start, end)) = table_range(&lines.concat(), "tool.forge") else {
        return false;
    };
    for line in &mut lines[start + 1..end] {
        if option_assignment_key(line) == Some("ignore") {
            let Some((_, value)) = line.split_once('=') else {
                return false;
            };
            let mut entries = value
                .trim()
                .trim_end_matches(',')
                .trim_start_matches('[')
                .trim_end_matches(']')
                .split(',')
                .map(|entry| entry.trim().trim_matches('"').to_string())
                .filter(|entry| !entry.is_empty())
                .collect::<Vec<_>>();
            let had_option = entries.iter().any(|entry| entry == option_name);
            if enabled {
                if !had_option {
                    return false;
                }
                entries.retain(|entry| entry != option_name);
            } else if !had_option {
                entries.push(option_name.to_string());
            }
            *line = format!(
                "ignore = [{}]\n",
                entries
                    .iter()
                    .map(|entry| format!("\"{entry}\""))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            return true;
        }
    }
    false
}

fn append_option_table(pyproject: &str, overrides: &[(ManagedOption, bool)]) -> Result<String> {
    if overrides.is_empty() {
        return Ok(pyproject.to_string());
    }

    let mut output = pyproject.to_string();
    if !output.ends_with('\n') {
        output.push('\n');
    }
    output.push_str("\n[tool.forge.overrides]\n");
    for (option, value) in overrides {
        output.push_str(option.as_str());
        output.push_str(" = ");
        output.push_str(if *value { "true" } else { "false" });
        output.push('\n');
    }

    Ok(output)
}

fn table_range(content: &str, table_name: &str) -> Option<(usize, usize)> {
    let lines: Vec<&str> = content.split_inclusive('\n').collect();
    let start = lines
        .iter()
        .position(|line| table_header_name(line) == Some(table_name))?;
    let end = lines[start + 1..]
        .iter()
        .position(|line| table_header_name(line).is_some())
        .map_or(lines.len(), |offset| start + 1 + offset);

    Some((start, end))
}

fn table_header_name(line: &str) -> Option<&str> {
    let without_comment = line.split_once('#').map_or(line, |(prefix, _)| prefix);
    let trimmed = without_comment.trim();
    if !(trimmed.starts_with('[') && trimmed.ends_with(']')) {
        return None;
    }

    Some(trimmed.trim_start_matches('[').trim_end_matches(']').trim())
}

fn option_assignment_key(line: &str) -> Option<&str> {
    let without_comment = line.split_once('#').map_or(line, |(prefix, _)| prefix);
    let (key, _) = without_comment.split_once('=')?;
    let key = key.trim();
    if key.is_empty() { None } else { Some(key) }
}

fn replace_boolean_assignment_value(line: &str, value: bool) -> String {
    let Some((before_equals, after_equals)) = line.split_once('=') else {
        return line.to_string();
    };
    let value_start = after_equals
        .char_indices()
        .find_map(|(index, character)| (!character.is_whitespace()).then_some(index))
        .unwrap_or(after_equals.len());
    let suffix_start = after_equals[value_start..]
        .char_indices()
        .find_map(|(index, character)| (character == '#' || character == '\n').then_some(index))
        .map_or(after_equals.len(), |index| value_start + index);
    let value_end = value_start + after_equals[value_start..suffix_start].trim_end().len();
    let leading_whitespace = &after_equals[..value_start];
    let suffix = &after_equals[value_end..];

    format!("{before_equals}={leading_whitespace}{value}{suffix}")
}

use crate::commands::new::SelectedOption;

fn selected_options_from_pyproject(
    pyproject: &str,
    blueprint: BlueprintName,
) -> Result<Vec<SelectedOption>> {
    let parsed: Value = toml::from_str(pyproject).context("failed to parse pyproject.toml")?;
    let options = forge_option_values(&parsed, blueprint)?;

    blueprint
        .supported_options()
        .iter()
        .map(|option| {
            let enabled = managed_option_enabled(&options, *option)?;
            Ok(SelectedOption {
                name: option.as_str(),
                enabled,
            })
        })
        .collect()
}

fn format_selected_options(options: &[SelectedOption]) -> String {
    crate::commands::new::format_selected_options(options)
}

fn parse_option_override(value: &str) -> Result<(&str, bool)> {
    let Some((key, raw_value)) = value.split_once('=') else {
        return Err(coded_error(
            ErrorCode::Input,
            format!("invalid option override '{value}', expected OPTION=BOOL"),
        ));
    };
    if key.is_empty() {
        return Err(coded_error(
            ErrorCode::Input,
            format!("invalid option override '{value}', option name cannot be empty"),
        ));
    }
    if key.trim() != key || raw_value.trim() != raw_value {
        return Err(coded_error(
            ErrorCode::Input,
            format!("invalid option override '{value}', expected OPTION=BOOL"),
        ));
    }
    let enabled = match raw_value {
        "true" => true,
        "false" => false,
        _ => {
            return Err(coded_error(
                ErrorCode::Input,
                format!("invalid value for option '{key}', expected true or false"),
            ));
        }
    };

    Ok((key, enabled))
}

fn clean_optional_files_for_blueprint(
    blueprint: BlueprintName,
    root: &Path,
    pyproject: &str,
) -> Result<()> {
    blueprint.clean_optional_files_from_pyproject(root, pyproject)?;
    if blueprint == BlueprintName::PythonLibrary {
        remove_managed_file_if_exists(&root.join(".cspell.json"))?;
        remove_managed_file_if_exists(&root.join("typos.toml"))?;
    }
    Ok(())
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
    let breakdown = action_breakdown(actions);
    ui::info("create", breakdown.create);
    ui::info("update", breakdown.update);
    ui::info("relink", breakdown.relink);
    ui::info("remove", breakdown.remove);
    ui::info("keep", breakdown.keep);
    ui::info("conflict", breakdown.conflict);
    ui::info("changes", count_changes(actions));
    ui::info("conflicts", count_conflicts(actions));
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Serialize)]
struct ActionBreakdown {
    create: usize,
    update: usize,
    relink: usize,
    remove: usize,
    keep: usize,
    conflict: usize,
}

fn action_breakdown(actions: &[ManagedFileAction]) -> ActionBreakdown {
    let mut breakdown = ActionBreakdown::default();
    for action in actions {
        match action {
            ManagedFileAction::Create(_) => breakdown.create += 1,
            ManagedFileAction::Update(_) => breakdown.update += 1,
            ManagedFileAction::Relink(_) => breakdown.relink += 1,
            ManagedFileAction::Remove(_) => breakdown.remove += 1,
            ManagedFileAction::Keep(_) => breakdown.keep += 1,
            ManagedFileAction::Conflict { .. } => breakdown.conflict += 1,
        }
    }

    breakdown
}

fn print_sync_context(
    root: &Path,
    blueprint: BlueprintName,
    blueprint_version: &str,
    options: &[SelectedOption],
    infrastructure: &str,
) {
    ui::info("path", root.display());
    ui::info("blueprint", blueprint.as_str());
    ui::info("blueprint version", blueprint_version);
    ui::info("options", format_selected_options(options));
    ui::info(
        "required tools",
        required_tools_summary_for_options(blueprint, options),
    );
    ui::info("infrastructure", infrastructure);
}

fn sync_result_section_title(read_only: bool, changes: usize) -> &'static str {
    if read_only || changes == 0 {
        "Project checked"
    } else {
        "Project synced"
    }
}

fn required_tools_summary_for_options(
    blueprint: BlueprintName,
    options: &[SelectedOption],
) -> String {
    crate::commands::new::required_tools_summary_for_options(blueprint, options)
}

fn print_next_steps(
    root: &Path,
    dry_run: bool,
    check: bool,
    option_overrides: &[String],
    actions: &[ManagedFileAction],
) {
    let next_steps = next_steps_for_actions(root, dry_run, check, option_overrides, actions);
    if !next_steps.is_empty() {
        ui::section("Next steps");
        for step in next_steps {
            ui::next_step(&step);
        }
    }
}

fn next_steps_for_actions(
    root: &Path,
    dry_run: bool,
    check: bool,
    option_overrides: &[String],
    actions: &[ManagedFileAction],
) -> Vec<String> {
    if count_conflicts(actions) > 0 {
        return vec!["resolve conflicted paths and rerun sync".to_string()];
    }

    let mut next_steps = Vec::new();
    if (dry_run || check) && count_changes(actions) > 0 {
        next_steps.push(sync_command(root, option_overrides));
    }
    if actions
        .iter()
        .any(|action| action.changes_filesystem() && action.path() == Path::new("pyproject.toml"))
    {
        next_steps.push("uv lock".to_string());
    }

    next_steps
}

fn sync_command(root: &Path, option_overrides: &[String]) -> String {
    let mut command = format!(
        "forge sync --path {}",
        ui::shell_arg(root.display().to_string())
    );
    for option_override in option_overrides {
        command.push_str(" --set ");
        command.push_str(&ui::shell_arg(option_override));
    }
    command.push_str(" --yes");
    command
}

struct SyncJsonReportInput<'a> {
    root: &'a Path,
    metadata: &'a BlueprintMetadata,
    blueprint_version: &'a str,
    dry_run: bool,
    check: bool,
    infrastructure: &'a str,
    option_overrides: &'a [String],
    options: &'a [SelectedOption],
    actions: &'a [ManagedFileAction],
}

fn print_json_report(input: SyncJsonReportInput<'_>) -> Result<()> {
    let blueprint = input.metadata.name;
    let required_tools = required_tools_summary_for_options(blueprint, input.options);
    let changes = count_changes(input.actions);
    let conflicts = count_conflicts(input.actions);
    let report = SyncReport {
        path: input.root.display().to_string(),
        blueprint: blueprint.as_str(),
        blueprint_version: input.blueprint_version,
        status_code: sync_status_code(input.dry_run, input.check, changes, conflicts),
        dry_run: input.dry_run,
        check: input.check,
        infrastructure: input.infrastructure,
        required_tools,
        options: input.options,
        changes,
        conflicts,
        action_counts: action_breakdown(input.actions),
        next_steps: next_steps_for_actions(
            input.root,
            input.dry_run,
            input.check,
            input.option_overrides,
            input.actions,
        ),
        actions: input
            .actions
            .iter()
            .map(|action| SyncAction {
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

#[derive(Serialize)]
struct SyncReport<'a> {
    path: String,
    blueprint: &'a str,
    blueprint_version: &'a str,
    status_code: &'static str,
    dry_run: bool,
    check: bool,
    infrastructure: &'a str,
    required_tools: String,
    options: &'a [SelectedOption],
    changes: usize,
    conflicts: usize,
    action_counts: ActionBreakdown,
    next_steps: Vec<String>,
    actions: Vec<SyncAction<'a>>,
}

fn sync_status_code(dry_run: bool, check: bool, changes: usize, conflicts: usize) -> &'static str {
    if conflicts > 0 {
        return "conflicts";
    }
    if check && changes > 0 {
        return "out_of_date";
    }
    if dry_run {
        return "dry_run";
    }
    if changes == 0 {
        return "current";
    }
    "synced"
}

#[derive(Serialize)]
struct SyncAction<'a> {
    action: &'a str,
    path: String,
    reason_code: Option<&'a str>,
    reason: Option<&'a str>,
    changes_filesystem: bool,
}

fn cleanup_actions_for_blueprint(
    blueprint: BlueprintName,
    root: &Path,
    pyproject: &str,
) -> Result<Vec<ManagedFileAction>> {
    let mut paths = blueprint.optional_cleanup_paths_from_pyproject(pyproject)?;
    if blueprint == BlueprintName::PythonLibrary {
        paths.push(PathBuf::from(".cspell.json"));
        paths.push(PathBuf::from("typos.toml"));
    }
    paths.push(PathBuf::from(".github/workflows/forge-update.yaml"));

    let mut cleanup_actions: Vec<ManagedFileAction> = paths
        .into_iter()
        .filter_map(|relative_path| cleanup_action_for_optional_path(root, relative_path))
        .collect();
    cleanup_actions.extend(empty_parent_directory_cleanup_actions(
        root,
        &cleanup_actions,
    )?);

    Ok(cleanup_actions)
}

fn cleanup_action_for_optional_path(
    root: &Path,
    relative_path: PathBuf,
) -> Option<ManagedFileAction> {
    let full_path = match managed_file_path(root, &relative_path) {
        Ok(full_path) => full_path,
        Err(_) => {
            return Some(ManagedFileAction::Conflict {
                path: relative_path,
                reason: ManagedFileConflict::UnsafePath,
            });
        }
    };
    let Ok(metadata) = full_path.symlink_metadata() else {
        return None;
    };

    if metadata.is_dir() {
        Some(ManagedFileAction::Conflict {
            path: relative_path,
            reason: ManagedFileConflict::Directory,
        })
    } else {
        Some(ManagedFileAction::Remove(relative_path))
    }
}

fn empty_parent_directory_cleanup_actions(
    root: &Path,
    cleanup_actions: &[ManagedFileAction],
) -> Result<Vec<ManagedFileAction>> {
    let removed_paths: BTreeSet<PathBuf> = cleanup_actions
        .iter()
        .filter_map(|action| match action {
            ManagedFileAction::Remove(relative_path) => Some(relative_path.clone()),
            _ => None,
        })
        .collect();
    let mut parent_dirs = BTreeSet::new();

    for removed_path in &removed_paths {
        let Some(parent) = removed_path.parent() else {
            continue;
        };
        if parent.as_os_str().is_empty() {
            continue;
        }
        let full_parent = root.join(parent);
        if !full_parent.is_dir() {
            continue;
        }
        if directory_will_be_empty_after_removals(&full_parent, parent, &removed_paths)? {
            parent_dirs.insert(parent.to_path_buf());
        }
    }

    Ok(parent_dirs
        .into_iter()
        .filter(|relative_path| !removed_paths.contains(relative_path))
        .map(ManagedFileAction::Remove)
        .collect())
}

fn directory_will_be_empty_after_removals(
    full_parent: &Path,
    relative_parent: &Path,
    removed_paths: &BTreeSet<PathBuf>,
) -> Result<bool> {
    for entry in fs::read_dir(full_parent)
        .with_context(|| format!("failed to read {}", full_parent.display()))?
    {
        let entry = entry.with_context(|| format!("failed to read {}", full_parent.display()))?;
        let relative_entry = relative_parent.join(entry.file_name());
        if !removed_paths.contains(&relative_entry) {
            return Ok(false);
        }
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::blueprint::BlueprintName;
    use crate::blueprint::files::{ManagedFileAction, ManagedFileConflict};
    use crate::commands::new::SelectedOption;
    use crate::commands::sync::{
        NonInteractiveApplyGuardInput, action_breakdown, cleanup_action_for_optional_path,
        cleanup_actions_for_blueprint, ensure_noninteractive_apply_allowed,
        required_tools_summary_for_options, should_confirm_sync, sync_command,
        sync_result_section_title, sync_review_summary, sync_status_code,
    };
    use tempfile::TempDir;

    #[cfg(unix)]
    #[test]
    fn cleanup_actions_include_broken_symlinks() {
        let temp = TempDir::new().expect("temp dir should create");
        std::os::unix::fs::symlink(
            "missing-prettier-config",
            temp.path().join(".prettierrc.json"),
        )
        .expect("broken symlink should create");
        let pyproject = r#"[tool.forge]
blueprint = "python-library"
project_name = "ops-tools"
package_name = "ops_tools"
description = "Ops toolchain"
author_name = "Grace Hopper"
author_email = "grace@example.com"
license = "MIT"
python_min = "3.12"

[tool.forge.overrides]
prettier = false
editorconfig = false
markdownlint = false
"#;

        let actions =
            cleanup_actions_for_blueprint(BlueprintName::PythonLibrary, temp.path(), pyproject)
                .expect("cleanup actions should render");

        assert!(actions.contains(&ManagedFileAction::Remove(PathBuf::from(
            ".prettierrc.json"
        ))));
    }

    #[test]
    fn optional_cleanup_directory_paths_are_conflicts() {
        let temp = TempDir::new().expect("temp dir should create");
        std::fs::create_dir(temp.path().join(".prettierrc.json")).expect("directory should create");

        let action =
            cleanup_action_for_optional_path(temp.path(), PathBuf::from(".prettierrc.json"))
                .expect("directory should produce action");

        assert_eq!(
            action,
            ManagedFileAction::Conflict {
                path: PathBuf::from(".prettierrc.json"),
                reason: ManagedFileConflict::Directory,
            }
        );
    }

    #[test]
    fn optional_cleanup_unsafe_paths_are_conflicts() {
        let temp = TempDir::new().expect("temp dir should create");

        let action = cleanup_action_for_optional_path(temp.path(), PathBuf::from("../escape"))
            .expect("unsafe path should produce action");

        assert_eq!(
            action,
            ManagedFileAction::Conflict {
                path: PathBuf::from("../escape"),
                reason: ManagedFileConflict::UnsafePath,
            }
        );
    }

    #[test]
    fn required_tools_summary_includes_component_tools_for_enabled_options() {
        let options = vec![
            SelectedOption {
                name: "docs",
                enabled: true,
            },
            SelectedOption {
                name: "markdownlint",
                enabled: true,
            },
        ];

        assert_eq!(
            required_tools_summary_for_options(BlueprintName::AnyProject, &options),
            "uv, just, npx"
        );
    }

    #[test]
    fn action_breakdown_counts_each_action_type() {
        let actions = vec![
            ManagedFileAction::Create(PathBuf::from("pyproject.toml")),
            ManagedFileAction::Update(PathBuf::from("justfile")),
            ManagedFileAction::Relink(PathBuf::from("CLAUDE.md")),
            ManagedFileAction::Remove(PathBuf::from(".prettierrc.json")),
            ManagedFileAction::Keep(PathBuf::from("README.md")),
            ManagedFileAction::Conflict {
                path: PathBuf::from("docs/src/content/docs/index.mdx"),
                reason: ManagedFileConflict::Directory,
            },
        ];

        let breakdown = action_breakdown(&actions);

        assert_eq!(breakdown.create, 1);
        assert_eq!(breakdown.update, 1);
        assert_eq!(breakdown.relink, 1);
        assert_eq!(breakdown.remove, 1);
        assert_eq!(breakdown.keep, 1);
        assert_eq!(breakdown.conflict, 1);
    }

    #[test]
    fn sync_confirmation_gate_requires_interactive_apply_mode() {
        assert!(should_confirm_sync(false, false, false, false, true, 1));
        assert!(!should_confirm_sync(true, false, false, false, true, 1));
        assert!(!should_confirm_sync(false, true, false, false, true, 1));
        assert!(!should_confirm_sync(false, false, true, false, true, 1));
        assert!(!should_confirm_sync(false, false, false, true, true, 1));
        assert!(!should_confirm_sync(false, false, false, false, false, 1));
        assert!(!should_confirm_sync(false, false, false, false, true, 0));
    }

    #[test]
    fn noninteractive_apply_mode_requires_yes_when_changes_exist() {
        let error = ensure_noninteractive_apply_allowed(NonInteractiveApplyGuardInput {
            assume_yes: false,
            json: false,
            dry_run: false,
            check: false,
            stdin_is_terminal: false,
            changes: 1,
            root: Path::new("/tmp/ops"),
            option_overrides: &[String::from("prettier=true")],
        })
        .expect_err("non-interactive apply mode should require explicit --yes");
        assert!(
            error
                .to_string()
                .contains("interactive confirmation requires a terminal")
        );
        assert!(
            error
                .to_string()
                .contains("forge sync --path /tmp/ops --set prettier=true --yes")
        );

        ensure_noninteractive_apply_allowed(NonInteractiveApplyGuardInput {
            assume_yes: true,
            json: false,
            dry_run: false,
            check: false,
            stdin_is_terminal: false,
            changes: 1,
            root: Path::new("/tmp/ops"),
            option_overrides: &[],
        })
        .expect("--yes should bypass the non-interactive guard");
        ensure_noninteractive_apply_allowed(NonInteractiveApplyGuardInput {
            assume_yes: false,
            json: true,
            dry_run: false,
            check: false,
            stdin_is_terminal: false,
            changes: 1,
            root: Path::new("/tmp/ops"),
            option_overrides: &[],
        })
        .expect("--json should bypass the non-interactive guard");
        ensure_noninteractive_apply_allowed(NonInteractiveApplyGuardInput {
            assume_yes: false,
            json: false,
            dry_run: true,
            check: false,
            stdin_is_terminal: false,
            changes: 1,
            root: Path::new("/tmp/ops"),
            option_overrides: &[],
        })
        .expect("--dry-run should bypass the non-interactive guard");
        ensure_noninteractive_apply_allowed(NonInteractiveApplyGuardInput {
            assume_yes: false,
            json: false,
            dry_run: false,
            check: true,
            stdin_is_terminal: false,
            changes: 1,
            root: Path::new("/tmp/ops"),
            option_overrides: &[],
        })
        .expect("--check should bypass the non-interactive guard");
        ensure_noninteractive_apply_allowed(NonInteractiveApplyGuardInput {
            assume_yes: false,
            json: false,
            dry_run: false,
            check: false,
            stdin_is_terminal: false,
            changes: 0,
            root: Path::new("/tmp/ops"),
            option_overrides: &[],
        })
        .expect("no-op updates should not require explicit --yes");
    }

    #[test]
    fn sync_command_preserves_overrides_and_appends_yes() {
        let command = sync_command(
            Path::new("/tmp/ops tools"),
            &[
                String::from("prettier=true"),
                String::from("markdownlint=false"),
            ],
        );

        assert_eq!(
            command,
            "forge sync --path '/tmp/ops tools' --set prettier=true --set markdownlint=false --yes"
        );
    }

    #[test]
    fn sync_review_summary_includes_apply_and_changes() {
        let options = vec![
            SelectedOption {
                name: "docs",
                enabled: true,
            },
            SelectedOption {
                name: "markdownlint",
                enabled: false,
            },
        ];

        let summary = sync_review_summary(
            Path::new("/tmp/ops tools"),
            BlueprintName::AnyProject,
            "0.1.0",
            &options,
            "pyproject.toml, justfile",
            3,
            String::from("forge sync --path '/tmp/ops tools' --yes"),
        );

        assert_eq!(summary.path, "/tmp/ops tools");
        assert_eq!(summary.blueprint, "any-project");
        assert_eq!(summary.blueprint_version, "0.1.0");
        assert_eq!(summary.options, "enabled: docs; disabled: markdownlint");
        assert_eq!(summary.required_tools, "uv, just");
        assert_eq!(summary.infrastructure, "pyproject.toml, justfile");
        assert_eq!(summary.changes, 3);
        assert_eq!(summary.apply, "forge sync --path '/tmp/ops tools' --yes");
    }

    #[test]
    fn sync_result_section_title_matches_mode_and_changes() {
        assert_eq!(sync_result_section_title(true, 0), "Project checked");
        assert_eq!(sync_result_section_title(true, 4), "Project checked");
        assert_eq!(sync_result_section_title(false, 0), "Project checked");
        assert_eq!(sync_result_section_title(false, 2), "Project synced");
    }

    #[test]
    fn sync_status_code_covers_json_outcomes() {
        assert_eq!(sync_status_code(false, false, 2, 1), "conflicts");
        assert_eq!(sync_status_code(false, true, 1, 0), "out_of_date");
        assert_eq!(sync_status_code(true, false, 3, 0), "dry_run");
        assert_eq!(sync_status_code(false, false, 0, 0), "current");
        assert_eq!(sync_status_code(false, false, 3, 0), "synced");
    }
}
