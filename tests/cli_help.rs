use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

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

[tool.forge.overrides]
prettier = {prettier}
"#
    )
}

fn python_forge_metadata_without_version_and_options() -> &'static str {
    r#"[tool.forge]
blueprint = "python-library"
project_name = "ops-tools"
package_name = "ops_tools"
description = "Ops toolchain"
author_name = "Grace Hopper"
author_email = "grace@example.com"
license = "MIT"
python_min = "3.12"
"#
}

fn canonical_display(path: &Path) -> String {
    path.canonicalize()
        .expect("path should be canonicalizable")
        .display()
        .to_string()
}

#[test]
fn top_level_help_lists_expected_commands() {
    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");

    cmd.arg("--help")
        .assert()
        .success()
        .stdout(contains("Quickstart:"))
        .stdout(contains("forge blueprints"))
        .stdout(contains("--description \"My library\""))
        .stdout(contains("forge sync --path ./my-lib --check"))
        .stdout(contains("--color"))
        .stdout(contains("blueprints"))
        .stdout(contains("components"))
        .stdout(contains("completions"))
        .stdout(contains("init"))
        .stdout(contains("new"))
        .stdout(contains("sync"))
        .stdout(contains("self"))
        .stdout(contains("upgrade").not())
        .stdout(contains("--plain").not());
}

#[test]
fn legacy_upgrade_subcommand_is_rejected() {
    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");

    cmd.arg("upgrade")
        .assert()
        .failure()
        .stderr(contains("unrecognized subcommand"))
        .stderr(contains("upgrade"))
        .stderr(contains("error_code: FORGE_E_CLI_USAGE"));
}

#[test]
fn legacy_plain_flag_is_rejected() {
    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");

    cmd.args(["--plain", "new", "--help"])
        .assert()
        .failure()
        .stderr(contains("unexpected argument '--plain'"))
        .stderr(contains("error_code: FORGE_E_CLI_USAGE"));
}

#[test]
fn top_level_without_args_shows_help_and_exits_success() {
    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");

    cmd.assert()
        .success()
        .stdout(contains(
            "Create projects and sync repository infrastructure from blueprints",
        ))
        .stdout(contains("Usage: forge [OPTIONS] [COMMAND]"))
        .stdout(contains("Quickstart:"));
}

#[test]
fn components_lists_available_optional_components() {
    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");

    cmd.arg("components")
        .assert()
        .success()
        .stdout(contains("Available components"))
        .stdout(contains("prettier"))
        .stdout(contains("editorconfig"))
        .stdout(contains("markdownlint"))
        .stdout(contains("option: prettier"))
        .stdout(contains("option: editorconfig"))
        .stdout(contains("option: markdownlint"))
        .stdout(contains(".prettierrc.json"))
        .stdout(contains(".editorconfig"))
        .stdout(contains(".markdownlint.jsonc"))
        .stdout(contains("required tools: npx"))
        .stdout(contains(
            "format command: npx --yes prettier@3.8.3 --write --ignore-unknown .",
        ))
        .stdout(contains(
            "check command: npx --yes prettier@3.8.3 --check --ignore-unknown .",
        ))
        .stdout(contains(
            "check command: npx --yes markdownlint-cli2@0.18.1 \"**/*.md\"",
        ))
        .stdout(contains("enable: forge sync --path . --set prettier=true"))
        .stdout(contains(
            "disable: forge sync --path . --set prettier=false",
        ))
        .stdout(contains(
            "enable: forge sync --path . --set markdownlint=true",
        ))
        .stdout(contains("python-library"))
        .stdout(contains("forge sync --path . --set prettier=true"));
}

#[test]
fn components_can_emit_json() {
    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");

    let output = cmd
        .args(["components", "--json"])
        .output()
        .expect("components should run");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(!stdout.contains("Available components"));

    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert_eq!(report["status_code"], "ok");
    let components = report["components"]
        .as_array()
        .expect("components output should be a JSON array");

    assert!(components.iter().any(|component| {
        component["name"] == "prettier"
            && component["option"] == "prettier"
            && component["description"]
                .as_str()
                .expect("description should be a string")
                .contains("JSON")
            && component["managed_files"]
                .as_array()
                .expect("managed_files should be an array")
                .iter()
                .any(|path| path == ".prettierrc.json")
            && component["required_tools"]
                .as_array()
                .expect("required_tools should be an array")
                .iter()
                .any(|tool| tool == "npx")
            && component["supported_blueprints"]
                .as_array()
                .expect("supported_blueprints should be an array")
                .iter()
                .any(|blueprint| blueprint == "rust-library")
            && component["pre_commit_hook"] == true
            && component["format_command"] == "npx --yes prettier@3.8.3 --write --ignore-unknown ."
            && component["check_command"] == "npx --yes prettier@3.8.3 --check --ignore-unknown ."
            && component["enable_command"] == "forge sync --path . --set prettier=true"
            && component["disable_command"] == "forge sync --path . --set prettier=false"
    }));
    assert!(components.iter().any(|component| {
        component["name"] == "editorconfig"
            && component["option"] == "editorconfig"
            && component["managed_files"]
                .as_array()
                .expect("managed_files should be an array")
                .iter()
                .any(|path| path == ".editorconfig")
            && component["required_tools"]
                .as_array()
                .expect("required_tools should be an array")
                .is_empty()
            && component["pre_commit_hook"] == false
            && component["format_command"].is_null()
            && component["check_command"].is_null()
            && component["enable_command"] == "forge sync --path . --set editorconfig=true"
            && component["disable_command"] == "forge sync --path . --set editorconfig=false"
    }));
    assert!(components.iter().any(|component| {
        component["name"] == "markdownlint"
            && component["option"] == "markdownlint"
            && component["managed_files"]
                .as_array()
                .expect("managed_files should be an array")
                .iter()
                .any(|path| path == ".markdownlint.jsonc")
            && component["required_tools"]
                .as_array()
                .expect("required_tools should be an array")
                .iter()
                .any(|tool| tool == "npx")
            && component["pre_commit_hook"] == true
            && component["format_command"] == "npx --yes markdownlint-cli2@0.18.1 --fix \"**/*.md\""
            && component["check_command"] == "npx --yes markdownlint-cli2@0.18.1 \"**/*.md\""
            && component["enable_command"] == "forge sync --path . --set markdownlint=true"
            && component["disable_command"] == "forge sync --path . --set markdownlint=false"
    }));
}

#[test]
fn components_help_exposes_blueprint_filter() {
    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");

    cmd.args(["components", "--help"])
        .assert()
        .success()
        .stdout(contains("--blueprint"))
        .stdout(contains("any-project"))
        .stdout(contains("python-library"))
        .stdout(contains("rust-library"));
}

#[test]
fn components_accept_blueprint_filter() {
    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");

    cmd.args(["components", "--blueprint", "python-library"])
        .assert()
        .success()
        .stdout(contains("Available components"))
        .stdout(contains("prettier"))
        .stdout(contains(
            "supported blueprints: any-project, python-library, rust-library",
        ));
}

#[test]
fn components_json_accepts_blueprint_filter() {
    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");

    let output = cmd
        .args(["components", "--blueprint", "rust-library", "--json"])
        .output()
        .expect("components should run");
    assert!(output.status.success());

    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should be valid JSON");
    let components = report["components"]
        .as_array()
        .expect("components output should be a JSON array");
    assert!(
        components
            .iter()
            .any(|component| component["name"] == "prettier")
    );
    assert!(
        components
            .iter()
            .any(|component| component["name"] == "editorconfig")
    );
    assert!(
        components
            .iter()
            .any(|component| component["name"] == "markdownlint")
    );
}

#[test]
fn completions_help_lists_supported_shells() {
    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");

    cmd.args(["completions", "--help"])
        .assert()
        .success()
        .stdout(contains("Generate shell completion scripts"))
        .stdout(contains("bash"))
        .stdout(contains("zsh"))
        .stdout(contains("fish"));
}

#[test]
fn completions_can_generate_bash_script() {
    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");

    cmd.args(["completions", "bash"])
        .assert()
        .success()
        .stdout(contains("forge"))
        .stdout(contains("blueprints"))
        .stdout(contains("update"));
}

#[test]
fn blueprints_lists_available_blueprints() {
    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");

    cmd.arg("blueprints")
        .assert()
        .success()
        .stdout(contains("Available blueprints"))
        .stdout(contains("any-project"))
        .stdout(contains("python-library"))
        .stdout(contains("rust-library"))
        .stdout(contains("version: 0.1.0").not())
        .stdout(contains("required tools: uv, just"))
        .stdout(contains("required tools: cargo, uv, just"))
        .stdout(contains(
            "managed: pyproject.toml metadata, justfile, prek hooks",
        ))
        .stdout(contains("CLAUDE.md link"))
        .stdout(contains("repository infrastructure only"))
        .stdout(contains("python package scaffolding"))
        .stdout(contains("cargo package scaffolding"))
        .stdout(contains(
            "fields: project-name (required), description (required)",
        ))
        .stdout(contains(
            "package-name (default: derived from project-name)",
        ))
        .stdout(contains(
            "create: forge new --blueprint python-library --yes ...",
        ))
        .stdout(contains(
            "init: forge init --path . --blueprint python-library --yes ...",
        ))
        .stdout(contains("check: forge sync --path . --check"));
}

#[test]
fn bp_alias_lists_available_blueprints() {
    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");

    cmd.arg("bp")
        .assert()
        .success()
        .stdout(contains("Available blueprints"))
        .stdout(contains("any-project"))
        .stdout(contains("python-library"))
        .stdout(contains("rust-library"));
}

#[test]
fn blueprints_can_emit_json() {
    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");

    let output = cmd
        .args(["blueprints", "--json"])
        .output()
        .expect("blueprints should run");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(!stdout.contains("Available blueprints"));

    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert_eq!(report["status_code"], "ok");
    let blueprints = report["blueprints"]
        .as_array()
        .expect("blueprints output should be a JSON array");

    assert!(blueprints.iter().any(|blueprint| {
        blueprint["name"] == "any-project"
            && blueprint["version"] == "0.1.0"
            && blueprint["summary"] == "managed infrastructure for any repository"
            && blueprint["create_command"] == "forge new --blueprint any-project --yes ..."
            && blueprint["init_command"] == "forge init --path . --blueprint any-project --yes ..."
            && blueprint["sync_check_command"] == "forge sync --path . --check"
            && blueprint["fields"]
                .as_array()
                .expect("fields should be an array")
                .iter()
                .any(|field| {
                    field["name"] == "project-name"
                        && field["required"] == true
                        && field["default"].is_null()
                })
            && blueprint["required_tools"]
                .as_array()
                .expect("required_tools should be an array")
                .iter()
                .any(|tool| tool == "uv")
            && blueprint["managed_highlights"]
                .as_array()
                .expect("managed_highlights should be an array")
                .iter()
                .any(|highlight| highlight == "CLAUDE.md link")
            && blueprint["managed_highlights"]
                .as_array()
                .expect("managed_highlights should be an array")
                .iter()
                .any(|highlight| highlight == "repository infrastructure only")
            && blueprint["options"]
                .as_array()
                .expect("options should be an array")
                .iter()
                .any(|option| {
                    option["name"] == "prettier"
                        && option["default_enabled"] == false
                        && option["description"]
                            .as_str()
                            .expect("description should be a string")
                            .contains("JSON")
                })
    }));
    assert!(blueprints.iter().any(|blueprint| {
        blueprint["name"] == "python-library"
            && blueprint["create_command"] == "forge new --blueprint python-library --yes ..."
            && blueprint["init_command"]
                == "forge init --path . --blueprint python-library --yes ..."
            && blueprint["sync_check_command"] == "forge sync --path . --check"
            && blueprint["fields"]
                .as_array()
                .expect("fields should be an array")
                .iter()
                .any(|field| {
                    field["name"] == "package-name"
                        && field["required"] == false
                        && field["default"] == "derived from project-name"
                })
            && blueprint["required_tools"]
                .as_array()
                .expect("required_tools should be an array")
                .iter()
                .any(|tool| tool == "just")
            && blueprint["managed_highlights"]
                .as_array()
                .expect("managed_highlights should be an array")
                .iter()
                .any(|highlight| highlight == "python package scaffolding")
            && blueprint["options"]
                .as_array()
                .expect("options should be an array")
                .iter()
                .any(|option| {
                    option["name"] == "codecov"
                        && option["default_enabled"] == true
                        && option["description"]
                            .as_str()
                            .expect("description should be a string")
                            .contains("Codecov")
                })
    }));
    assert!(blueprints.iter().any(|blueprint| {
        blueprint["name"] == "rust-library"
            && blueprint["create_command"] == "forge new --blueprint rust-library --yes ..."
            && blueprint["init_command"] == "forge init --path . --blueprint rust-library --yes ..."
            && blueprint["sync_check_command"] == "forge sync --path . --check"
            && blueprint["required_tools"]
                .as_array()
                .expect("required_tools should be an array")
                .iter()
                .any(|tool| tool == "cargo")
            && blueprint["managed_highlights"]
                .as_array()
                .expect("managed_highlights should be an array")
                .iter()
                .any(|highlight| highlight == "cargo package scaffolding")
            && !blueprint["options"]
                .as_array()
                .expect("options should be an array")
                .iter()
                .any(|option| option["name"] == "codecov")
    }));
}

#[test]
fn self_update_help_is_exposed() {
    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");

    cmd.args(["self", "update", "--help"])
        .assert()
        .success()
        .stdout(contains("Update forge itself"))
        .stdout(contains("--dry-run"))
        .stdout(contains("--token"));
}

#[test]
fn self_upgrade_alias_is_exposed() {
    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");

    cmd.args(["self", "upgrade", "--help"])
        .assert()
        .success()
        .stdout(contains("Usage: forge self update"));
}

#[test]
fn self_update_requires_standalone_installer_receipt() {
    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");

    cmd.args(["self", "update", "--dry-run"])
        .assert()
        .failure()
        .stderr(contains("cannot self-update"));
}

#[test]
fn new_help_exposes_blueprint_selection() {
    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");

    cmd.args(["new", "--help"])
        .assert()
        .success()
        .stdout(contains("--blueprint"))
        .stdout(contains("--dry-run"))
        .stdout(contains("--diff"))
        .stdout(contains("--json"))
        .stdout(contains("Defaults from the project name"))
        .stdout(contains("Defaults to BSD-3-Clause"))
        .stdout(contains("as major.minor"))
        .stdout(contains("Defaults to 3.11"))
        .stdout(contains("Defaults to public when --github is enabled"))
        .stdout(contains("Examples:"))
        .stdout(contains("forge new --blueprint rust-library"))
        .stdout(contains("--package-name tools"))
        .stdout(contains("--description \"Internal tools\""))
        .stdout(contains("any-project"))
        .stdout(contains("python-library"))
        .stdout(contains("rust-library"));
}

#[test]
fn init_help_exposes_existing_repo_setup() {
    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");

    cmd.args(["init", "--help"])
        .assert()
        .success()
        .stdout(contains("Initialize Forge-managed infrastructure"))
        .stdout(contains("--blueprint"))
        .stdout(contains("--dry-run"))
        .stdout(contains("--diff"))
        .stdout(contains("--force"))
        .stdout(contains("--json"))
        .stdout(contains("--yes"));
}

#[test]
fn new_help_yes_mode_examples_are_valid_as_dry_runs() {
    let temp = TempDir::new().expect("temp dir should create");

    let examples = [
        vec![
            "new".to_string(),
            "--blueprint".to_string(),
            "python-library".to_string(),
            "--path".to_string(),
            temp.path().join("my-lib").display().to_string(),
            "--project-name".to_string(),
            "my-lib".to_string(),
            "--description".to_string(),
            "My library".to_string(),
            "--author-name".to_string(),
            "Ada Lovelace".to_string(),
            "--author-email".to_string(),
            "ada@example.com".to_string(),
            "--yes".to_string(),
            "--dry-run".to_string(),
        ],
        vec![
            "new".to_string(),
            "--blueprint".to_string(),
            "rust-library".to_string(),
            "--path".to_string(),
            temp.path().join("tools").display().to_string(),
            "--project-name".to_string(),
            "tools".to_string(),
            "--package-name".to_string(),
            "tools".to_string(),
            "--description".to_string(),
            "Internal tools".to_string(),
            "--author-name".to_string(),
            "Ada Lovelace".to_string(),
            "--author-email".to_string(),
            "ada@example.com".to_string(),
            "--yes".to_string(),
            "--dry-run".to_string(),
        ],
        vec![
            "new".to_string(),
            "--blueprint".to_string(),
            "any-project".to_string(),
            "--path".to_string(),
            temp.path().join("infra").display().to_string(),
            "--project-name".to_string(),
            "infra".to_string(),
            "--description".to_string(),
            "Shared repo infrastructure".to_string(),
            "--yes".to_string(),
            "--dry-run".to_string(),
        ],
    ];

    for args in examples {
        let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
        cmd.args(args)
            .assert()
            .success()
            .stdout(contains("Project creation preview"));
    }
}

#[test]
fn doctor_uses_status_output() {
    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");

    cmd.arg("doctor")
        .assert()
        .stdout(contains("Forge doctor"))
        .stdout(contains("Required tools"))
        .stdout(contains("Optional tools"))
        .stdout(contains("cargo"))
        .stdout(contains("version:"))
        .stdout(contains("git"))
        .stdout(contains("uv"))
        .stdout(contains("just"))
        .stdout(contains("python3"))
        .stdout(contains("ruff"))
        .stdout(contains("gh cli"))
        .stdout(contains("npx"))
        .stdout(contains("ty"))
        .stdout(contains("prek"));
}

#[test]
fn doctor_help_exposes_json_output() {
    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");

    cmd.args(["doctor", "--help"])
        .assert()
        .success()
        .stdout(contains("--blueprint"))
        .stdout(contains("--path"))
        .stdout(contains("--json"))
        .stdout(contains("machine-readable JSON"));
}

#[test]
fn doctor_can_emit_json() {
    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");

    let output = cmd
        .args(["doctor", "--json"])
        .output()
        .expect("doctor should run");

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(!stdout.contains("Forge doctor"));

    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert!(report["status_code"].is_string());
    assert!(report["ok"].is_boolean());
    assert!(report["ok"].as_bool().is_some_and(|ok| {
        ok == report["missing_required"]
            .as_array()
            .is_some_and(|missing| missing.is_empty())
    }));
    assert!(
        report["tools"]
            .as_array()
            .expect("tools should be an array")
            .iter()
            .any(|tool| { tool["name"] == "cargo" && tool["required"] == true })
    );
    assert!(
        report["tools"]
            .as_array()
            .expect("tools should be an array")
            .iter()
            .any(|tool| {
                tool["name"] == "npx"
                    && tool["required"] == false
                    && tool["purpose"]
                        .as_str()
                        .is_some_and(|purpose| purpose.contains("Prettier"))
            })
    );
    assert!(
        report["tools"]
            .as_array()
            .expect("tools should be an array")
            .iter()
            .any(|tool| { tool["name"] == "python3" && tool["required"] == true })
    );
    assert!(
        report["tools"]
            .as_array()
            .expect("tools should be an array")
            .iter()
            .any(|tool| { tool["name"] == "ruff" && tool["required"] == true })
    );
}

#[test]
fn doctor_can_scope_required_tools_to_a_blueprint() {
    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");

    cmd.args(["doctor", "--blueprint", "python-library"])
        .env("PATH", "")
        .assert()
        .failure()
        .stdout(contains("scope: python-library"))
        .stdout(contains("git: missing"))
        .stdout(contains("uv: missing"))
        .stdout(contains("just: missing"))
        .stdout(contains("forge doctor --blueprint python-library"))
        .stdout(predicates::str::contains("cargo: missing").not())
        .stderr(contains(
            "required tools are missing: git, just, python3, ruff, uv",
        ));
}

#[test]
fn doctor_json_reports_blueprint_scoped_tool_contract() {
    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");

    let output = cmd
        .args(["doctor", "--blueprint", "python-library", "--json"])
        .env("PATH", "")
        .output()
        .expect("doctor should run");
    assert!(!output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert_eq!(report["scope_code"], "blueprint");
    assert_eq!(report["scope"], "python-library");
    assert_eq!(report["status_code"], "missing_required");
    assert_eq!(report["blueprint"], "python-library");
    assert_eq!(report["blueprint_version"], "0.1.0");
    assert_eq!(
        report["missing_required"],
        serde_json::json!(["git", "just", "python3", "ruff", "uv"])
    );
    assert_eq!(
        report["next_steps"],
        serde_json::json!([
            "install missing required tools: git, just, python3, ruff, uv",
            "forge doctor --blueprint python-library"
        ])
    );
    assert!(
        report["tools"]
            .as_array()
            .expect("tools should be an array")
            .iter()
            .all(|tool| tool["name"] != "cargo")
    );
}

#[test]
fn doctor_can_scope_required_tools_to_a_managed_project_path() {
    let temp = TempDir::new().expect("temp dir should create");
    fs::write(
        temp.path().join("pyproject.toml"),
        python_forge_metadata(false),
    )
    .expect("pyproject should be writable");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "doctor",
        "--path",
        temp.path().to_str().expect("valid UTF-8 path"),
    ])
    .env("PATH", "")
    .assert()
    .failure()
    .stdout(contains("path:"))
    .stdout(contains("scope: python-library"))
    .stdout(contains("blueprint version: 0.1.0"))
    .stdout(contains("git: missing"))
    .stdout(contains("uv: missing"))
    .stdout(contains("just: missing"))
    .stdout(contains(format!(
        "forge doctor --path {}",
        canonical_display(temp.path())
    )))
    .stdout(predicates::str::contains("cargo: missing").not())
    .stderr(contains(
        "required tools are missing: git, just, python3, ruff, uv",
    ));
}

#[test]
fn doctor_json_reports_path_scoped_tool_contract() {
    let temp = TempDir::new().expect("temp dir should create");
    fs::write(
        temp.path().join("pyproject.toml"),
        python_forge_metadata(false),
    )
    .expect("pyproject should be writable");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    let output = cmd
        .args([
            "doctor",
            "--path",
            temp.path().to_str().expect("valid UTF-8 path"),
            "--json",
        ])
        .env("PATH", "")
        .output()
        .expect("doctor should run");
    assert!(!output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert_eq!(report["scope_code"], "path");
    assert_eq!(report["scope"], "path");
    assert_eq!(report["status_code"], "missing_required");
    assert_eq!(report["blueprint"], "python-library");
    assert_eq!(report["blueprint_version"], "0.1.0");
    assert_eq!(report["path"], canonical_display(temp.path()));
    assert_eq!(
        report["missing_required"],
        serde_json::json!(["git", "just", "python3", "ruff", "uv"])
    );
    assert_eq!(
        report["next_steps"],
        serde_json::json!([
            "install missing required tools: git, just, python3, ruff, uv",
            format!("forge doctor --path {}", canonical_display(temp.path()))
        ])
    );
    assert!(
        report["tools"]
            .as_array()
            .expect("tools should be an array")
            .iter()
            .all(|tool| tool["name"] != "cargo")
    );
    assert!(
        report["tools"]
            .as_array()
            .expect("tools should be an array")
            .iter()
            .all(|tool| tool["name"] != "npx")
    );
}

#[test]
fn doctor_json_quotes_path_scoped_next_step_with_spaces() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("managed repo");
    fs::create_dir(&project_path).expect("project dir should create");
    fs::write(
        project_path.join("pyproject.toml"),
        python_forge_metadata(false),
    )
    .expect("pyproject should be writable");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    let output = cmd
        .args([
            "doctor",
            "--path",
            project_path.to_str().expect("valid UTF-8 path"),
            "--json",
        ])
        .env("PATH", "")
        .output()
        .expect("doctor should run");
    assert!(!output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert_eq!(report["scope_code"], "path");
    assert_eq!(report["scope"], "path");
    assert_eq!(
        report["next_steps"],
        serde_json::json!([
            "install missing required tools: git, just, python3, ruff, uv",
            format!("forge doctor --path '{}'", canonical_display(&project_path))
        ])
    );
}

#[test]
fn doctor_path_scope_reports_enabled_component_tools() {
    let temp = TempDir::new().expect("temp dir should create");
    fs::write(
        temp.path().join("pyproject.toml"),
        python_forge_metadata(true),
    )
    .expect("pyproject should be writable");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "doctor",
        "--path",
        temp.path().to_str().expect("valid UTF-8 path"),
    ])
    .env("PATH", "")
    .assert()
    .failure()
    .stdout(contains(
        "npx: missing (optional unless you need to run Prettier",
    ));
}

#[test]
fn doctor_path_rejects_corrupt_forge_metadata_before_tool_checks() {
    let temp = TempDir::new().expect("temp dir should create");
    let mut metadata = python_forge_metadata(false);
    metadata = metadata.replace(
        "python_min = \"3.12\"",
        "python_min = \"3.12\"\ntypo_field = true",
    );
    fs::write(temp.path().join("pyproject.toml"), metadata).expect("pyproject should be writable");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "doctor",
        "--path",
        temp.path().to_str().expect("valid UTF-8 path"),
    ])
    .env("PATH", "")
    .assert()
    .failure()
    .stdout(predicates::str::is_empty())
    .stderr(contains("failed to validate Forge metadata"))
    .stderr(contains("unknown field `typo_field`"))
    .stderr(contains("error_code: FORGE_E_ENV"))
    .stderr(contains("required tools are missing").not());
}

#[test]
fn doctor_path_rejects_unknown_forge_option_before_tool_checks() {
    let temp = TempDir::new().expect("temp dir should create");
    let mut metadata = python_forge_metadata(false);
    metadata = metadata.replace("prettier = false", "prettier = false\ncodcov = true");
    fs::write(temp.path().join("pyproject.toml"), metadata).expect("pyproject should be writable");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "doctor",
        "--path",
        temp.path().to_str().expect("valid UTF-8 path"),
    ])
    .env("PATH", "")
    .assert()
    .failure()
    .stdout(predicates::str::is_empty())
    .stderr(contains("failed to validate Forge metadata"))
    .stderr(contains("unsupported managed option 'codcov'"))
    .stderr(contains("error_code: FORGE_E_ENV"))
    .stderr(contains("required tools are missing").not());
}

#[test]
fn doctor_path_missing_pyproject_suggests_init_or_new() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("unmanaged repo");
    fs::create_dir(&project_path).expect("project dir should create");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "doctor",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
    ])
    .assert()
    .failure()
    .stderr(contains(format!(
        "missing Forge metadata at {}/pyproject.toml",
        canonical_display(&project_path)
    )))
    .stderr(contains(format!(
        "forge init --path '{}'",
        canonical_display(&project_path)
    )))
    .stderr(contains(format!(
        "forge new --path '{}'",
        canonical_display(&project_path)
    )));
}

#[test]
fn doctor_json_path_missing_pyproject_keeps_stdout_empty() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("unmanaged repo");
    fs::create_dir(&project_path).expect("project dir should create");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    let output = cmd
        .args([
            "doctor",
            "--path",
            project_path.to_str().expect("valid UTF-8 path"),
            "--json",
        ])
        .output()
        .expect("doctor should run");
    assert!(!output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.is_empty());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains(&format!(
        "missing Forge metadata at {}/pyproject.toml",
        canonical_display(&project_path)
    )));
}

#[test]
fn doctor_file_path_explains_repository_directory_requirement() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("pyproject.toml");
    fs::write(&project_path, "[project]\nname = \"not-a-directory\"\n")
        .expect("file path should be writable");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "doctor",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
    ])
    .assert()
    .failure()
    .stderr(contains(format!(
        "repository path is not a directory: {}",
        canonical_display(&project_path)
    )))
    .stderr(contains(
        "choose an existing Forge-managed repository directory",
    ))
    .stderr(contains("error_code: FORGE_E_ENV"));
}

#[test]
fn doctor_json_file_path_keeps_stdout_empty() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("pyproject.toml");
    fs::write(&project_path, "[project]\nname = \"not-a-directory\"\n")
        .expect("file path should be writable");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    let output = cmd
        .args([
            "doctor",
            "--path",
            project_path.to_str().expect("valid UTF-8 path"),
            "--json",
        ])
        .output()
        .expect("doctor should run");
    assert!(!output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.is_empty());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains(&format!(
        "repository path is not a directory: {}",
        canonical_display(&project_path)
    )));
    assert!(stderr.contains("choose an existing Forge-managed repository directory",));
    assert!(stderr.contains("error_code: FORGE_E_ENV"));
}

#[test]
fn doctor_path_without_forge_metadata_suggests_init() {
    let temp = TempDir::new().expect("temp dir should create");
    fs::write(
        temp.path().join("pyproject.toml"),
        "[project]\nname = \"unmanaged\"\nversion = \"0.1.0\"\n",
    )
    .expect("pyproject should be writable");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "doctor",
        "--path",
        temp.path().to_str().expect("valid UTF-8 path"),
    ])
    .assert()
    .failure()
    .stderr(contains("missing [tool.forge] metadata"))
    .stderr(contains(format!(
        "forge init --path {}",
        canonical_display(temp.path())
    )))
    .stderr(contains("error_code: FORGE_E_ENV"));
}

#[test]
fn doctor_json_path_without_forge_metadata_keeps_stdout_empty() {
    let temp = TempDir::new().expect("temp dir should create");
    fs::write(
        temp.path().join("pyproject.toml"),
        "[project]\nname = \"unmanaged\"\nversion = \"0.1.0\"\n",
    )
    .expect("pyproject should be writable");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    let output = cmd
        .args([
            "doctor",
            "--path",
            temp.path().to_str().expect("valid UTF-8 path"),
            "--json",
        ])
        .output()
        .expect("doctor should run");
    assert!(!output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.is_empty());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("missing [tool.forge] metadata"));
    assert!(stderr.contains("error_code: FORGE_E_ENV"));
}

#[test]
fn doctor_rejects_blueprint_and_path_together() {
    let temp = TempDir::new().expect("temp dir should create");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "doctor",
        "--blueprint",
        "python-library",
        "--path",
        temp.path().to_str().expect("valid UTF-8 path"),
    ])
    .assert()
    .failure()
    .stderr(contains("--blueprint cannot be used with --path"))
    .stderr(contains("error_code: FORGE_E_INPUT"));
}

#[test]
fn doctor_path_rejects_missing_version_and_options_metadata() {
    let temp = TempDir::new().expect("temp dir should create");
    fs::write(
        temp.path().join("pyproject.toml"),
        python_forge_metadata_without_version_and_options(),
    )
    .expect("pyproject should be writable");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "doctor",
        "--path",
        temp.path().to_str().expect("valid UTF-8 path"),
    ])
    .env("PATH", "")
    .assert()
    .failure()
    .stdout(predicates::str::is_empty())
    .stderr(contains("failed to detect Forge blueprint"))
    .stderr(contains("missing tool.forge.blueprint version"))
    .stderr(contains("error_code: FORGE_E_ENV"));
}

#[test]
fn doctor_json_path_rejects_missing_version_and_options_metadata() {
    let temp = TempDir::new().expect("temp dir should create");
    fs::write(
        temp.path().join("pyproject.toml"),
        python_forge_metadata_without_version_and_options(),
    )
    .expect("pyproject should be writable");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    let output = cmd
        .args([
            "doctor",
            "--json",
            "--path",
            temp.path().to_str().expect("valid UTF-8 path"),
        ])
        .env("PATH", "")
        .output()
        .expect("doctor should run");
    assert!(!output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.is_empty());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("failed to detect Forge blueprint"));
    assert!(stderr.contains("missing tool.forge.blueprint version"));
    assert!(stderr.contains("error_code: FORGE_E_ENV"));
}

#[test]
fn doctor_json_rejects_blueprint_and_path_with_empty_stdout() {
    let temp = TempDir::new().expect("temp dir should create");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    let output = cmd
        .args([
            "doctor",
            "--blueprint",
            "python-library",
            "--path",
            temp.path().to_str().expect("valid UTF-8 path"),
            "--json",
        ])
        .output()
        .expect("doctor should run");
    assert!(!output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.is_empty());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("--blueprint cannot be used with --path"));
    assert!(stderr.contains("error_code: FORGE_E_INPUT"));
}

#[test]
fn doctor_fails_when_required_tools_are_missing() {
    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");

    cmd.arg("doctor")
        .env("PATH", "")
        .assert()
        .failure()
        .stdout(contains("cargo: missing"))
        .stdout(contains("git: missing"))
        .stdout(contains("uv: missing"))
        .stdout(contains("just: missing"))
        .stdout(contains("Next steps"))
        .stdout(contains(
            "install missing required tools: cargo, git, just, python3, ruff, uv",
        ))
        .stdout(contains("forge doctor"))
        .stderr(contains("required tools are missing"))
        .stderr(contains("error_code: FORGE_E_ENV"));
}

#[test]
fn doctor_json_reports_missing_required_tools_before_failing() {
    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");

    let output = cmd
        .args(["doctor", "--json"])
        .env("PATH", "")
        .output()
        .expect("doctor should run");
    assert!(!output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert_eq!(report["scope_code"], "global");
    assert_eq!(report["scope"], "global");
    assert_eq!(report["status_code"], "missing_required");
    assert_eq!(report["ok"], false);
    assert_eq!(
        report["next_steps"],
        serde_json::json!([
            "install missing required tools: cargo, git, just, python3, ruff, uv",
            "forge doctor"
        ])
    );
    assert!(
        report["missing_required"]
            .as_array()
            .expect("missing_required should be an array")
            .iter()
            .any(|tool| tool == "cargo")
    );
    assert!(
        report["missing_required"]
            .as_array()
            .expect("missing_required should be an array")
            .iter()
            .all(|tool| tool != "npx")
    );
    let tools = report["tools"]
        .as_array()
        .expect("tools should be an array");
    assert!(
        tools
            .iter()
            .any(|tool| { tool["name"] == "cargo" && tool["status_code"] == "missing_required" })
    );
    assert!(
        tools
            .iter()
            .any(|tool| { tool["name"] == "npx" && tool["status_code"] == "missing_optional" })
    );

    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("required tools are missing"));
    assert!(stderr.contains("error_code: FORGE_E_ENV"));
}

#[test]
fn sync_help_exposes_dry_run_preview() {
    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");

    cmd.args(["sync", "--help"])
        .assert()
        .success()
        .stdout(contains("--dry-run"))
        .stdout(contains("--diff"))
        .stdout(contains("--check"))
        .stdout(contains("--set <OPTION=BOOL>"))
        .stdout(contains("--json"))
        .stdout(contains("--yes"))
        .stdout(contains("forge sync --path . --yes"))
        .stdout(contains("forge sync --path . --set prettier=true --yes"))
        .stdout(contains("Preview managed infrastructure changes"));
}
