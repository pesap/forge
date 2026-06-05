use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use std::fs;
use tempfile::TempDir;

fn generate_python_project(project_path: &std::path::Path) {
    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "init",
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
        "--yes",
    ]);
    cmd.assert().success();
}

fn forge_section(pyproject: &str) -> &str {
    pyproject
        .split("[tool.forge]")
        .nth(1)
        .expect("forge metadata should exist")
        .split("\n[")
        .next()
        .expect("forge section should be bounded")
}

fn create_existing_python_repo_with_pyproject(project_path: &std::path::Path) {
    fs::create_dir_all(project_path.join("src/ops_tools")).expect("source tree should create");
    fs::write(
        project_path.join("pyproject.toml"),
        r#"[project]
name = "ops-tools"
version = "2.3.4"
description = "Existing ops toolchain"
requires-python = ">=3.12"
dependencies = ["click>=8"]

[dependency-groups]
dev = [
    "prek>=0.3.1",
    "pytest>=9.0.2",
    "ruff>=0.15.0",
    "ty>=0.0.15",
]
"#,
    )
    .expect("existing pyproject should be writable");
}

fn assert_external_pyproject_adopted(project_path: &std::path::Path) {
    let pyproject =
        fs::read_to_string(project_path.join("pyproject.toml")).expect("pyproject should exist");
    assert!(pyproject.contains("version = \"2.3.4\""));
    assert!(pyproject.contains("dependencies = [\"click>=8\"]"));
    assert!(pyproject.contains(
        "[build-system]\nrequires = [\"uv_build~=0.11.7\"]\nbuild-backend = \"uv_build\""
    ));
    assert_eq!(
        forge_section(&pyproject).trim(),
        "blueprint = \"python-library>=0.1.0\"\ngitignore_profile = [\"python\", \"macos\", \"visualstudiocode\", \"jetbrains\", \"node\"]\nignore = [\"editorconfig\"]"
    );
    assert!(!pyproject.contains("forge = ["));
    assert!(!pyproject.contains("\nbuild = ["));
    assert!(pyproject.contains("code-quality = ["));
    assert!(!pyproject.contains("linting = ["));
    assert!(pyproject.contains("test = ["));
    assert!(pyproject.contains("\"prek~=0.4.1\""));
    assert!(pyproject.contains("\"pytest~=9.0.0\""));
    assert!(pyproject.contains("\"ruff~=0.14.0\""));
    assert!(pyproject.contains("\"ty~=0.0.1\""));
    assert!(pyproject.contains("line-length = 110"));
    assert!(pyproject.contains("[tool.ty.rules]\nall = \"error\""));
    assert!(
        pyproject
            .find("[tool.ruff.lint]")
            .expect("ruff lint section should exist")
            < pyproject
                .find("[tool.ty.rules]")
                .expect("ty rules section should exist")
    );
    assert!(pyproject.contains("strict = true"));
    assert!(pyproject.contains("cache_dir = \"$XDG_CACHE_HOME/pytest/ops-tools\""));
    assert!(pyproject.contains(
        "[tool.pytest_env]\nXDG_CACHE_HOME = { value = \"{HOME}/.cache\", transform = true, skip_if_set = true }"
    ));
    assert!(!pyproject.contains("Library/Caches/pytest"));
    assert!(!pyproject.contains("{ include-group = \"build\" }"));
    assert!(pyproject.contains("{ include-group = \"code-quality\" }"));
    assert!(pyproject.contains("{ include-group = \"test\" }"));
}

#[test]
fn init_adds_managed_infrastructure_to_existing_python_repo() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    fs::create_dir_all(project_path.join("src/ops_tools")).expect("source tree should create");
    fs::write(
        project_path.join("src/ops_tools/core.py"),
        "def existing() -> str:\n    return \"kept\"\n",
    )
    .expect("existing source should be writable");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "init",
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

    cmd.assert()
        .success()
        .stdout(contains("Repository initialized"))
        .stdout(contains("[ok] managed infrastructure added"))
        .stdout(contains("blueprint: python-library"))
        .stdout(contains("detected package: ops_tools"))
        .stdout(contains("infrastructure:"))
        .stdout(contains("prek hooks"))
        .stdout(contains(format!("cd {}", project_path.display())))
        .stdout(contains("uv sync --all-groups"))
        .stdout(contains("just verify"))
        .stdout(contains("forge sync --path ."));

    let existing_source = fs::read_to_string(project_path.join("src/ops_tools/core.py"))
        .expect("source should exist");
    assert!(existing_source.contains("return \"kept\""));

    let pyproject =
        fs::read_to_string(project_path.join("pyproject.toml")).expect("pyproject should exist");
    assert!(pyproject.contains("blueprint = \"python-library>=0.1.0\""));
    assert!(pyproject.contains("requires-python = \">=3.12,<3.15\""));
    assert!(!pyproject.contains("python_min = \"3.12\""));
    assert!(project_path.join(".github/workflows/ci.yaml").exists());
    assert!(project_path.join("justfile").exists());
    assert_eq!(
        fs::read_link(project_path.join("CLAUDE.md")).expect("CLAUDE.md should be a symlink"),
        std::path::PathBuf::from("AGENTS.md")
    );

    let mut check = Command::cargo_bin("forge").expect("forge binary should build");
    check.args([
        "sync",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--check",
    ]);
    check
        .assert()
        .success()
        .stdout(contains("managed infrastructure is current"));
}

#[test]
fn init_infers_python_package_from_existing_src_layout() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("sandbox");
    fs::create_dir_all(project_path.join("src/test")).expect("source package should create");
    fs::write(project_path.join("src/test/__init__.py"), "").expect("package init should write");
    fs::write(
        project_path.join("pyproject.toml"),
        r#"[project]
name = "sandbox"
version = "0.1.0"
description = "Existing sandbox"
requires-python = ">=3.12"
"#,
    )
    .expect("existing pyproject should be writable");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "init",
        "--blueprint",
        "python-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--yes",
    ]);

    cmd.assert()
        .success()
        .stdout(contains("detected package: test"));

    let justfile =
        fs::read_to_string(project_path.join("justfile")).expect("justfile should exist");
    assert!(justfile.contains("uv run python -c \"import test\""));
    assert!(!justfile.contains("uv run python -c \"import sandbox\""));
}

#[test]
fn init_accepts_positional_path_and_ignored_managed_files() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    create_existing_python_repo_with_pyproject(&project_path);

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "init",
        project_path.to_str().expect("valid UTF-8 path"),
        "--blueprint",
        "python-library",
        "--ignore",
        "AGENTS.md",
        "--ignore",
        "CLAUDE.md",
        "--yes",
    ]);

    cmd.assert().success();

    let pyproject =
        fs::read_to_string(project_path.join("pyproject.toml")).expect("pyproject should exist");
    assert!(pyproject.contains("ignore = [\"AGENTS.md\", \"CLAUDE.md\", \"editorconfig\"]"));
    assert!(!project_path.join("AGENTS.md").exists());
    assert!(!project_path.join("CLAUDE.md").exists());

    let mut check = Command::cargo_bin("forge").expect("forge binary should build");
    check.args([
        "sync",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--check",
    ]);
    check
        .assert()
        .success()
        .stdout(contains("managed infrastructure is current"));
}

#[test]
fn init_infers_python_metadata_from_existing_pyproject() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    create_existing_python_repo_with_pyproject(&project_path);

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "init",
        "--blueprint",
        "python-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--yes",
    ]);

    cmd.assert()
        .success()
        .stdout(contains("Repository initialized"))
        .stdout(contains("update  pyproject.toml"));

    assert_external_pyproject_adopted(&project_path);

    let justfile =
        fs::read_to_string(project_path.join("justfile")).expect("justfile should exist");
    assert!(justfile.contains("uv run --locked ruff check ."));

    let mut check = Command::cargo_bin("forge").expect("forge binary should build");
    check.args([
        "sync",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--check",
    ]);
    check
        .assert()
        .success()
        .stdout(contains("managed infrastructure is current"));
}

#[test]
fn init_yes_preserves_existing_pyproject_metadata() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    create_existing_python_repo_with_pyproject(&project_path);
    fs::write(project_path.join("README.md"), "# Handwritten\n")
        .expect("readme should be writable");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "init",
        "--blueprint",
        "python-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--yes",
    ]);

    cmd.assert()
        .success()
        .stdout(contains("update  README.md"))
        .stdout(contains("update  pyproject.toml"));

    assert_external_pyproject_adopted(&project_path);
}

#[test]
fn init_adds_managed_infrastructure_to_existing_rust_repo() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-rs");
    fs::create_dir_all(project_path.join("src")).expect("source tree should create");
    fs::write(
        project_path.join("src/lib.rs"),
        "pub fn existing() -> &'static str {\n    \"kept\"\n}\n",
    )
    .expect("existing source should be writable");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "init",
        "--blueprint",
        "rust-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "ops-rs",
        "--package-name",
        "ops_rs",
        "--description",
        "Ops toolchain",
        "--author-name",
        "Grace Hopper",
        "--author-email",
        "grace@example.com",
        "--license",
        "MIT",
        "--yes",
    ]);

    cmd.assert()
        .success()
        .stdout(contains("Repository initialized"))
        .stdout(contains("[ok] managed infrastructure added"))
        .stdout(contains("blueprint: rust-library"))
        .stdout(contains("infrastructure:"))
        .stdout(contains("forge sync --path ."));

    let existing_source =
        fs::read_to_string(project_path.join("src/lib.rs")).expect("source should exist");
    assert!(existing_source.contains("\"kept\""));

    let cargo_toml =
        fs::read_to_string(project_path.join("Cargo.toml")).expect("Cargo.toml should exist");
    assert!(cargo_toml.contains("name = \"ops-rs\""));
    assert!(cargo_toml.contains("edition = \"2024\""));

    let pyproject =
        fs::read_to_string(project_path.join("pyproject.toml")).expect("pyproject should exist");
    assert!(pyproject.contains("blueprint = \"rust-library>=0.1.0\""));
    assert!(project_path.join(".github/workflows/ci.yaml").exists());
    assert!(project_path.join("justfile").exists());
    assert_eq!(
        fs::read_link(project_path.join("CLAUDE.md")).expect("CLAUDE.md should be a symlink"),
        std::path::PathBuf::from("AGENTS.md")
    );

    let mut check = Command::cargo_bin("forge").expect("forge binary should build");
    check.args([
        "sync",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--check",
    ]);
    check
        .assert()
        .success()
        .stdout(contains("managed infrastructure is current"));
}

#[test]
fn init_adds_managed_infrastructure_to_existing_any_project_repo() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo-infra");
    fs::create_dir_all(project_path.join("scripts")).expect("scripts tree should create");
    fs::write(
        project_path.join("scripts/healthcheck.sh"),
        "#!/usr/bin/env bash\necho kept\n",
    )
    .expect("existing script should be writable");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "init",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--description",
        "Shared repository infrastructure",
        "--prettier",
        "--yes",
    ]);

    cmd.assert()
        .success()
        .stdout(contains("Repository initialized"))
        .stdout(contains("[ok] managed infrastructure added"))
        .stdout(contains("blueprint: any-project"))
        .stdout(contains("infrastructure:"))
        .stdout(contains("docs"))
        .stdout(contains("forge sync --path ."));

    let script = fs::read_to_string(project_path.join("scripts/healthcheck.sh"))
        .expect("script should remain readable");
    assert!(script.contains("echo kept"));

    let pyproject =
        fs::read_to_string(project_path.join("pyproject.toml")).expect("pyproject should exist");
    assert!(pyproject.contains("blueprint = \"any-project>=0.1.0\""));
    assert!(pyproject.contains("prettier = true"));
    assert!(project_path.join(".prettierrc.json").exists());
    assert!(project_path.join("docs/package.json").exists());
    assert!(
        project_path
            .join("docs/src/content/docs/index.mdx")
            .exists()
    );
    assert_eq!(
        fs::read_link(project_path.join("CLAUDE.md")).expect("CLAUDE.md should be a symlink"),
        std::path::PathBuf::from("AGENTS.md")
    );

    let mut check = Command::cargo_bin("forge").expect("forge binary should build");
    check.args([
        "sync",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--check",
    ]);
    check
        .assert()
        .success()
        .stdout(contains("managed infrastructure is current"));
}

#[test]
fn init_accepts_explicit_false_for_prettier_component() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    fs::create_dir_all(&project_path).expect("project dir should create");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "init",
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
        "--prettier=false",
        "--yes",
    ]);

    cmd.assert().success();

    let pyproject =
        fs::read_to_string(project_path.join("pyproject.toml")).expect("pyproject should exist");
    assert!(!pyproject.contains("prettier = false"));
    assert!(!project_path.join(".prettierrc.json").exists());
    assert!(!project_path.join(".prettierignore").exists());
}

#[test]
fn init_json_reports_next_steps_after_writing() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo-infra");
    fs::create_dir_all(&project_path).expect("project dir should create");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "init",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--description",
        "Shared repo infrastructure",
        "--json",
        "--yes",
    ]);

    let output = cmd.output().expect("init should run");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    let report: serde_json::Value = serde_json::from_str(&stdout).expect("stdout should be JSON");
    assert_eq!(report["status_code"], "created");
    assert_eq!(report["blueprint_version"], "0.1.0");
    assert_eq!(
        report["next_steps"],
        serde_json::json!([
            format!("cd {}", project_path.display()),
            "uv sync --all-groups",
            "just verify"
        ])
    );

    assert!(project_path.join("pyproject.toml").exists());
}

#[test]
fn init_json_quotes_success_cd_next_step_for_paths_with_spaces() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo infra");
    fs::create_dir_all(&project_path).expect("project dir should create");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "init",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--description",
        "Shared repo infrastructure",
        "--json",
        "--yes",
    ]);

    let output = cmd.output().expect("init should run");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    let report: serde_json::Value = serde_json::from_str(&stdout).expect("stdout should be JSON");
    assert_eq!(report["status_code"], "created");
    assert_eq!(report["blueprint_version"], "0.1.0");
    assert_eq!(
        report["next_steps"],
        serde_json::json!([
            format!("cd '{}'", project_path.display()),
            "uv sync --all-groups",
            "just verify"
        ])
    );
}

#[test]
fn init_rejects_already_managed_repository() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    generate_python_project(&project_path);

    let original_pyproject =
        fs::read_to_string(project_path.join("pyproject.toml")).expect("pyproject should exist");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "init",
        "--blueprint",
        "python-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "ops-tools",
        "--package-name",
        "ops_tools",
        "--description",
        "Replacement metadata",
        "--author-name",
        "Grace Hopper",
        "--author-email",
        "grace@example.com",
        "--yes",
    ]);

    cmd.assert()
        .failure()
        .stderr(contains("repository is already managed by forge"))
        .stderr(contains("forge sync --path"))
        .stderr(contains("error_code: FORGE_E_CONFLICT"));

    let pyproject_after =
        fs::read_to_string(project_path.join("pyproject.toml")).expect("pyproject should exist");
    assert_eq!(pyproject_after, original_pyproject);
}

#[test]
fn init_missing_path_creates_new_project() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo infra");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "init",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--description",
        "Shared repo infrastructure",
        "--yes",
    ]);

    cmd.assert()
        .success()
        .stdout(contains("Project created"))
        .stdout(contains("generated repo-infra"));
    assert!(project_path.join("pyproject.toml").exists());
}

#[test]
fn init_json_missing_path_creates_project_report() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo infra");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "init",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--description",
        "Shared repo infrastructure",
        "--json",
        "--yes",
    ]);

    let output = cmd.output().expect("init should run");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    let report: serde_json::Value = serde_json::from_str(&stdout).expect("stdout should be JSON");
    assert_eq!(report["status_code"], "created");
    assert_eq!(report["path"], project_path.display().to_string());
}

#[test]
fn init_file_path_explains_repository_directory_requirement() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo infra");
    fs::write(&project_path, "not a directory\n").expect("file path should be writable");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "init",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--description",
        "Shared repo infrastructure",
        "--yes",
    ]);

    cmd.assert()
        .failure()
        .stderr(contains("repository path is not a directory"))
        .stderr(contains("choose an existing repository directory"))
        .stderr(contains(format!(
            "forge init --path '{}'",
            project_path.display()
        )));
}

#[test]
fn init_json_file_path_keeps_stdout_empty() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo infra");
    fs::write(&project_path, "not a directory\n").expect("file path should be writable");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "init",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--description",
        "Shared repo infrastructure",
        "--json",
        "--yes",
    ]);

    let output = cmd.output().expect("init should run");
    assert!(!output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.is_empty());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("repository path is not a directory"));
    assert!(stderr.contains("choose an existing repository directory"));
}

#[test]
fn init_rejects_corrupt_existing_forge_metadata() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    fs::create_dir_all(&project_path).expect("project dir should create");
    fs::write(
        project_path.join("pyproject.toml"),
        "[tool.forge]\nblueprint = \"python-library\"\n",
    )
    .expect("pyproject should be writable");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "init",
        "--blueprint",
        "python-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "ops-tools",
        "--package-name",
        "ops_tools",
        "--description",
        "Replacement metadata",
        "--author-name",
        "Grace Hopper",
        "--author-email",
        "grace@example.com",
        "--yes",
    ]);

    cmd.assert()
        .failure()
        .stderr(contains("missing tool.forge.blueprint version"))
        .stderr(contains("error_code: FORGE_E_ENV"));

    let pyproject_after =
        fs::read_to_string(project_path.join("pyproject.toml")).expect("pyproject should exist");
    assert_eq!(
        pyproject_after,
        "[tool.forge]\nblueprint = \"python-library\"\n"
    );
}

#[test]
fn init_rejects_newer_blueprint_version_than_supported() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("ops-tools");
    fs::create_dir_all(&project_path).expect("project dir should create");
    fs::write(
        project_path.join("pyproject.toml"),
        "[tool.forge]\nblueprint = \"python-library\"\nblueprint_version = \"9.0.0\"\n",
    )
    .expect("pyproject should be writable");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "init",
        "--blueprint",
        "python-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "ops-tools",
        "--package-name",
        "ops_tools",
        "--description",
        "Replacement metadata",
        "--author-name",
        "Grace Hopper",
        "--author-email",
        "grace@example.com",
        "--yes",
    ]);

    cmd.assert()
        .failure()
        .stderr(contains("newer than this forge supports"))
        .stderr(contains("upgrade forge"))
        .stderr(contains("error_code: FORGE_E_ENV"));
}

#[test]
fn init_yes_overwrites_existing_files_without_force() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo-infra");
    fs::create_dir_all(&project_path).expect("project dir should create");
    fs::write(project_path.join("README.md"), "# Handwritten\n")
        .expect("readme should be writable");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "init",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--description",
        "Shared repo infrastructure",
        "--yes",
    ]);

    cmd.assert()
        .success()
        .stdout(contains("update  README.md"))
        .stdout(contains("Repository initialized"));

    let readme = fs::read_to_string(project_path.join("README.md")).expect("README should exist");
    assert!(readme.contains("Shared repo infrastructure"));
    assert!(project_path.join("pyproject.toml").exists());
}

#[test]
fn init_json_conflict_report_includes_recovery_next_steps() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo-infra");
    fs::create_dir_all(&project_path).expect("project dir should create");
    fs::write(project_path.join("README.md"), "# Handwritten\n")
        .expect("readme should be writable");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "init",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--description",
        "Shared repo infrastructure",
        "--json",
        "--yes",
    ]);

    let output = cmd.output().expect("init should run");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(!stdout.contains("Repository initialization"));
    assert!(!stdout.contains("Next steps"));
    let report: serde_json::Value = serde_json::from_str(&stdout).expect("stdout should be JSON");
    assert_eq!(report["status_code"], "initialized");
    assert_eq!(report["blueprint_version"], "0.1.0");
    assert_eq!(report["conflicts"], 0);
    assert!(
        report["actions"]
            .as_array()
            .expect("actions should be an array")
            .iter()
            .any(|action| action["action"] == "update" && action["path"] == "README.md")
    );
    assert!(
        fs::read_to_string(project_path.join("README.md"))
            .expect("README should exist")
            .contains("Shared repo infrastructure")
    );
    assert!(project_path.join("pyproject.toml").exists());
}

#[test]
fn init_json_conflict_report_quotes_yes_command_path_with_spaces() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo infra");
    fs::create_dir_all(&project_path).expect("project dir should create");
    fs::write(project_path.join("README.md"), "# Handwritten\n")
        .expect("readme should be writable");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "init",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--description",
        "Shared repo infrastructure",
        "--json",
        "--yes",
    ]);

    let output = cmd.output().expect("init should run");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(!stdout.contains("Repository initialization"));
    assert!(!stdout.contains("Next steps"));
    let report: serde_json::Value = serde_json::from_str(&stdout).expect("stdout should be JSON");
    assert_eq!(report["status_code"], "initialized");
    assert_eq!(report["blueprint_version"], "0.1.0");
    assert_eq!(
        report["next_steps"][0],
        format!("cd '{}'", project_path.display())
    );
}

#[test]
fn init_diff_shows_conflicting_managed_file_changes_before_overwrite() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo-infra");
    fs::create_dir_all(&project_path).expect("project dir should create");
    fs::write(project_path.join("README.md"), "# Handwritten\n")
        .expect("readme should be writable");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "init",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--description",
        "Shared repo infrastructure",
        "--dry-run",
        "--diff",
        "--yes",
    ]);

    cmd.assert()
        .success()
        .stdout(contains("update  README.md"))
        .stdout(contains("Managed diff"))
        .stdout(contains("--- a/README.md"))
        .stdout(contains("+++ b/README.md"))
        .stdout(contains("-# Handwritten"))
        .stdout(contains("+# repo-infra"));

    let readme = fs::read_to_string(project_path.join("README.md")).expect("README should exist");
    assert_eq!(readme, "# Handwritten\n");
    assert!(!project_path.join("pyproject.toml").exists());
}

#[test]
fn init_diff_requires_dry_run() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo-infra");
    fs::create_dir_all(&project_path).expect("project dir should create");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "init",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--description",
        "Shared repo infrastructure",
        "--diff",
        "--yes",
    ]);

    cmd.assert()
        .failure()
        .stderr(contains("--diff requires --dry-run"));
    assert!(!project_path.join("pyproject.toml").exists());
}

#[test]
fn init_yes_overwrites_existing_managed_files_after_explicit_review() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo-infra");
    fs::create_dir_all(&project_path).expect("project dir should create");
    fs::write(project_path.join("README.md"), "# Handwritten\n")
        .expect("readme should be writable");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "init",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--description",
        "Shared repo infrastructure",
        "--yes",
    ]);

    cmd.assert()
        .success()
        .stdout(contains("update  README.md"))
        .stdout(contains("Repository initialized"));

    let readme = fs::read_to_string(project_path.join("README.md")).expect("README should exist");
    assert!(readme.contains("Shared repo infrastructure"));
    assert!(readme.contains("Forge Metadata"));
    let pyproject =
        fs::read_to_string(project_path.join("pyproject.toml")).expect("pyproject should exist");
    assert!(pyproject.contains("blueprint = \"any-project>=0.1.0\""));
}

#[test]
fn init_yes_dry_run_json_reports_overwrites_without_writing() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo-infra");
    fs::create_dir_all(&project_path).expect("project dir should create");
    fs::write(project_path.join("README.md"), "# Handwritten\n")
        .expect("readme should be writable");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "init",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--description",
        "Shared repo infrastructure",
        "--dry-run",
        "--json",
        "--yes",
    ]);

    let output = cmd.output().expect("init should run");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    let report: serde_json::Value = serde_json::from_str(&stdout).expect("stdout should be JSON");
    assert_eq!(report["status_code"], "dry_run");
    assert_eq!(report["blueprint_version"], "0.1.0");
    assert_eq!(report["dry_run"], true);
    assert!(report.get("force").is_none());
    assert_eq!(report["required_tools"], "uv, just");
    assert_eq!(report["conflicts"], 0);
    assert_eq!(
        report["next_steps"],
        serde_json::json!([format!(
            "forge init --path {} --blueprint any-project --project-name repo-infra --description 'Shared repo infrastructure' --yes",
            project_path.display()
        )])
    );
    assert!(
        report["actions"]
            .as_array()
            .expect("actions should be an array")
            .iter()
            .any(|action| action["action"] == "update" && action["path"] == "README.md")
    );

    let readme = fs::read_to_string(project_path.join("README.md")).expect("README should exist");
    assert_eq!(readme, "# Handwritten\n");
    assert!(!project_path.join("pyproject.toml").exists());
}

#[test]
fn init_dry_run_json_reports_without_writing() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo-infra");
    fs::create_dir_all(&project_path).expect("project dir should create");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "init",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--description",
        "Shared repo infrastructure",
        "--dry-run",
        "--json",
        "--yes",
    ]);

    let output = cmd.output().expect("init should run");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    let report: serde_json::Value = serde_json::from_str(&stdout).expect("stdout should be JSON");
    assert_eq!(report["status_code"], "dry_run");
    assert_eq!(report["dry_run"], true);
    assert_eq!(report["blueprint"], "any-project");
    assert_eq!(report["blueprint_version"], "0.1.0");
    assert_eq!(report["required_tools"], "uv, just");
    assert!(
        report["infrastructure"]
            .as_str()
            .expect("infrastructure should be a string")
            .contains("pyproject.toml")
    );
    assert_eq!(
        report["next_steps"],
        serde_json::json!([format!(
            "forge init --path {} --blueprint any-project --project-name repo-infra --description 'Shared repo infrastructure' --yes",
            project_path.display()
        )])
    );
    assert!(
        report["files"]
            .as_array()
            .expect("files should be an array")
            .iter()
            .any(|path| path == "pyproject.toml")
    );

    assert!(!project_path.join("pyproject.toml").exists());
}

#[test]
fn init_dry_run_json_quotes_apply_command_path_with_spaces() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo infra");
    fs::create_dir_all(&project_path).expect("project dir should create");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "init",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--description",
        "Shared repo infrastructure",
        "--dry-run",
        "--json",
        "--yes",
    ]);

    let output = cmd.output().expect("init should run");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    let report: serde_json::Value = serde_json::from_str(&stdout).expect("stdout should be JSON");
    assert_eq!(
        report["next_steps"],
        serde_json::json!([format!(
            "forge init --path '{}' --blueprint any-project --project-name repo-infra --description 'Shared repo infrastructure' --yes",
            project_path.display()
        )])
    );
    assert!(!project_path.join("pyproject.toml").exists());
}

#[test]
fn init_non_tty_dry_run_can_run_without_yes_when_flags_are_explicit() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo-infra");
    fs::create_dir_all(&project_path).expect("project dir should create");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "init",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--description",
        "Shared repo infrastructure",
        "--dry-run",
    ]);

    cmd.assert()
        .success()
        .stdout(contains("Project creation preview"))
        .stdout(contains("repo-infra"))
        .stdout(contains("required tools: uv, just"))
        .stdout(contains("infrastructure:"))
        .stderr(contains("interactive confirmation requires a terminal").not());
    assert!(!project_path.join("pyproject.toml").exists());
}

#[test]
fn init_dry_run_reports_component_required_tools() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo-infra");
    fs::create_dir_all(&project_path).expect("project dir should create");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "init",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--description",
        "Shared repo infrastructure",
        "--markdownlint",
        "--dry-run",
    ]);

    cmd.assert()
        .success()
        .stdout(contains("Project creation preview"))
        .stdout(contains("required tools: uv, just, npx"));
}

#[test]
fn init_dry_run_json_reports_component_required_tools() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo-infra");
    fs::create_dir_all(&project_path).expect("project dir should create");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "init",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--description",
        "Shared repo infrastructure",
        "--markdownlint",
        "--dry-run",
        "--json",
        "--yes",
    ]);

    let output = cmd.output().expect("init should run");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    let report: serde_json::Value = serde_json::from_str(&stdout).expect("stdout should be JSON");
    assert_eq!(report["required_tools"], "uv, just, npx");
}

#[test]
fn init_non_tty_requires_blueprint_when_interactive_setup_is_unavailable() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo-infra");
    fs::create_dir_all(&project_path).expect("project dir should create");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "init",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--description",
        "Shared repo infrastructure",
        "--dry-run",
    ]);

    cmd.assert()
        .failure()
        .stderr(contains(
            "--blueprint is required when interactive setup is unavailable",
        ))
        .stderr(contains("error_code: FORGE_E_INPUT"));
    assert!(!project_path.join("pyproject.toml").exists());
}
