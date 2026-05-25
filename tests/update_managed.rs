use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn generate_project(project_path: &std::path::Path) {
    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "python-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "ops-tools",
        "--package-name",
        "ops_tools",
        "--description",
        "Ops toolchain",
        "--author-name",
        "Grace Hopper",
        "--author-email",
        "grace@example.com",
        "--license",
        "MIT",
        "--python-min",
        "3.12",
        "--yes",
    ]);
    cmd.assert().success();
}

fn generate_project_with_markdownlint(project_path: &std::path::Path) {
    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "python-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "ops-tools",
        "--package-name",
        "ops_tools",
        "--description",
        "Ops toolchain",
        "--author-name",
        "Grace Hopper",
        "--author-email",
        "grace@example.com",
        "--license",
        "MIT",
        "--python-min",
        "3.12",
        "--markdownlint",
        "--yes",
    ]);
    cmd.assert().success();
}

fn canonical_display(path: &Path) -> String {
    path.canonicalize()
        .expect("path should be canonicalizable")
        .display()
        .to_string()
}

#[test]
fn update_missing_pyproject_suggests_init_or_new() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops tools");
    fs::create_dir_all(&project_path).expect("project dir should create");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
    ]);

    update
        .assert()
        .failure()
        .stderr(contains("missing Forge metadata"))
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
fn update_json_missing_pyproject_keeps_stdout_empty() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops tools");
    fs::create_dir_all(&project_path).expect("project dir should create");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--json",
    ]);

    let output = update.output().expect("update should run");
    assert!(!output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.is_empty());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("missing Forge metadata"));
    assert!(stderr.contains("forge init --path"));
    assert!(stderr.contains("forge new --path"));
    assert!(stderr.contains("error_code: FORGE_E_ENV"));
}

#[test]
fn update_pyproject_without_forge_metadata_suggests_init() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops tools");
    fs::create_dir_all(&project_path).expect("project dir should create");
    fs::write(
        project_path.join("pyproject.toml"),
        "[project]\nname = \"ops-tools\"\nversion = \"0.1.0\"\n",
    )
    .expect("pyproject should be writable");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
    ]);

    update
        .assert()
        .failure()
        .stderr(contains("missing [tool.forge] metadata"))
        .stderr(contains(format!(
            "forge init --path '{}'",
            canonical_display(&project_path)
        )));
}

#[test]
fn update_json_rejects_invalid_set_before_printing_report() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--set",
        "prettier=maybe",
        "--json",
    ]);

    let output = update.output().expect("update should run");
    assert!(!output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.is_empty());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("invalid value for option 'prettier'"));
    assert!(stderr.contains("error_code: FORGE_E_INPUT"));
}

#[test]
fn update_file_path_explains_repository_directory_requirement() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("pyproject.toml");
    fs::write(&project_path, "[project]\nname = \"not-a-directory\"\n")
        .expect("file path should be writable");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
    ]);

    update
        .assert()
        .failure()
        .stderr(contains(format!(
            "repository path is not a directory: {}",
            canonical_display(&project_path)
        )))
        .stderr(contains(
            "choose an existing Forge-managed repository directory",
        ));
}

#[test]
fn update_json_file_path_keeps_stdout_empty() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("pyproject.toml");
    fs::write(&project_path, "[project]\nname = \"not-a-directory\"\n")
        .expect("file path should be writable");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--json",
    ]);

    let output = update.output().expect("update should run");
    assert!(!output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.is_empty());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("repository path is not a directory"));
    assert!(stderr.contains("choose an existing Forge-managed repository directory"));
}

#[test]
fn update_only_rewrites_managed_infra_files() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let src_file = project_path.join("src/ops_tools/core.py");
    fs::write(
        &src_file,
        "def hello() -> str:\n    return \"custom user code\"\n",
    )
    .expect("should write src override");

    let justfile = project_path.join("justfile");
    fs::write(&justfile, "BROKEN\n").expect("should write broken justfile");
    let ci_workflow = project_path.join(".github/workflows/ci.yaml");
    fs::write(&ci_workflow, "BROKEN\n").expect("should write broken CI workflow");
    let claude_file = project_path.join("CLAUDE.md");
    fs::remove_file(&claude_file).expect("generated symlink should be removable");
    fs::write(&claude_file, "stale duplicate instructions\n")
        .expect("should write stale claude file");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
    ]);
    update
        .assert()
        .success()
        .stdout(contains("Project updated"))
        .stdout(contains("[ok] managed infrastructure refreshed"))
        .stdout(contains("blueprint: python-library"))
        .stdout(contains("required tools: uv, just"))
        .stdout(contains("infrastructure:"))
        .stdout(contains("prek hooks"))
        .stdout(contains("Next steps").not());

    let src_after = fs::read_to_string(src_file).expect("source should remain readable");
    assert!(src_after.contains("custom user code"));

    let just_after = fs::read_to_string(justfile).expect("justfile should remain readable");
    assert!(!just_after.contains("BROKEN"));
    assert!(just_after.contains("verify"));
    assert!(just_after.contains("uv run --locked ruff format --check ."));
    assert!(just_after.contains("uv lock --check"));
    assert!(just_after.contains("uv run --locked ruff check ."));
    assert!(just_after.contains("uv run --locked prek run --all-files"));
    assert!(just_after.contains("forge update --path . --check"));
    assert_eq!(
        fs::read_link(claude_file).expect("update should restore CLAUDE.md as a symlink"),
        std::path::PathBuf::from("AGENTS.md")
    );
}

#[test]
fn update_refreshes_managed_infra() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let justfile = project_path.join("justfile");
    fs::write(&justfile, "BROKEN\n").expect("should write broken justfile");
    let ci_workflow = project_path.join(".github/workflows/ci.yaml");
    fs::write(&ci_workflow, "BROKEN\n").expect("should write broken CI workflow");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
    ]);
    update.assert().success();

    let just_after = fs::read_to_string(justfile).expect("justfile should remain readable");
    assert!(!just_after.contains("BROKEN"));
    assert!(just_after.contains("verify"));
    let ci_after = fs::read_to_string(ci_workflow).expect("CI workflow should remain readable");
    assert!(ci_after.contains("uv run --locked ruff format --check ."));
    assert!(ci_after.contains("uv lock --check"));
    assert!(ci_after.contains("uv run --locked ruff check ."));
    assert!(ci_after.contains("forge update --path . --check"));
}

#[test]
fn update_apply_without_yes_in_noninteractive_mode_fails_fast() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let justfile = project_path.join("justfile");
    fs::write(&justfile, "BROKEN\n").expect("should write broken justfile");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
    ]);
    update
        .assert()
        .failure()
        .stderr(contains("interactive confirmation requires a terminal"))
        .stderr(contains("forge update --path"))
        .stderr(contains("--yes"))
        .stderr(contains("or pass --json, --dry-run, or --check"))
        .stderr(contains("error_code: FORGE_E_INPUT"));

    let just_after = fs::read_to_string(justfile).expect("justfile should remain readable");
    assert_eq!(just_after, "BROKEN\n");
}

#[test]
fn update_noninteractive_error_includes_apply_command_with_overrides() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops tools");
    generate_project(&project_path);

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--set",
        "prettier=true",
    ]);
    update
        .assert()
        .failure()
        .stderr(contains("interactive confirmation requires a terminal"))
        .stderr(contains(format!(
            "forge update --path '{}' --set prettier=true --yes",
            canonical_display(&project_path)
        )))
        .stderr(contains("or pass --json, --dry-run, or --check"))
        .stderr(contains("error_code: FORGE_E_INPUT"));
}

#[test]
fn update_apply_without_yes_allows_noop_in_noninteractive_mode() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
    ]);
    update
        .assert()
        .success()
        .stdout(contains("Project checked"))
        .stdout(contains("[ok] managed infrastructure is already current"))
        .stderr(predicates::str::is_empty());
}

#[test]
fn update_rejects_metadata_without_options_table() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let pyproject_path = project_path.join("pyproject.toml");
    let pyproject = fs::read_to_string(&pyproject_path).expect("pyproject should be readable");
    let mut pyproject_value: toml::Value =
        toml::from_str(&pyproject).expect("pyproject should parse as TOML");
    pyproject_value["tool"]["forge"]
        .as_table_mut()
        .expect("tool.forge should be a table")
        .remove("options");
    fs::write(
        &pyproject_path,
        toml::to_string_pretty(&pyproject_value).expect("pyproject should serialize"),
    )
    .expect("pyproject should be writable");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
    ]);
    update
        .assert()
        .failure()
        .stdout(predicates::str::is_empty())
        .stderr(contains("failed to validate Forge metadata at"))
        .stderr(contains("missing tool.forge.options.docs"))
        .stderr(contains("error_code: FORGE_E_ENV"));
}

#[test]
fn update_set_rejects_missing_options_table() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let pyproject_path = project_path.join("pyproject.toml");
    let pyproject = fs::read_to_string(&pyproject_path).expect("pyproject should be readable");
    let mut pyproject_value: toml::Value =
        toml::from_str(&pyproject).expect("pyproject should parse as TOML");
    pyproject_value["tool"]["forge"]
        .as_table_mut()
        .expect("tool.forge should be a table")
        .remove("options");
    fs::write(
        &pyproject_path,
        toml::to_string_pretty(&pyproject_value).expect("pyproject should serialize"),
    )
    .expect("pyproject should be writable");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--set",
        "prettier=true",
    ]);
    update
        .assert()
        .failure()
        .stdout(predicates::str::is_empty())
        .stderr(contains("missing tool.forge.options.docs"))
        .stderr(contains("error_code: FORGE_E_ENV"));
}

#[test]
fn update_check_rejects_missing_options_table() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let pyproject_path = project_path.join("pyproject.toml");
    let pyproject = fs::read_to_string(&pyproject_path).expect("pyproject should be readable");
    let mut pyproject_value: toml::Value =
        toml::from_str(&pyproject).expect("pyproject should parse as TOML");
    pyproject_value["tool"]["forge"]
        .as_table_mut()
        .expect("tool.forge should be a table")
        .remove("options");
    fs::write(
        &pyproject_path,
        toml::to_string_pretty(&pyproject_value).expect("pyproject should serialize"),
    )
    .expect("pyproject should be writable");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--check",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
    ]);
    update
        .assert()
        .failure()
        .stdout(predicates::str::is_empty())
        .stderr(contains("missing tool.forge.options.docs"))
        .stderr(contains("error_code: FORGE_E_ENV"));
}

#[test]
fn update_dry_run_rejects_missing_options_table() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let pyproject_path = project_path.join("pyproject.toml");
    let pyproject = fs::read_to_string(&pyproject_path).expect("pyproject should be readable");
    let mut pyproject_value: toml::Value =
        toml::from_str(&pyproject).expect("pyproject should parse as TOML");
    pyproject_value["tool"]["forge"]
        .as_table_mut()
        .expect("tool.forge should be a table")
        .remove("options");
    fs::write(
        &pyproject_path,
        toml::to_string_pretty(&pyproject_value).expect("pyproject should serialize"),
    )
    .expect("pyproject should be writable");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--dry-run",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
    ]);
    update
        .assert()
        .failure()
        .stdout(predicates::str::is_empty())
        .stderr(contains("missing tool.forge.options.docs"))
        .stderr(contains("error_code: FORGE_E_ENV"));
}

#[test]
fn update_check_json_rejects_missing_options_table() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let pyproject_path = project_path.join("pyproject.toml");
    let pyproject = fs::read_to_string(&pyproject_path).expect("pyproject should be readable");
    let mut pyproject_value: toml::Value =
        toml::from_str(&pyproject).expect("pyproject should parse as TOML");
    pyproject_value["tool"]["forge"]
        .as_table_mut()
        .expect("tool.forge should be a table")
        .remove("options");
    fs::write(
        &pyproject_path,
        toml::to_string_pretty(&pyproject_value).expect("pyproject should serialize"),
    )
    .expect("pyproject should be writable");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--check",
        "--json",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
    ]);

    let output = update.output().expect("update should run");
    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.is_empty());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("missing tool.forge.options.docs"));
    assert!(stderr.contains("error_code: FORGE_E_ENV"));
}

#[test]
fn update_dry_run_json_rejects_missing_options_table() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let pyproject_path = project_path.join("pyproject.toml");
    let pyproject = fs::read_to_string(&pyproject_path).expect("pyproject should be readable");
    let mut pyproject_value: toml::Value =
        toml::from_str(&pyproject).expect("pyproject should parse as TOML");
    pyproject_value["tool"]["forge"]
        .as_table_mut()
        .expect("tool.forge should be a table")
        .remove("options");
    fs::write(
        &pyproject_path,
        toml::to_string_pretty(&pyproject_value).expect("pyproject should serialize"),
    )
    .expect("pyproject should be writable");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--dry-run",
        "--json",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
    ]);

    let output = update.output().expect("update should run");
    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("missing tool.forge.options.docs"));
    assert!(stderr.contains("error_code: FORGE_E_ENV"));
}

#[test]
fn update_rejects_missing_blueprint_version_metadata() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let pyproject_path = project_path.join("pyproject.toml");
    let pyproject = fs::read_to_string(&pyproject_path).expect("pyproject should be readable");
    let mut pyproject_value: toml::Value =
        toml::from_str(&pyproject).expect("pyproject should parse as TOML");
    pyproject_value["tool"]["forge"]
        .as_table_mut()
        .expect("tool.forge should be a table")
        .remove("blueprint_version");
    fs::write(
        &pyproject_path,
        toml::to_string_pretty(&pyproject_value).expect("pyproject should serialize"),
    )
    .expect("pyproject should be writable");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
    ]);
    update
        .assert()
        .failure()
        .stdout(predicates::str::is_empty())
        .stderr(contains("failed to validate Forge metadata at"))
        .stderr(contains("missing tool.forge.blueprint_version"))
        .stderr(contains("error_code: FORGE_E_ENV"));
}

#[test]
fn update_check_rejects_missing_blueprint_version() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let pyproject_path = project_path.join("pyproject.toml");
    let pyproject = fs::read_to_string(&pyproject_path).expect("pyproject should be readable");
    let mut pyproject_value: toml::Value =
        toml::from_str(&pyproject).expect("pyproject should parse as TOML");
    pyproject_value["tool"]["forge"]
        .as_table_mut()
        .expect("tool.forge should be a table")
        .remove("blueprint_version");
    fs::write(
        &pyproject_path,
        toml::to_string_pretty(&pyproject_value).expect("pyproject should serialize"),
    )
    .expect("pyproject should be writable");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--check",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
    ]);
    update
        .assert()
        .failure()
        .stdout(predicates::str::is_empty())
        .stderr(contains("missing tool.forge.blueprint_version"))
        .stderr(contains("error_code: FORGE_E_ENV"));
}

#[test]
fn update_check_json_rejects_missing_blueprint_version() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let pyproject_path = project_path.join("pyproject.toml");
    let pyproject = fs::read_to_string(&pyproject_path).expect("pyproject should be readable");
    let mut pyproject_value: toml::Value =
        toml::from_str(&pyproject).expect("pyproject should parse as TOML");
    pyproject_value["tool"]["forge"]
        .as_table_mut()
        .expect("tool.forge should be a table")
        .remove("blueprint_version");
    fs::write(
        &pyproject_path,
        toml::to_string_pretty(&pyproject_value).expect("pyproject should serialize"),
    )
    .expect("pyproject should be writable");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--check",
        "--json",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
    ]);
    let output = update.output().expect("update should run");
    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("missing tool.forge.blueprint_version"));
    assert!(stderr.contains("error_code: FORGE_E_ENV"));
}

#[test]
fn update_set_rejects_missing_version_and_options_metadata() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let pyproject_path = project_path.join("pyproject.toml");
    let pyproject = fs::read_to_string(&pyproject_path).expect("pyproject should be readable");
    let mut pyproject_value: toml::Value =
        toml::from_str(&pyproject).expect("pyproject should parse as TOML");
    pyproject_value["tool"]["forge"]
        .as_table_mut()
        .expect("tool.forge should be a table")
        .remove("blueprint_version");
    pyproject_value["tool"]["forge"]
        .as_table_mut()
        .expect("tool.forge should be a table")
        .remove("options");
    fs::write(
        &pyproject_path,
        toml::to_string_pretty(&pyproject_value).expect("pyproject should serialize"),
    )
    .expect("pyproject should be writable");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--set",
        "prettier=true",
    ]);
    update
        .assert()
        .failure()
        .stdout(predicates::str::is_empty())
        .stderr(contains("missing tool.forge.blueprint_version"))
        .stderr(contains("error_code: FORGE_E_ENV"));
}

#[test]
fn update_json_rejects_missing_version_and_options_metadata() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let pyproject_path = project_path.join("pyproject.toml");
    let pyproject = fs::read_to_string(&pyproject_path).expect("pyproject should be readable");
    let mut pyproject_value: toml::Value =
        toml::from_str(&pyproject).expect("pyproject should parse as TOML");
    pyproject_value["tool"]["forge"]
        .as_table_mut()
        .expect("tool.forge should be a table")
        .remove("blueprint_version");
    pyproject_value["tool"]["forge"]
        .as_table_mut()
        .expect("tool.forge should be a table")
        .remove("options");
    fs::write(
        &pyproject_path,
        toml::to_string_pretty(&pyproject_value).expect("pyproject should serialize"),
    )
    .expect("pyproject should be writable");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    let output = update
        .args([
            "update",
            "--yes",
            "--json",
            "--path",
            project_path.to_str().expect("valid UTF-8 path"),
        ])
        .output()
        .expect("update should run");
    assert!(!output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("missing tool.forge.blueprint_version"));
    assert!(stderr.contains("error_code: FORGE_E_ENV"));
}

#[test]
fn update_check_json_rejects_missing_version_and_options_metadata() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let pyproject_path = project_path.join("pyproject.toml");
    let pyproject = fs::read_to_string(&pyproject_path).expect("pyproject should be readable");
    let mut pyproject_value: toml::Value =
        toml::from_str(&pyproject).expect("pyproject should parse as TOML");
    pyproject_value["tool"]["forge"]
        .as_table_mut()
        .expect("tool.forge should be a table")
        .remove("blueprint_version");
    pyproject_value["tool"]["forge"]
        .as_table_mut()
        .expect("tool.forge should be a table")
        .remove("options");
    fs::write(
        &pyproject_path,
        toml::to_string_pretty(&pyproject_value).expect("pyproject should serialize"),
    )
    .expect("pyproject should be writable");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    let output = update
        .args([
            "update",
            "--check",
            "--json",
            "--path",
            project_path.to_str().expect("valid UTF-8 path"),
        ])
        .output()
        .expect("update should run");
    assert!(!output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("missing tool.forge.blueprint_version"));
    assert!(stderr.contains("error_code: FORGE_E_ENV"));
}

#[test]
fn update_dry_run_json_rejects_missing_version_and_options_metadata() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let pyproject_path = project_path.join("pyproject.toml");
    let pyproject = fs::read_to_string(&pyproject_path).expect("pyproject should be readable");
    let mut pyproject_value: toml::Value =
        toml::from_str(&pyproject).expect("pyproject should parse as TOML");
    pyproject_value["tool"]["forge"]
        .as_table_mut()
        .expect("tool.forge should be a table")
        .remove("blueprint_version");
    pyproject_value["tool"]["forge"]
        .as_table_mut()
        .expect("tool.forge should be a table")
        .remove("options");
    fs::write(
        &pyproject_path,
        toml::to_string_pretty(&pyproject_value).expect("pyproject should serialize"),
    )
    .expect("pyproject should be writable");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    let output = update
        .args([
            "update",
            "--dry-run",
            "--json",
            "--path",
            project_path.to_str().expect("valid UTF-8 path"),
        ])
        .output()
        .expect("update should run");
    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("missing tool.forge.blueprint_version"));
    assert!(stderr.contains("error_code: FORGE_E_ENV"));
}

#[test]
fn update_rejects_unknown_forge_metadata_fields() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let pyproject_path = project_path.join("pyproject.toml");
    let pyproject = fs::read_to_string(&pyproject_path).expect("pyproject should be readable");
    let mut pyproject_value: toml::Value =
        toml::from_str(&pyproject).expect("pyproject should parse as TOML");
    pyproject_value["tool"]["forge"]
        .as_table_mut()
        .expect("tool.forge should be a table")
        .insert("typo_field".to_string(), toml::Value::Boolean(true));
    fs::write(
        &pyproject_path,
        toml::to_string_pretty(&pyproject_value).expect("pyproject should serialize"),
    )
    .expect("pyproject should be writable");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
    ]);
    update
        .assert()
        .failure()
        .stderr(contains("unknown field `typo_field`"))
        .stderr(contains("error_code: FORGE_E_ENV"));
}

#[test]
fn update_rejects_unknown_forge_option_fields() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let pyproject_path = project_path.join("pyproject.toml");
    let pyproject = fs::read_to_string(&pyproject_path).expect("pyproject should be readable");
    let mut pyproject_value: toml::Value =
        toml::from_str(&pyproject).expect("pyproject should parse as TOML");
    pyproject_value["tool"]["forge"]["options"]
        .as_table_mut()
        .expect("tool.forge.options should be a table")
        .insert("codcov".to_string(), toml::Value::Boolean(true));
    fs::write(
        &pyproject_path,
        toml::to_string_pretty(&pyproject_value).expect("pyproject should serialize"),
    )
    .expect("pyproject should be writable");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
    ]);
    update
        .assert()
        .failure()
        .stderr(contains("unsupported managed option 'codcov'"))
        .stderr(contains("error_code: FORGE_E_ENV"));
}

#[test]
fn update_rejects_missing_supported_option_keys() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let pyproject_path = project_path.join("pyproject.toml");
    let pyproject = fs::read_to_string(&pyproject_path).expect("pyproject should be readable");
    let mut pyproject_value: toml::Value =
        toml::from_str(&pyproject).expect("pyproject should parse as TOML");
    pyproject_value["tool"]["forge"]["options"]
        .as_table_mut()
        .expect("tool.forge.options should be a table")
        .remove("prettier");
    fs::write(
        &pyproject_path,
        toml::to_string_pretty(&pyproject_value).expect("pyproject should serialize"),
    )
    .expect("pyproject should be writable");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
    ]);
    update
        .assert()
        .failure()
        .stdout(predicates::str::is_empty())
        .stderr(contains("missing tool.forge.options.prettier"))
        .stderr(contains("error_code: FORGE_E_ENV"));
}

#[test]
fn update_rejects_unknown_forge_metadata() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let pyproject_path = project_path.join("pyproject.toml");
    let pyproject = fs::read_to_string(&pyproject_path).expect("pyproject should be readable");
    let mut pyproject_value: toml::Value =
        toml::from_str(&pyproject).expect("pyproject should parse as TOML");
    pyproject_value["tool"]["forge"]
        .as_table_mut()
        .expect("tool.forge should be a table")
        .insert(
            "metadata_owner".to_string(),
            toml::Value::String("ops".to_string()),
        );
    fs::write(
        &pyproject_path,
        toml::to_string_pretty(&pyproject_value).expect("pyproject should serialize"),
    )
    .expect("pyproject should be writable");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
    ]);
    update
        .assert()
        .failure()
        .stderr(contains("unknown field `metadata_owner`"))
        .stderr(contains("error_code: FORGE_E_ENV"));
}

#[test]
fn update_rejects_newer_blueprint_version_than_supported() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let pyproject_path = project_path.join("pyproject.toml");
    let pyproject = fs::read_to_string(&pyproject_path).expect("pyproject should be readable");
    let mut pyproject_value: toml::Value =
        toml::from_str(&pyproject).expect("pyproject should parse as TOML");
    pyproject_value["tool"]["forge"]["blueprint_version"] =
        toml::Value::String("9.0.0".to_string());
    fs::write(
        &pyproject_path,
        toml::to_string_pretty(&pyproject_value).expect("pyproject should serialize"),
    )
    .expect("pyproject should be writable");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
    ]);
    update
        .assert()
        .failure()
        .stderr(contains("newer than this forge supports"))
        .stderr(contains("upgrade forge"))
        .stderr(contains("error_code: FORGE_E_ENV"));
}

#[test]
fn update_dry_run_previews_without_writing_files() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let justfile = project_path.join("justfile");
    fs::write(&justfile, "BROKEN\n").expect("should write broken justfile");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--dry-run",
    ]);
    update
        .assert()
        .success()
        .stdout(contains("Managed changes preview"))
        .stdout(contains("update: 1"))
        .stdout(contains("update  justfile"))
        .stdout(contains("infrastructure:"))
        .stdout(contains("dry run complete; no files changed"));

    let just_after = fs::read_to_string(justfile).expect("justfile should remain readable");
    assert_eq!(just_after, "BROKEN\n");
}

#[test]
fn update_dry_run_diff_shows_text_changes_without_writing() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let justfile = project_path.join("justfile");
    fs::write(&justfile, "BROKEN\n").expect("should write broken justfile");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--dry-run",
        "--diff",
    ]);
    update
        .assert()
        .success()
        .stdout(contains("Managed diff"))
        .stdout(contains("--- a/justfile"))
        .stdout(contains("+++ b/justfile"))
        .stdout(contains("@@ -1,1 +1,"))
        .stdout(contains("-BROKEN"))
        .stdout(contains("+set dotenv-load := false"))
        .stdout(contains("dry run complete; no files changed"));

    let just_after = fs::read_to_string(justfile).expect("justfile should remain readable");
    assert_eq!(just_after, "BROKEN\n");
}

#[test]
fn update_diff_requires_dry_run_or_check() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let justfile = project_path.join("justfile");
    fs::write(&justfile, "BROKEN\n").expect("should write broken justfile");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--diff",
    ]);
    update
        .assert()
        .failure()
        .stderr(contains("--diff requires --dry-run or --check"));

    let just_after = fs::read_to_string(justfile).expect("justfile should remain readable");
    assert_eq!(just_after, "BROKEN\n");
}

#[test]
fn update_reports_conflicts_before_writing_any_files() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let justfile = project_path.join("justfile");
    fs::remove_file(&justfile).expect("managed justfile should be removable");
    fs::create_dir(&justfile).expect("conflicting justfile directory should create");
    let ci_workflow = project_path.join(".github/workflows/ci.yaml");
    fs::write(&ci_workflow, "BROKEN\n").expect("should write broken CI workflow");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
    ]);
    update
        .assert()
        .failure()
        .stdout(contains("conflict justfile (managed path is a directory)"))
        .stdout(contains("conflicts: 1"))
        .stdout(contains("blueprint: python-library"))
        .stdout(contains("infrastructure:"))
        .stdout(contains("Next steps"))
        .stdout(contains("resolve conflicted paths and rerun update"))
        .stderr(contains("managed infrastructure has conflicts"));

    let ci_after = fs::read_to_string(ci_workflow).expect("CI workflow should remain readable");
    assert_eq!(ci_after, "BROKEN\n");
}

#[test]
fn update_reports_parent_path_conflicts_before_writing_any_files() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let justfile = project_path.join("justfile");
    fs::write(&justfile, "BROKEN\n").expect("should write broken justfile");

    let github_dir = project_path.join(".github");
    fs::remove_dir_all(&github_dir).expect("generated .github directory should be removable");
    fs::write(&github_dir, "not a directory\n").expect("conflicting .github file should write");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
    ]);
    update
        .assert()
        .failure()
        .stdout(contains(
            "conflict .github/workflows/ci.yaml (managed parent path is not a directory)",
        ))
        .stdout(contains("conflicts:"))
        .stderr(contains("managed infrastructure has conflicts"));

    let just_after = fs::read_to_string(justfile).expect("justfile should remain readable");
    assert_eq!(just_after, "BROKEN\n");
}

#[cfg(unix)]
#[test]
fn update_reports_unreadable_managed_text_file_conflict_before_writing() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let justfile = project_path.join("justfile");
    fs::remove_file(&justfile).expect("generated justfile should be removable");
    std::os::unix::fs::symlink("missing-justfile", &justfile)
        .expect("broken justfile symlink should create");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
    ]);
    update
        .assert()
        .failure()
        .stdout(contains(
            "conflict justfile (managed text file cannot be read)",
        ))
        .stderr(contains("managed infrastructure has conflicts"));

    assert_eq!(
        fs::read_link(justfile).expect("conflicted path should remain a symlink"),
        std::path::PathBuf::from("missing-justfile")
    );
}

#[test]
fn update_json_reports_conflicts_before_failing() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let claude_file = project_path.join("CLAUDE.md");
    fs::remove_file(&claude_file).expect("managed symlink should be removable");
    fs::create_dir(&claude_file).expect("conflicting claude directory should create");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--json",
    ]);

    let output = update.output().expect("update should run");
    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(!stdout.contains("Managed changes"));
    assert!(!stdout.contains("Next steps"));
    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert_eq!(report["blueprint_version"], "0.1.0");
    assert_eq!(report["status_code"], "conflicts");
    assert_eq!(report["conflicts"], 1);
    assert_eq!(
        report["infrastructure"],
        "pyproject.toml, justfile, prek hooks, AGENTS.md, CLAUDE.md link, docs, github actions (3)"
    );
    assert_eq!(report["action_counts"]["conflict"], 1);
    assert_eq!(
        report["next_steps"],
        serde_json::json!(["resolve conflicted paths and rerun update"])
    );

    let conflict = report["actions"]
        .as_array()
        .expect("actions should be an array")
        .iter()
        .find(|action| action["action"] == "conflict" && action["path"] == "CLAUDE.md")
        .expect("conflict action should be reported");
    assert_eq!(conflict["reason_code"], "directory");
    assert_eq!(conflict["reason"], "managed path is a directory");
    assert_eq!(conflict["changes_filesystem"], false);

    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("managed infrastructure has conflicts"));
    assert!(stderr.contains("error_code: FORGE_E_CONFLICT"));
}

#[test]
fn update_dry_run_can_emit_json_report_without_human_output() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let justfile = project_path.join("justfile");
    fs::write(&justfile, "BROKEN\n").expect("should write broken justfile");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--dry-run",
        "--json",
    ]);

    let output = update.output().expect("update should run");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(!stdout.contains("Managed changes preview"));

    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert_eq!(report["blueprint"], "python-library");
    assert_eq!(report["blueprint_version"], "0.1.0");
    assert_eq!(report["status_code"], "dry_run");
    assert_eq!(report["dry_run"], true);
    assert_eq!(report["required_tools"], "uv, just");
    assert_eq!(report["conflicts"], 0);
    assert!(
        report["infrastructure"]
            .as_str()
            .expect("infrastructure should be a string")
            .contains("pyproject.toml")
    );
    assert_eq!(report["action_counts"]["update"], 1);
    assert_eq!(report["action_counts"]["conflict"], 0);
    assert_eq!(
        report["next_steps"],
        serde_json::json!([format!(
            "forge update --path {} --yes",
            canonical_display(&project_path)
        )])
    );
    assert!(
        report["actions"]
            .as_array()
            .expect("actions should be an array")
            .iter()
            .any(|action| action["action"] == "update" && action["path"] == "justfile")
    );

    let just_after = fs::read_to_string(justfile).expect("justfile should remain readable");
    assert_eq!(just_after, "BROKEN\n");
}

#[test]
fn update_can_emit_json_report_while_applying_changes() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let justfile = project_path.join("justfile");
    fs::write(&justfile, "BROKEN\n").expect("should write broken justfile");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--json",
    ]);

    let output = update.output().expect("update should run");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(!stdout.contains("Managed changes"));
    assert!(!stdout.contains("Project updated"));

    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert_eq!(report["blueprint"], "python-library");
    assert_eq!(report["blueprint_version"], "0.1.0");
    assert_eq!(report["status_code"], "updated");
    assert_eq!(report["dry_run"], false);
    assert_eq!(report["check"], false);
    assert_eq!(report["required_tools"], "uv, just");
    assert_eq!(report["conflicts"], 0);
    assert_eq!(report["next_steps"], serde_json::json!([]));
    assert!(
        report["actions"]
            .as_array()
            .expect("actions should be an array")
            .iter()
            .any(|action| action["action"] == "update" && action["path"] == "justfile")
    );

    let just_after = fs::read_to_string(justfile).expect("justfile should remain readable");
    assert!(!just_after.contains("BROKEN"));
    assert!(just_after.contains("forge update --path . --check"));
}

#[test]
fn update_json_reports_current_when_no_changes_are_needed() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--json",
    ]);

    let output = update.output().expect("update should run");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert_eq!(report["blueprint_version"], "0.1.0");
    assert_eq!(report["status_code"], "current");
    assert_eq!(report["changes"], 0);
    assert_eq!(report["conflicts"], 0);
    assert_eq!(report["next_steps"], serde_json::json!([]));
}

#[test]
fn update_check_succeeds_when_managed_infra_is_current() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--check",
    ]);
    update
        .assert()
        .success()
        .stdout(contains("Managed changes preview"))
        .stdout(contains("changes: 0"))
        .stdout(contains("infrastructure:"))
        .stdout(contains("managed infrastructure is current"));
}

#[test]
fn update_check_json_reports_current_when_managed_infra_is_current() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--check",
        "--json",
    ]);

    let output = update.output().expect("update should run");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert_eq!(report["blueprint_version"], "0.1.0");
    assert_eq!(report["status_code"], "current");
    assert_eq!(report["check"], true);
    assert_eq!(report["dry_run"], false);
    assert_eq!(report["changes"], 0);
    assert_eq!(report["conflicts"], 0);
    assert_eq!(report["next_steps"], serde_json::json!([]));
}

#[test]
fn update_check_fails_when_managed_infra_has_drift_without_writing() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let justfile = project_path.join("justfile");
    fs::write(&justfile, "BROKEN\n").expect("should write broken justfile");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--check",
    ]);
    update
        .assert()
        .failure()
        .stdout(contains("update  justfile"))
        .stdout(contains("blueprint: python-library"))
        .stdout(contains("infrastructure:"))
        .stdout(contains("Next steps"))
        .stdout(contains(format!(
            "forge update --path {} --yes",
            canonical_display(&project_path)
        )))
        .stderr(contains("managed infrastructure is out of date"))
        .stderr(contains("error_code: FORGE_E_CONFLICT"));

    let just_after = fs::read_to_string(justfile).expect("justfile should remain readable");
    assert_eq!(just_after, "BROKEN\n");
}

#[test]
fn update_check_json_reports_changes_before_failing() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let justfile = project_path.join("justfile");
    fs::write(&justfile, "BROKEN\n").expect("should write broken justfile");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--check",
        "--json",
    ]);

    let output = update.output().expect("update should run");
    assert!(!output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(!stdout.contains("Managed changes preview"));
    assert!(!stdout.contains("Next steps"));
    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert_eq!(report["blueprint_version"], "0.1.0");
    assert_eq!(report["status_code"], "out_of_date");
    assert_eq!(report["check"], true);
    assert_eq!(report["dry_run"], false);
    assert!(
        report["changes"]
            .as_u64()
            .expect("changes should be numeric")
            > 0
    );
    assert!(
        report["actions"]
            .as_array()
            .expect("actions should be an array")
            .iter()
            .any(|action| action["action"] == "update" && action["path"] == "justfile")
    );
    assert_eq!(
        report["next_steps"],
        serde_json::json!([format!(
            "forge update --path {} --yes",
            canonical_display(&project_path)
        )])
    );

    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("managed infrastructure is out of date"));
    assert!(stderr.contains("error_code: FORGE_E_CONFLICT"));

    let just_after = fs::read_to_string(justfile).expect("justfile should remain readable");
    assert_eq!(just_after, "BROKEN\n");
}

#[test]
fn update_check_json_quotes_next_step_path_with_spaces() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops tools");
    generate_project(&project_path);

    let justfile = project_path.join("justfile");
    fs::write(&justfile, "BROKEN\n").expect("should write broken justfile");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--check",
        "--json",
    ]);

    let output = update.output().expect("update should run");
    assert!(!output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(!stdout.contains("Managed changes preview"));
    assert!(!stdout.contains("Next steps"));
    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert_eq!(
        report["next_steps"],
        serde_json::json!([format!(
            "forge update --path '{}' --yes",
            canonical_display(&project_path)
        )])
    );
}

#[test]
fn update_set_can_enable_prettier_component() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--set",
        "prettier=true",
    ]);
    update
        .assert()
        .success()
        .stdout(contains("create  .prettierignore"))
        .stdout(contains("create  .prettierrc.json"))
        .stdout(contains("Next steps"))
        .stdout(contains("uv lock"));

    let pyproject =
        fs::read_to_string(project_path.join("pyproject.toml")).expect("pyproject should exist");
    assert!(pyproject.contains("prettier = true"));

    let precommit = fs::read_to_string(project_path.join(".pre-commit-config.yaml"))
        .expect("pre-commit config should exist");
    assert!(precommit.contains("id: prettier"));
    assert!(precommit.contains("npx --yes prettier@3.8.3 --check --ignore-unknown"));
    assert!(!precommit.contains("npx --yes prettier@3.8.3 --write --ignore-unknown"));
    let justfile =
        fs::read_to_string(project_path.join("justfile")).expect("justfile should exist");
    assert!(justfile.contains("npx --yes prettier@3.8.3 --write --ignore-unknown ."));
    assert!(project_path.join(".prettierrc.json").exists());
    assert!(project_path.join(".prettierignore").exists());
}

#[test]
fn update_set_can_enable_editorconfig_component() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--set",
        "editorconfig=true",
    ]);
    update
        .assert()
        .success()
        .stdout(contains("create  .editorconfig"))
        .stdout(contains("Next steps"))
        .stdout(contains("uv lock"));

    let pyproject =
        fs::read_to_string(project_path.join("pyproject.toml")).expect("pyproject should exist");
    assert!(pyproject.contains("editorconfig = true"));
    assert!(project_path.join(".editorconfig").exists());
}

#[test]
fn update_set_can_enable_markdownlint_component() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--set",
        "markdownlint=true",
    ]);
    update
        .assert()
        .success()
        .stdout(contains("create  .markdownlint.jsonc"))
        .stdout(contains("required tools: uv, just, npx"))
        .stdout(contains("Next steps"))
        .stdout(contains("uv lock"));

    let pyproject =
        fs::read_to_string(project_path.join("pyproject.toml")).expect("pyproject should exist");
    assert!(pyproject.contains("markdownlint = true"));
    assert!(project_path.join(".markdownlint.jsonc").exists());

    let precommit = fs::read_to_string(project_path.join(".pre-commit-config.yaml"))
        .expect("pre-commit config should exist");
    assert!(precommit.contains("id: markdownlint"));
    assert!(precommit.contains("entry: npx --yes markdownlint-cli2@0.18.1"));

    let justfile =
        fs::read_to_string(project_path.join("justfile")).expect("justfile should exist");
    assert!(justfile.contains("npx --yes markdownlint-cli2@0.18.1 --fix \"**/*.md\""));
}

#[test]
fn update_set_preserves_pyproject_comments_and_unmanaged_formatting() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let pyproject_path = project_path.join("pyproject.toml");
    let pyproject = fs::read_to_string(&pyproject_path).expect("pyproject should exist");
    let pyproject = pyproject
        .replace("[project]\n", "# user project comment\n[project]\n")
        .replace(
            "prettier = false\n",
            "prettier = false # keep local option note\n",
        );
    fs::write(&pyproject_path, pyproject).expect("pyproject should be writable");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--set",
        "prettier=true",
    ]);
    update.assert().success();

    let pyproject = fs::read_to_string(pyproject_path).expect("pyproject should exist");
    assert!(pyproject.contains("# user project comment"));
    assert!(pyproject.contains("prettier = true # keep local option note"));
}

#[test]
fn update_set_dry_run_previews_option_change_without_writing() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--set",
        "prettier=true",
        "--dry-run",
    ]);
    update
        .assert()
        .success()
        .stdout(contains("Managed changes preview"))
        .stdout(contains("create  .prettierignore"))
        .stdout(contains("create  .prettierrc.json"))
        .stdout(contains("Next steps"))
        .stdout(contains(format!(
            "forge update --path {} --set prettier=true --yes",
            canonical_display(&project_path)
        )))
        .stdout(contains("uv lock"))
        .stdout(contains("dry run complete; no files changed"));

    let pyproject =
        fs::read_to_string(project_path.join("pyproject.toml")).expect("pyproject should exist");
    assert!(pyproject.contains("prettier = false"));
    assert!(!project_path.join(".prettierrc.json").exists());
    assert!(!project_path.join(".prettierignore").exists());
}

#[test]
fn update_set_dry_run_diff_shows_created_files_with_dev_null_headers() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--set",
        "prettier=true",
        "--dry-run",
        "--diff",
    ]);
    update
        .assert()
        .success()
        .stdout(contains("--- /dev/null"))
        .stdout(contains("+++ b/.prettierrc.json"))
        .stdout(contains("@@ -0,0 +1,"))
        .stdout(contains("dry run complete; no files changed"));

    assert!(!project_path.join(".prettierrc.json").exists());
    assert!(!project_path.join(".prettierignore").exists());
}

#[test]
fn update_set_check_reports_option_change_without_writing() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--set",
        "prettier=true",
        "--check",
    ]);
    update
        .assert()
        .failure()
        .stdout(contains("Managed changes preview"))
        .stdout(contains("create  .prettierignore"))
        .stdout(contains("create  .prettierrc.json"))
        .stderr(contains("managed infrastructure is out of date"));

    let pyproject =
        fs::read_to_string(project_path.join("pyproject.toml")).expect("pyproject should exist");
    assert!(pyproject.contains("prettier = false"));
    assert!(!project_path.join(".prettierrc.json").exists());
    assert!(!project_path.join(".prettierignore").exists());
}

#[test]
fn update_set_dry_run_json_reports_option_change_without_writing() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--set",
        "prettier=true",
        "--dry-run",
        "--json",
    ]);

    let output = update.output().expect("update should run");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert_eq!(report["dry_run"], true);
    assert_eq!(report["required_tools"], "uv, just, npx");
    assert!(
        report["options"]
            .as_array()
            .expect("options should be an array")
            .iter()
            .any(|option| option["name"] == "prettier" && option["enabled"] == true)
    );
    assert!(
        report["actions"]
            .as_array()
            .expect("actions should be an array")
            .iter()
            .any(|action| action["action"] == "create" && action["path"] == ".prettierrc.json")
    );
    assert_eq!(
        report["next_steps"],
        serde_json::json!([
            format!(
                "forge update --path {} --set prettier=true --yes",
                canonical_display(&project_path)
            ),
            "uv lock"
        ])
    );

    let pyproject =
        fs::read_to_string(project_path.join("pyproject.toml")).expect("pyproject should exist");
    assert!(pyproject.contains("prettier = false"));
    assert!(!project_path.join(".prettierrc.json").exists());
    assert!(!project_path.join(".prettierignore").exists());
}

#[test]
fn update_set_can_disable_prettier_component_without_leaving_files() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");

    let mut new_project = Command::cargo_bin("forge").expect("forge binary should build");
    new_project.args([
        "new",
        "--blueprint",
        "python-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "ops-tools",
        "--package-name",
        "ops_tools",
        "--description",
        "Ops toolchain",
        "--author-name",
        "Grace Hopper",
        "--author-email",
        "grace@example.com",
        "--license",
        "MIT",
        "--python-min",
        "3.12",
        "--prettier",
        "--yes",
    ]);
    new_project.assert().success();

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--set",
        "prettier=false",
    ]);
    update
        .assert()
        .success()
        .stdout(contains("remove  .prettierignore"))
        .stdout(contains("remove  .prettierrc.json"));

    let pyproject =
        fs::read_to_string(project_path.join("pyproject.toml")).expect("pyproject should exist");
    assert!(pyproject.contains("prettier = false"));

    let precommit = fs::read_to_string(project_path.join(".pre-commit-config.yaml"))
        .expect("pre-commit config should exist");
    assert!(!precommit.contains("id: prettier"));
    assert!(!project_path.join(".prettierrc.json").exists());
    assert!(!project_path.join(".prettierignore").exists());
}

#[test]
fn update_set_can_disable_markdownlint_component_without_leaving_files() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project_with_markdownlint(&project_path);

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--set",
        "markdownlint=false",
    ]);
    update
        .assert()
        .success()
        .stdout(contains("remove  .markdownlint.jsonc"))
        .stdout(contains("required tools: uv, just"));

    let pyproject =
        fs::read_to_string(project_path.join("pyproject.toml")).expect("pyproject should exist");
    assert!(pyproject.contains("markdownlint = false"));

    let precommit = fs::read_to_string(project_path.join(".pre-commit-config.yaml"))
        .expect("pre-commit config should exist");
    assert!(!precommit.contains("id: markdownlint"));

    let justfile =
        fs::read_to_string(project_path.join("justfile")).expect("justfile should exist");
    assert!(!justfile.contains("npx --yes markdownlint-cli2@0.18.1 --fix \"**/*.md\""));
    assert!(!project_path.join(".markdownlint.jsonc").exists());
}

#[test]
fn update_set_markdownlint_disable_dry_run_previews_option_change_without_writing() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project_with_markdownlint(&project_path);

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--set",
        "markdownlint=false",
        "--dry-run",
    ]);
    update
        .assert()
        .success()
        .stdout(contains("Managed changes preview"))
        .stdout(contains("remove  .markdownlint.jsonc"))
        .stdout(contains("Next steps"))
        .stdout(contains(format!(
            "forge update --path {} --set markdownlint=false --yes",
            canonical_display(&project_path)
        )))
        .stdout(contains("uv lock"))
        .stdout(contains("dry run complete; no files changed"));

    let pyproject =
        fs::read_to_string(project_path.join("pyproject.toml")).expect("pyproject should exist");
    assert!(pyproject.contains("markdownlint = true"));
    assert!(project_path.join(".markdownlint.jsonc").exists());
}

#[test]
fn update_set_markdownlint_disable_check_reports_option_change_without_writing() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project_with_markdownlint(&project_path);

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--set",
        "markdownlint=false",
        "--check",
    ]);
    update
        .assert()
        .failure()
        .stdout(contains("Managed changes preview"))
        .stdout(contains("remove  .markdownlint.jsonc"))
        .stderr(contains("managed infrastructure is out of date"));

    let pyproject =
        fs::read_to_string(project_path.join("pyproject.toml")).expect("pyproject should exist");
    assert!(pyproject.contains("markdownlint = true"));
    assert!(project_path.join(".markdownlint.jsonc").exists());
}

#[test]
fn update_set_markdownlint_disable_dry_run_json_reports_option_change_without_writing() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project_with_markdownlint(&project_path);

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--set",
        "markdownlint=false",
        "--dry-run",
        "--json",
    ]);

    let output = update.output().expect("update should run");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert_eq!(report["dry_run"], true);
    assert!(
        report["options"]
            .as_array()
            .expect("options should be an array")
            .iter()
            .any(|option| option["name"] == "markdownlint" && option["enabled"] == false)
    );
    assert!(
        report["actions"]
            .as_array()
            .expect("actions should be an array")
            .iter()
            .any(|action| action["action"] == "remove" && action["path"] == ".markdownlint.jsonc")
    );
    assert_eq!(
        report["next_steps"],
        serde_json::json!([
            format!(
                "forge update --path {} --set markdownlint=false --yes",
                canonical_display(&project_path)
            ),
            "uv lock"
        ])
    );

    let pyproject =
        fs::read_to_string(project_path.join("pyproject.toml")).expect("pyproject should exist");
    assert!(pyproject.contains("markdownlint = true"));
    assert!(project_path.join(".markdownlint.jsonc").exists());
}

#[test]
fn update_set_can_enable_python_pypi_publish_workflow() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--set",
        "pypi-publish=true",
    ]);
    update
        .assert()
        .success()
        .stdout(contains("create  .github/workflows/publish-pypi.yaml"));

    let pyproject =
        fs::read_to_string(project_path.join("pyproject.toml")).expect("pyproject should exist");
    assert!(pyproject.contains("pypi-publish = true"));

    let publish_workflow =
        fs::read_to_string(project_path.join(".github/workflows/publish-pypi.yaml"))
            .expect("publish workflow should exist");
    assert!(publish_workflow.contains(
        "# Register this workflow as a trusted publisher in PyPI before uncommenting the publish step."
    ));
    assert!(publish_workflow.contains("# - name: Publish package distributions to PyPI"));
    assert!(publish_workflow.contains("#   uses: pypa/gh-action-pypi-publish@release/v1"));
    assert!(publish_workflow.contains(
        "concurrency:\n  group: ${{ github.workflow }}-${{ github.event.release.id }}\n  cancel-in-progress: false\n\njobs:"
    ));
    assert!(publish_workflow.contains("    environment:\n      name: pypi\n"));
    assert!(publish_workflow.contains("      url: https://pypi.org/p/<your-pypi-project-name>\n"));
    assert!(
        publish_workflow
            .contains("    permissions:\n      id-token: write\n      contents: read\n")
    );
    assert!(!publish_workflow.contains("\npermissions:\n  id-token: write\n"));
}

#[test]
fn update_set_rejects_duplicate_canonical_options() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--set",
        "pypi-publish=true",
        "--set",
        "pypi-publish=false",
    ]);
    update
        .assert()
        .failure()
        .stderr(contains("option 'pypi-publish' was set more than once"));
}

#[test]
fn update_set_can_disable_python_pypi_publish_workflow() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");

    let mut new_project = Command::cargo_bin("forge").expect("forge binary should build");
    new_project.args([
        "new",
        "--blueprint",
        "python-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "ops-tools",
        "--package-name",
        "ops_tools",
        "--description",
        "Ops toolchain",
        "--author-name",
        "Grace Hopper",
        "--author-email",
        "grace@example.com",
        "--license",
        "MIT",
        "--python-min",
        "3.12",
        "--pypi-publish",
        "true",
        "--yes",
    ]);
    new_project.assert().success();

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--set",
        "pypi-publish=false",
    ]);
    update
        .assert()
        .success()
        .stdout(contains("remove  .github/workflows/publish-pypi.yaml"));

    let pyproject =
        fs::read_to_string(project_path.join("pyproject.toml")).expect("pyproject should exist");
    assert!(pyproject.contains("pypi-publish = false"));
    assert!(
        !project_path
            .join(".github/workflows/publish-pypi.yaml")
            .exists()
    );
}

#[test]
fn update_set_rejects_options_not_supported_by_detected_blueprint() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-rs");

    let mut new_project = Command::cargo_bin("forge").expect("forge binary should build");
    new_project.args([
        "new",
        "--blueprint",
        "rust-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-rs",
        "--description",
        "Grid utilities for Rust",
        "--author-name",
        "Ferris Engineer",
        "--author-email",
        "ferris@example.com",
        "--yes",
    ]);
    new_project.assert().success();

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--set",
        "codecov=true",
    ]);
    update.assert().failure().stderr(contains(
        "option 'codecov' is not supported by rust-library",
    ));

    let pyproject =
        fs::read_to_string(project_path.join("pyproject.toml")).expect("pyproject should exist");
    assert!(!pyproject.contains("codecov = true"));
}

#[test]
fn update_set_rejects_duplicate_option_overrides() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--set",
        "prettier=true",
        "--set",
        "prettier=false",
    ]);
    update
        .assert()
        .failure()
        .stderr(contains("option 'prettier' was set more than once"));

    let pyproject =
        fs::read_to_string(project_path.join("pyproject.toml")).expect("pyproject should exist");
    assert!(pyproject.contains("prettier = false"));
    assert!(!project_path.join(".prettierrc.json").exists());
    assert!(!project_path.join(".prettierignore").exists());
}

#[test]
fn update_set_rejects_whitespace_padded_option_overrides() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--set",
        " prettier=true",
    ]);
    update
        .assert()
        .failure()
        .stderr(contains("invalid option override ' prettier=true'"));

    let pyproject =
        fs::read_to_string(project_path.join("pyproject.toml")).expect("pyproject should exist");
    assert!(pyproject.contains("prettier = false"));
}

#[test]
fn update_set_rejects_cli_style_option_names() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--set=--prettier=true",
    ]);
    update
        .assert()
        .failure()
        .stderr(contains("unsupported managed option '--prettier'"));
}

#[test]
fn update_set_json_rejects_cli_style_option_names_with_empty_stdout() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--set=--pypi-publish=true",
        "--json",
    ]);

    let output = update.output().expect("update should run");
    assert!(!output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.is_empty());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("unsupported managed option '--pypi-publish'"));
}

#[test]
fn update_cleans_prettier_files_when_component_is_disabled() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    fs::write(project_path.join(".prettierrc.json"), "{}\n")
        .expect("stale prettier config should be writable");
    fs::write(project_path.join(".prettierignore"), "dist/\n")
        .expect("stale prettier ignore should be writable");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
    ]);
    update.assert().success();

    assert!(!project_path.join(".prettierrc.json").exists());
    assert!(!project_path.join(".prettierignore").exists());
}

#[test]
fn update_cleans_markdownlint_files_when_component_is_disabled() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    fs::write(
        project_path.join(".markdownlint.jsonc"),
        "{\n  \"default\": true\n}\n",
    )
    .expect("stale markdownlint config should be writable");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
    ]);
    update.assert().success();

    assert!(!project_path.join(".markdownlint.jsonc").exists());
}

#[test]
fn update_reports_optional_cleanup_directory_conflicts_before_removing_files() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    fs::create_dir(project_path.join(".prettierrc.json"))
        .expect("stale prettier config directory should create");
    fs::write(project_path.join(".prettierignore"), "dist/\n")
        .expect("stale prettier ignore should be writable");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
    ]);
    update
        .assert()
        .failure()
        .stdout(contains(
            "conflict .prettierrc.json (managed path is a directory)",
        ))
        .stderr(contains("managed infrastructure has conflicts"));

    assert!(project_path.join(".prettierrc.json").is_dir());
    assert!(project_path.join(".prettierignore").exists());
}

#[test]
fn update_dry_run_previews_optional_file_removal_without_removing() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_project(&project_path);

    fs::write(project_path.join(".prettierrc.json"), "{}\n")
        .expect("stale prettier config should be writable");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--dry-run",
    ]);
    update
        .assert()
        .success()
        .stdout(contains("remove  .prettierrc.json"));

    assert!(project_path.join(".prettierrc.json").exists());
}

#[test]
fn update_refreshes_language_agnostic_infra_project() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo-infra");

    let mut new_project = Command::cargo_bin("forge").expect("forge binary should build");
    new_project.args([
        "new",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--description",
        "Shared repository infrastructure",
        "--yes",
    ]);
    new_project.assert().success();

    let justfile = project_path.join("justfile");
    fs::write(&justfile, "BROKEN\n").expect("should write broken justfile");
    let ci_workflow = project_path.join(".github/workflows/ci.yaml");
    fs::write(&ci_workflow, "BROKEN\n").expect("should write broken CI workflow");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
    ]);
    update
        .assert()
        .success()
        .stdout(contains("blueprint: any-project"))
        .stdout(contains("required tools: uv, just"));

    let just_after = fs::read_to_string(justfile).expect("justfile should remain readable");
    assert!(!just_after.contains("BROKEN"));
    assert!(just_after.contains("uv run --locked prek run --all-files"));
    assert!(just_after.contains("uv lock --check"));
    assert!(just_after.contains("forge update --path . --check"));
    let ci_after = fs::read_to_string(ci_workflow).expect("CI workflow should remain readable");
    assert!(ci_after.contains("forge update --path . --check"));
    assert!(project_path.join("docs/package.json").exists());
    assert!(
        project_path
            .join("docs/src/content/docs/index.mdx")
            .exists()
    );
}

#[test]
fn update_set_can_enable_prettier_for_language_agnostic_project() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo-infra");

    let mut new_project = Command::cargo_bin("forge").expect("forge binary should build");
    new_project.args([
        "new",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--description",
        "Shared repository infrastructure",
        "--yes",
    ]);
    new_project.assert().success();

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--set",
        "prettier=true",
    ]);
    update
        .assert()
        .success()
        .stdout(contains("create  .prettierignore"))
        .stdout(contains("create  .prettierrc.json"));

    let pyproject =
        fs::read_to_string(project_path.join("pyproject.toml")).expect("pyproject should exist");
    assert!(pyproject.contains("prettier = true"));

    let precommit = fs::read_to_string(project_path.join(".pre-commit-config.yaml"))
        .expect("pre-commit config should exist");
    assert!(precommit.contains("id: prettier"));
    assert!(precommit.contains("npx --yes prettier@3.8.3 --check --ignore-unknown"));
    let justfile =
        fs::read_to_string(project_path.join("justfile")).expect("justfile should exist");
    assert!(justfile.contains("npx --yes prettier@3.8.3 --write --ignore-unknown ."));
}

#[test]
fn update_set_can_enable_markdownlint_for_language_agnostic_project() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo-infra");

    let mut new_project = Command::cargo_bin("forge").expect("forge binary should build");
    new_project.args([
        "new",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--description",
        "Shared repository infrastructure",
        "--yes",
    ]);
    new_project.assert().success();

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--set",
        "markdownlint=true",
    ]);
    update
        .assert()
        .success()
        .stdout(contains("create  .markdownlint.jsonc"))
        .stdout(contains("required tools: uv, just, npx"));

    let pyproject =
        fs::read_to_string(project_path.join("pyproject.toml")).expect("pyproject should exist");
    assert!(pyproject.contains("markdownlint = true"));
    assert!(project_path.join(".markdownlint.jsonc").exists());

    let precommit = fs::read_to_string(project_path.join(".pre-commit-config.yaml"))
        .expect("pre-commit config should exist");
    assert!(precommit.contains("id: markdownlint"));
    assert!(precommit.contains("entry: npx --yes markdownlint-cli2@0.18.1"));

    let justfile =
        fs::read_to_string(project_path.join("justfile")).expect("justfile should exist");
    assert!(justfile.contains("npx --yes markdownlint-cli2@0.18.1 --fix \"**/*.md\""));
}

#[test]
fn update_set_can_disable_markdownlint_for_language_agnostic_project() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo-infra");

    let mut new_project = Command::cargo_bin("forge").expect("forge binary should build");
    new_project.args([
        "new",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--description",
        "Shared repository infrastructure",
        "--markdownlint",
        "--yes",
    ]);
    new_project.assert().success();

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--set",
        "markdownlint=false",
    ]);
    update
        .assert()
        .success()
        .stdout(contains("remove  .markdownlint.jsonc"))
        .stdout(contains("required tools: uv, just"));

    let pyproject =
        fs::read_to_string(project_path.join("pyproject.toml")).expect("pyproject should exist");
    assert!(pyproject.contains("markdownlint = false"));
    assert!(!project_path.join(".markdownlint.jsonc").exists());

    let precommit = fs::read_to_string(project_path.join(".pre-commit-config.yaml"))
        .expect("pre-commit config should exist");
    assert!(!precommit.contains("id: markdownlint"));

    let justfile =
        fs::read_to_string(project_path.join("justfile")).expect("justfile should exist");
    assert!(!justfile.contains("npx --yes markdownlint-cli2@0.18.1 --fix \"**/*.md\""));
}

#[test]
fn update_refreshes_rust_library_managed_infra_without_touching_source() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-rs");

    let mut new_project = Command::cargo_bin("forge").expect("forge binary should build");
    new_project.args([
        "new",
        "--blueprint",
        "rust-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-rs",
        "--description",
        "Grid utilities for Rust",
        "--author-name",
        "Ferris Engineer",
        "--author-email",
        "ferris@example.com",
        "--yes",
    ]);
    new_project.assert().success();

    let source = project_path.join("src/lib.rs");
    fs::write(&source, "pub fn custom() -> bool {\n    true\n}\n")
        .expect("source should be writable");

    let justfile = project_path.join("justfile");
    fs::write(&justfile, "BROKEN\n").expect("justfile should be writable");
    let ci_workflow = project_path.join(".github/workflows/ci.yaml");
    fs::write(&ci_workflow, "BROKEN\n").expect("CI workflow should be writable");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
    ]);
    update
        .assert()
        .success()
        .stdout(contains("blueprint: rust-library"))
        .stdout(contains("required tools: cargo, uv, just"));

    let source_after = fs::read_to_string(source).expect("source should be readable");
    assert!(source_after.contains("custom"));

    let just_after = fs::read_to_string(justfile).expect("justfile should remain readable");
    assert!(!just_after.contains("BROKEN"));
    assert!(just_after.contains("cargo fmt --all --check"));
    assert!(just_after.contains("uv lock --check"));
    assert!(
        just_after.contains("cargo clippy --workspace --all-targets --all-features -- -D warnings")
    );
    assert!(just_after.contains("forge update --path . --check"));
    let ci_after = fs::read_to_string(ci_workflow).expect("CI workflow should remain readable");
    assert!(ci_after.contains("cargo fmt --all --check"));
    assert!(ci_after.contains("uv lock --check"));
    assert!(
        ci_after.contains("cargo clippy --workspace --all-targets --all-features -- -D warnings")
    );
    assert!(ci_after.contains("forge update --path . --check"));
    assert!(project_path.join("docs/package.json").exists());
    assert!(
        project_path
            .join("docs/src/content/docs/index.mdx")
            .exists()
    );
}

#[test]
fn update_set_can_enable_prettier_for_rust_library_project() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-rs");

    let mut new_project = Command::cargo_bin("forge").expect("forge binary should build");
    new_project.args([
        "new",
        "--blueprint",
        "rust-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-rs",
        "--description",
        "Grid utilities for Rust",
        "--author-name",
        "Ferris Engineer",
        "--author-email",
        "ferris@example.com",
        "--yes",
    ]);
    new_project.assert().success();

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--set",
        "prettier=true",
    ]);
    update
        .assert()
        .success()
        .stdout(contains("create  .prettierignore"))
        .stdout(contains("create  .prettierrc.json"));

    let pyproject =
        fs::read_to_string(project_path.join("pyproject.toml")).expect("pyproject should exist");
    assert!(pyproject.contains("prettier = true"));

    let precommit = fs::read_to_string(project_path.join(".pre-commit-config.yaml"))
        .expect("pre-commit config should exist");
    assert!(precommit.contains("id: prettier"));
    assert!(precommit.contains("npx --yes prettier@3.8.3 --check --ignore-unknown"));
    assert!(precommit.contains("cargo clippy"));
    let justfile =
        fs::read_to_string(project_path.join("justfile")).expect("justfile should exist");
    assert!(justfile.contains("npx --yes prettier@3.8.3 --write --ignore-unknown ."));
}

#[test]
fn update_set_can_enable_markdownlint_for_rust_library_project() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-rs");

    let mut new_project = Command::cargo_bin("forge").expect("forge binary should build");
    new_project.args([
        "new",
        "--blueprint",
        "rust-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-rs",
        "--description",
        "Grid utilities for Rust",
        "--author-name",
        "Ferris Engineer",
        "--author-email",
        "ferris@example.com",
        "--yes",
    ]);
    new_project.assert().success();

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--set",
        "markdownlint=true",
    ]);
    update
        .assert()
        .success()
        .stdout(contains("create  .markdownlint.jsonc"))
        .stdout(contains("required tools: cargo, uv, just, npx"));

    let pyproject =
        fs::read_to_string(project_path.join("pyproject.toml")).expect("pyproject should exist");
    assert!(pyproject.contains("markdownlint = true"));
    assert!(project_path.join(".markdownlint.jsonc").exists());

    let precommit = fs::read_to_string(project_path.join(".pre-commit-config.yaml"))
        .expect("pre-commit config should exist");
    assert!(precommit.contains("id: markdownlint"));
    assert!(precommit.contains("entry: npx --yes markdownlint-cli2@0.18.1"));
    assert!(precommit.contains("cargo clippy"));

    let justfile =
        fs::read_to_string(project_path.join("justfile")).expect("justfile should exist");
    assert!(justfile.contains("npx --yes markdownlint-cli2@0.18.1 --fix \"**/*.md\""));
}

#[test]
fn update_set_can_disable_markdownlint_for_rust_library_project() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-rs");

    let mut new_project = Command::cargo_bin("forge").expect("forge binary should build");
    new_project.args([
        "new",
        "--blueprint",
        "rust-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-rs",
        "--description",
        "Grid utilities for Rust",
        "--author-name",
        "Ferris Engineer",
        "--author-email",
        "ferris@example.com",
        "--markdownlint",
        "--yes",
    ]);
    new_project.assert().success();

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--set",
        "markdownlint=false",
    ]);
    update
        .assert()
        .success()
        .stdout(contains("remove  .markdownlint.jsonc"))
        .stdout(contains("required tools: cargo, uv, just"));

    let pyproject =
        fs::read_to_string(project_path.join("pyproject.toml")).expect("pyproject should exist");
    assert!(pyproject.contains("markdownlint = false"));
    assert!(!project_path.join(".markdownlint.jsonc").exists());

    let precommit = fs::read_to_string(project_path.join(".pre-commit-config.yaml"))
        .expect("pre-commit config should exist");
    assert!(!precommit.contains("id: markdownlint"));
    assert!(precommit.contains("cargo clippy"));

    let justfile =
        fs::read_to_string(project_path.join("justfile")).expect("justfile should exist");
    assert!(!justfile.contains("npx --yes markdownlint-cli2@0.18.1 --fix \"**/*.md\""));
}

#[test]
fn update_removes_rust_docs_when_disabled_in_metadata() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-rs");

    let mut new_project = Command::cargo_bin("forge").expect("forge binary should build");
    new_project.args([
        "new",
        "--blueprint",
        "rust-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-rs",
        "--description",
        "Grid utilities for Rust",
        "--author-name",
        "Ferris Engineer",
        "--author-email",
        "ferris@example.com",
        "--docs",
        "false",
        "--yes",
    ]);
    new_project.assert().success();

    fs::create_dir_all(project_path.join("docs/src/content/docs")).expect("docs dir should create");
    fs::write(project_path.join("docs/package.json"), "BROKEN\n")
        .expect("stale docs package should write");
    fs::write(
        project_path.join("docs/src/content/docs/index.mdx"),
        "BROKEN\n",
    )
    .expect("stale docs should write");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
    ]);
    update.assert().success();

    assert!(!project_path.join("docs/package.json").exists());
    assert!(
        !project_path
            .join("docs/src/content/docs/index.mdx")
            .exists()
    );
}

#[test]
fn update_set_can_disable_rust_docs() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-rs");

    let mut new_project = Command::cargo_bin("forge").expect("forge binary should build");
    new_project.args([
        "new",
        "--blueprint",
        "rust-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-rs",
        "--description",
        "Grid utilities for Rust",
        "--author-name",
        "Ferris Engineer",
        "--author-email",
        "ferris@example.com",
        "--yes",
    ]);
    new_project.assert().success();

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--set",
        "docs=false",
    ]);
    update
        .assert()
        .success()
        .stdout(contains("remove  docs/src/content/docs/index.mdx"))
        .stdout(contains("remove  docs/package.json"));

    let pyproject =
        fs::read_to_string(project_path.join("pyproject.toml")).expect("pyproject should exist");
    assert!(pyproject.contains("docs = false"));
    assert!(!pyproject.contains("@astrojs/starlight"));
    let justfile =
        fs::read_to_string(project_path.join("justfile")).expect("justfile should exist");
    assert!(!justfile.contains("\ndocs:\n"));
    assert!(!justfile.contains("npm run dev"));
    assert!(!project_path.join("docs/package.json").exists());
    assert!(
        !project_path
            .join("docs/src/content/docs/index.mdx")
            .exists()
    );
    assert!(project_path.join("docs").exists());
}

#[test]
fn update_set_dry_run_reports_empty_docs_directory_removal() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-rs");

    let mut new_project = Command::cargo_bin("forge").expect("forge binary should build");
    new_project.args([
        "new",
        "--blueprint",
        "rust-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-rs",
        "--description",
        "Grid utilities for Rust",
        "--author-name",
        "Ferris Engineer",
        "--author-email",
        "ferris@example.com",
        "--yes",
    ]);
    new_project.assert().success();

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--set",
        "docs=false",
        "--dry-run",
        "--json",
    ]);

    let output = update.output().expect("update should run");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert!(
        report["actions"]
            .as_array()
            .expect("actions should be an array")
            .iter()
            .any(|action| {
                action["action"] == "remove" && action["path"] == "docs/src/content/docs/index.mdx"
            })
    );

    assert!(
        project_path
            .join("docs/src/content/docs/index.mdx")
            .exists()
    );
    assert!(project_path.join("docs").exists());
}

#[test]
fn update_set_dry_run_preserves_nonempty_docs_directory_in_report() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-rs");

    let mut new_project = Command::cargo_bin("forge").expect("forge binary should build");
    new_project.args([
        "new",
        "--blueprint",
        "rust-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-rs",
        "--description",
        "Grid utilities for Rust",
        "--author-name",
        "Ferris Engineer",
        "--author-email",
        "ferris@example.com",
        "--yes",
    ]);
    new_project.assert().success();

    fs::write(project_path.join("docs/guide.md"), "# User guide\n")
        .expect("user docs should be writable");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--set",
        "docs=false",
        "--dry-run",
        "--json",
    ]);

    let output = update.output().expect("update should run");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert!(
        !report["actions"]
            .as_array()
            .expect("actions should be an array")
            .iter()
            .any(|action| action["action"] == "remove" && action["path"] == "docs")
    );

    assert!(
        project_path
            .join("docs/src/content/docs/index.mdx")
            .exists()
    );
    assert!(project_path.join("docs/guide.md").exists());
    assert!(project_path.join("docs").exists());
}

#[test]
fn update_set_can_enable_rust_docs() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-rs");

    let mut new_project = Command::cargo_bin("forge").expect("forge binary should build");
    new_project.args([
        "new",
        "--blueprint",
        "rust-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-rs",
        "--description",
        "Grid utilities for Rust",
        "--author-name",
        "Ferris Engineer",
        "--author-email",
        "ferris@example.com",
        "--docs",
        "false",
        "--yes",
    ]);
    new_project.assert().success();

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--set",
        "docs=true",
    ]);
    update
        .assert()
        .success()
        .stdout(contains("create  docs/src/content/docs/index.mdx"))
        .stdout(contains("create  docs/package.json"));

    let pyproject =
        fs::read_to_string(project_path.join("pyproject.toml")).expect("pyproject should exist");
    assert!(pyproject.contains("docs = true"));
    assert!(!pyproject.contains("mkdocs-material"));
    let justfile =
        fs::read_to_string(project_path.join("justfile")).expect("justfile should exist");
    assert!(justfile.contains("\ndocs:\n"));
    assert!(justfile.contains("cd docs && npm run dev"));
    assert!(project_path.join("docs/package.json").exists());
    assert!(
        project_path
            .join("docs/src/content/docs/index.mdx")
            .exists()
    );
}

#[test]
fn update_set_preserves_docs_directory_when_user_files_remain() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-rs");

    let mut new_project = Command::cargo_bin("forge").expect("forge binary should build");
    new_project.args([
        "new",
        "--blueprint",
        "rust-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-rs",
        "--description",
        "Grid utilities for Rust",
        "--author-name",
        "Ferris Engineer",
        "--author-email",
        "ferris@example.com",
        "--yes",
    ]);
    new_project.assert().success();

    fs::write(project_path.join("docs/guide.md"), "# User guide\n")
        .expect("user docs should be writable");

    let mut update = Command::cargo_bin("forge").expect("forge binary should build");
    update.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--set",
        "docs=false",
    ]);
    update.assert().success();

    assert!(
        !project_path
            .join("docs/src/content/docs/index.mdx")
            .exists()
    );
    assert!(project_path.join("docs/guide.md").exists());
    assert!(project_path.join("docs").exists());
}
