use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use tempfile::TempDir;

fn write_executable(path: &std::path::Path, content: &str) {
    fs::write(path, content).expect("fake command should be writable");
    let mut permissions = fs::metadata(path)
        .expect("fake command metadata should be readable")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("fake command should be executable");
}

#[test]
fn new_generates_python_project_with_metadata() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-tools");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "python-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-tools",
        "--package-name",
        "grid_tools",
        "--description",
        "Grid toolchain",
        "--author-name",
        "Ada Lovelace",
        "--author-email",
        "ada@example.com",
        "--license",
        "BSD-3-Clause",
        "--python-min",
        "3.11",
        "--yes",
    ]);

    cmd.assert()
        .success()
        .stdout(contains("Project created"))
        .stdout(contains("[ok] generated grid-tools"))
        .stdout(contains("blueprint: python-library"))
        .stdout(contains(
            "options: enabled: docs, codecov; disabled: pypi-publish, prettier, editorconfig, markdownlint",
        ))
        .stdout(contains("required tools: uv, just"))
        .stdout(contains("infrastructure:"))
        .stdout(contains("prek hooks"))
        .stdout(contains("Next steps"))
        .stdout(contains("just verify"));

    let pyproject = fs::read_to_string(project_path.join("pyproject.toml"))
        .expect("pyproject.toml should be generated");
    assert!(pyproject.contains("[tool.forge]"));
    assert!(pyproject.contains("blueprint = \"python-library\""));
    assert!(pyproject.contains("blueprint_version = \"0.1.0\""));
    assert!(pyproject.contains("project_name = \"grid-tools\""));

    let readme = fs::read_to_string(project_path.join("README.md")).expect("README should exist");
    assert!(readme.contains("forge update --path ."));
    assert!(readme.contains("forge update --path . --dry-run"));
    assert!(readme.contains("forge update --path . --check"));
    assert!(readme.contains("uv lock"));
    assert!(readme.contains("[tool.forge]"));
    assert!(readme.contains("Automated Forge Updates"));
    assert!(readme.contains("Forge-managed infrastructure updates"));
    assert!(!readme.contains("infra-only"));
    assert!(!readme.contains("template-managed"));

    let justfile =
        fs::read_to_string(project_path.join("justfile")).expect("justfile should exist");
    assert!(justfile.contains("\ndocs:\n"));
    assert!(justfile.contains("cd docs && npm run dev"));
    assert!(justfile.contains("uv lock --check"));
    assert!(justfile.contains("uv run --locked ruff format --check ."));
    assert!(justfile.contains("uv run --locked ruff check ."));
    assert!(justfile.contains("uv run --locked prek run --all-files"));
    assert!(justfile.contains("forge update --path . --check"));
    assert!(justfile.contains("uv build --locked"));

    assert!(project_path.join("src/grid_tools/__init__.py").exists());
    let agents = fs::read_to_string(project_path.join("AGENTS.md")).expect("AGENTS should exist");
    assert!(agents.contains("Follow TDD"));
    assert!(agents.contains("Preserve user-authored project code"));
    assert!(agents.contains("Preserve user-authored Python package code"));

    let ci = fs::read_to_string(project_path.join(".github/workflows/ci.yaml"))
        .expect("CI workflow should be generated");
    assert!(ci.contains("permissions:\n  contents: read\n\njobs:"));
    assert!(ci.contains("actions/checkout@v6"));
    assert!(ci.contains("actions/setup-python@v6"));
    assert!(ci.contains("cargo install --git https://github.com/pesap/forge --locked forge"));
    assert!(ci.contains("uv lock --check"));
    assert!(ci.contains("uv run --locked ruff format --check ."));
    assert!(ci.contains("uv run --locked ruff check ."));
    assert!(ci.contains("forge update --path . --check"));
    assert_eq!(
        fs::read_link(project_path.join("CLAUDE.md")).expect("CLAUDE.md should be a symlink"),
        std::path::PathBuf::from("AGENTS.md")
    );

    let update_workflow =
        fs::read_to_string(project_path.join(".github/workflows/forge-update.yaml"))
            .expect("forge update workflow should be generated");
    assert!(update_workflow.contains("forge update --path ."));
    assert!(update_workflow.contains("uv lock"));
    assert!(update_workflow.contains("persist-credentials: false"));
    assert!(update_workflow.contains("peter-evans/create-pull-request"));

    let precommit = fs::read_to_string(project_path.join(".pre-commit-config.yaml"))
        .expect("pre-commit config should be generated");
    assert!(precommit.contains("id: forge-update-check"));
    assert!(precommit.contains("forge update --path . --check"));
    assert!(precommit.contains("entry: uv run --locked ruff format --check"));
    assert!(precommit.contains("entry: uv run --locked ruff check"));
    assert!(precommit.contains("entry: uv run --locked pytest -q --maxfail=1"));
    assert!(!precommit.contains("uv run ruff check --fix"));
}

#[test]
fn new_can_emit_json_report_without_human_output() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-tools");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "python-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-tools",
        "--package-name",
        "grid_tools",
        "--description",
        "Grid toolchain",
        "--author-name",
        "Ada Lovelace",
        "--author-email",
        "ada@example.com",
        "--yes",
        "--json",
    ]);

    let output = cmd.output().expect("new should run");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(!stdout.contains("Project created"));
    assert!(!stdout.contains("Initialized empty Git repository"));

    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert_eq!(report["project_name"], "grid-tools");
    assert_eq!(report["blueprint"], "python-library");
    assert_eq!(report["blueprint_version"], "0.1.0");
    assert_eq!(report["status_code"], "created");
    assert_eq!(report["path"], project_path.display().to_string());
    assert_eq!(report["github"], false);
    assert_eq!(report["required_tools"], "uv, just");
    assert!(
        report["infrastructure"]
            .as_str()
            .expect("infrastructure should be a string")
            .contains("pyproject.toml")
    );
    assert!(
        report["options"]
            .as_array()
            .expect("options should be an array")
            .iter()
            .any(|option| option["name"] == "docs" && option["enabled"] == true)
    );
    assert!(
        report["options"]
            .as_array()
            .expect("options should be an array")
            .iter()
            .any(|option| option["name"] == "prettier" && option["enabled"] == false)
    );
    assert!(
        report["next_steps"]
            .as_array()
            .expect("next_steps should be an array")
            .iter()
            .any(|step| step
                .as_str()
                .expect("step should be a string")
                .starts_with("cd "))
    );
    assert!(
        report["next_steps"]
            .as_array()
            .expect("next_steps should be an array")
            .iter()
            .any(|step| step == "just verify")
    );

    assert!(project_path.join("pyproject.toml").exists());
}

#[test]
fn new_json_quotes_cd_next_step_for_paths_with_spaces() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid tools");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "python-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-tools",
        "--package-name",
        "grid_tools",
        "--description",
        "Grid toolchain",
        "--author-name",
        "Ada Lovelace",
        "--author-email",
        "ada@example.com",
        "--yes",
        "--json",
    ]);

    let output = cmd.output().expect("new should run");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert_eq!(report["path"], project_path.display().to_string());
    assert!(
        report["next_steps"]
            .as_array()
            .expect("next_steps should be an array")
            .iter()
            .any(|step| step
                == &serde_json::Value::String(format!("cd '{}'", project_path.display())))
    );
}

#[test]
fn new_dry_run_previews_files_without_writing() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-tools");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "python-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-tools",
        "--package-name",
        "grid_tools",
        "--description",
        "Grid toolchain",
        "--author-name",
        "Ada Lovelace",
        "--author-email",
        "ada@example.com",
        "--yes",
        "--dry-run",
    ]);

    cmd.assert()
        .success()
        .stdout(contains("Project creation preview"))
        .stdout(contains("blueprint: python-library"))
        .stdout(contains(
            "options: enabled: docs, codecov; disabled: pypi-publish, prettier, editorconfig, markdownlint",
        ))
        .stdout(contains("required tools: uv, just"))
        .stdout(contains("infrastructure:"))
        .stdout(contains("github actions ("))
        .stdout(contains("create  pyproject.toml"))
        .stdout(contains(format!(
            "forge new --path {} --blueprint python-library --project-name grid-tools --package-name grid_tools --description 'Grid toolchain' --author-name 'Ada Lovelace' --author-email 'ada@example.com' --yes",
            project_path.display()
        )));

    assert!(!project_path.exists());
}

#[test]
fn new_dry_run_color_always_forces_ansi_styles() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-tools");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "--color",
        "always",
        "new",
        "--blueprint",
        "python-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-tools",
        "--package-name",
        "grid_tools",
        "--description",
        "Grid toolchain",
        "--author-name",
        "Ada Lovelace",
        "--author-email",
        "ada@example.com",
        "--yes",
        "--dry-run",
    ]);

    let output = cmd.output().expect("new dry-run should run");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains("Project creation preview"));
    assert!(stdout.contains("\u{1b}["));
}

#[test]
fn new_dry_run_reports_component_required_tools() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo-infra");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--description",
        "Shared infrastructure",
        "--markdownlint",
        "--yes",
        "--dry-run",
    ]);

    cmd.assert()
        .success()
        .stdout(contains("Project creation preview"))
        .stdout(contains("required tools: uv, just, npx"));
}

#[test]
fn new_dry_run_color_never_disables_ansi_styles() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-tools");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "--color",
        "never",
        "new",
        "--blueprint",
        "python-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-tools",
        "--package-name",
        "grid_tools",
        "--description",
        "Grid toolchain",
        "--author-name",
        "Ada Lovelace",
        "--author-email",
        "ada@example.com",
        "--yes",
        "--dry-run",
    ]);

    let output = cmd.output().expect("new dry-run should run");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains("Project creation preview"));
    assert!(!stdout.contains("\u{1b}["));
}

#[test]
fn new_dry_run_diff_shows_generated_text_without_writing() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-tools");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "python-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-tools",
        "--package-name",
        "grid_tools",
        "--description",
        "Grid toolchain",
        "--author-name",
        "Ada Lovelace",
        "--author-email",
        "ada@example.com",
        "--yes",
        "--dry-run",
        "--diff",
    ]);

    cmd.assert()
        .success()
        .stdout(contains("Project creation preview"))
        .stdout(contains("Managed diff"))
        .stdout(contains("--- /dev/null"))
        .stdout(contains("+++ b/pyproject.toml"))
        .stdout(contains("+[project]"));

    assert!(!project_path.exists());
}

#[test]
fn new_diff_requires_dry_run() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-tools");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "python-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-tools",
        "--package-name",
        "grid_tools",
        "--description",
        "Grid toolchain",
        "--author-name",
        "Ada Lovelace",
        "--author-email",
        "ada@example.com",
        "--yes",
        "--diff",
    ]);

    cmd.assert()
        .failure()
        .stderr(contains("--diff requires --dry-run"));

    assert!(!project_path.exists());
}

#[test]
fn new_dry_run_can_emit_json_report_without_writing() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-tools");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "python-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-tools",
        "--package-name",
        "grid_tools",
        "--description",
        "Grid toolchain",
        "--author-name",
        "Ada Lovelace",
        "--author-email",
        "ada@example.com",
        "--yes",
        "--dry-run",
        "--json",
    ]);

    let output = cmd.output().expect("new dry-run should run");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(!stdout.contains("Project creation preview"));
    assert!(!stdout.contains("Initialized empty Git repository"));

    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert_eq!(report["project_name"], "grid-tools");
    assert_eq!(report["blueprint"], "python-library");
    assert_eq!(report["blueprint_version"], "0.1.0");
    assert_eq!(report["status_code"], "dry_run");
    assert_eq!(report["dry_run"], true);
    assert_eq!(report["required_tools"], "uv, just");
    assert!(
        report["options"]
            .as_array()
            .expect("options should be an array")
            .iter()
            .any(|option| option["name"] == "codecov" && option["enabled"] == true)
    );
    assert!(
        report["files"]
            .as_array()
            .expect("files should be an array")
            .iter()
            .any(|file| file == "pyproject.toml")
    );
    assert_eq!(
        report["next_steps"],
        serde_json::json!([format!(
            "forge new --path {} --blueprint python-library --project-name grid-tools --package-name grid_tools --description 'Grid toolchain' --author-name 'Ada Lovelace' --author-email 'ada@example.com' --yes",
            project_path.display()
        )])
    );

    assert!(!project_path.exists());
}

#[test]
fn new_dry_run_json_quotes_apply_command_path_with_spaces() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid tools");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-tools",
        "--description",
        "Grid toolchain",
        "--github",
        "--github-owner",
        "example-org",
        "--github-visibility",
        "private",
        "--prettier",
        "--yes",
        "--dry-run",
        "--json",
    ]);

    let output = cmd.output().expect("new dry-run should run");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert_eq!(report["required_tools"], "uv, just, npx");
    assert_eq!(
        report["next_steps"],
        serde_json::json!([format!(
            "forge new --path '{}' --blueprint any-project --project-name grid-tools --description 'Grid toolchain' --prettier --github --github-owner example-org --github-visibility private --yes",
            project_path.display()
        )])
    );
    assert!(!project_path.exists());
}

#[test]
fn new_dry_run_reports_github_intent_without_requiring_gh() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-tools");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-tools",
        "--description",
        "Grid toolchain",
        "--github",
        "--github-visibility",
        "private",
        "--yes",
        "--dry-run",
    ]);

    cmd.assert()
        .success()
        .stdout(contains("github: create private repository"));

    assert!(!project_path.exists());
}

#[test]
fn new_dry_run_json_reports_github_visibility() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-tools");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-tools",
        "--description",
        "Grid toolchain",
        "--github",
        "--github-visibility",
        "private",
        "--yes",
        "--dry-run",
        "--json",
    ]);

    let output = cmd.output().expect("new dry-run should run");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert_eq!(report["github"], true);
    assert_eq!(report["github_visibility"], "private");
    assert_eq!(report["dry_run"], true);

    assert!(!project_path.exists());
}

#[test]
fn new_github_creation_locks_dependencies_before_initial_commit() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo-infra");
    let bin_dir = temp.path().join("bin");
    fs::create_dir(&bin_dir).expect("fake bin dir should create");
    let log_path = temp.path().join("commands.log");

    write_executable(
        &bin_dir.join("uv"),
        &format!(
            "#!/bin/sh\nprintf 'uv %s\\n' \"$*\" >> {}\nif [ \"$1\" = lock ]; then printf 'version = 1\\n' > uv.lock; fi\n",
            log_path.display()
        ),
    );
    write_executable(
        &bin_dir.join("git"),
        &format!(
            "#!/bin/sh\nprintf 'git %s\\n' \"$*\" >> {}\nexit 0\n",
            log_path.display()
        ),
    );
    write_executable(
        &bin_dir.join("gh"),
        &format!(
            "#!/bin/sh\nprintf 'gh %s\\n' \"$*\" >> {}\ncase \"$1 $2\" in\n  'auth status') exit 0 ;;\n  'repo create') exit 0 ;;\n  *) exit 1 ;;\nesac\n",
            log_path.display()
        ),
    );

    let path = format!(
        "{}:{}",
        bin_dir.display(),
        std::env::var("PATH").expect("PATH should be set")
    );
    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.env("PATH", path);
    cmd.args([
        "new",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--description",
        "Shared repository infrastructure",
        "--github",
        "--github-owner",
        "example-org",
        "--yes",
        "--json",
    ]);

    let output = cmd.output().expect("new --github should run");
    assert!(output.status.success());

    assert!(project_path.join("uv.lock").exists());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    let report: serde_json::Value = serde_json::from_str(&stdout).expect("stdout should be JSON");
    assert!(
        report["files"]
            .as_array()
            .expect("files should be an array")
            .iter()
            .any(|file| file == "uv.lock")
    );
    let log = fs::read_to_string(log_path).expect("command log should exist");
    let uv_lock = log.find("uv lock").expect("uv lock should run");
    let git_add = log.find("git add .").expect("git add should run");
    let git_commit = log
        .find("git commit -m chore: initialize project with forge")
        .expect("git commit should run");
    let gh_create = log
        .find("gh repo create example-org/repo-infra")
        .expect("gh repo create should run");
    assert!(uv_lock < git_add);
    assert!(git_add < git_commit);
    assert!(git_commit < gh_create);
}

#[test]
fn new_github_lock_failure_reports_local_recovery_context() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo infra");
    let bin_dir = temp.path().join("bin");
    fs::create_dir(&bin_dir).expect("fake bin dir should create");
    let log_path = temp.path().join("commands.log");

    write_executable(
        &bin_dir.join("uv"),
        &format!(
            "#!/bin/sh\nprintf 'uv %s\\n' \"$*\" >> {}\nif [ \"$1\" = lock ]; then exit 42; fi\nexit 0\n",
            log_path.display()
        ),
    );
    write_executable(
        &bin_dir.join("git"),
        &format!(
            "#!/bin/sh\nprintf 'git %s\\n' \"$*\" >> {}\nexit 0\n",
            log_path.display()
        ),
    );
    write_executable(
        &bin_dir.join("gh"),
        &format!(
            "#!/bin/sh\nprintf 'gh %s\\n' \"$*\" >> {}\ncase \"$1 $2\" in\n  'auth status') exit 0 ;;\n  *) exit 1 ;;\nesac\n",
            log_path.display()
        ),
    );

    let path = format!(
        "{}:{}",
        bin_dir.display(),
        std::env::var("PATH").expect("PATH should be set")
    );
    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.env("PATH", path);
    cmd.args([
        "new",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--description",
        "Shared repository infrastructure",
        "--github",
        "--github-owner",
        "example-org",
        "--yes",
    ]);

    cmd.assert()
        .failure()
        .stderr(contains(
            "dependency locking failed after local project generation",
        ))
        .stderr(contains(format!("cd '{}'", project_path.display())))
        .stderr(contains("uv lock"))
        .stderr(contains("error_code: FORGE_E_ENV"));

    assert!(project_path.join("pyproject.toml").exists());
    assert!(!project_path.join("uv.lock").exists());
    let log = fs::read_to_string(log_path).expect("command log should exist");
    assert!(log.contains("uv lock"));
    assert!(!log.contains("git init"));
    assert!(!log.contains("gh repo create"));
}

#[test]
fn new_github_creation_failure_reports_local_recovery_context() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo infra");
    let bin_dir = temp.path().join("bin");
    fs::create_dir(&bin_dir).expect("fake bin dir should create");
    let log_path = temp.path().join("commands.log");

    write_executable(
        &bin_dir.join("uv"),
        &format!(
            "#!/bin/sh\nprintf 'uv %s\\n' \"$*\" >> {}\nif [ \"$1\" = lock ]; then printf 'version = 1\\n' > uv.lock; fi\n",
            log_path.display()
        ),
    );
    write_executable(
        &bin_dir.join("git"),
        &format!(
            "#!/bin/sh\nprintf 'git %s\\n' \"$*\" >> {}\nexit 0\n",
            log_path.display()
        ),
    );
    write_executable(
        &bin_dir.join("gh"),
        &format!(
            "#!/bin/sh\nprintf 'gh %s\\n' \"$*\" >> {}\ncase \"$1 $2\" in\n  'auth status') exit 0 ;;\n  'repo create') exit 42 ;;\n  *) exit 1 ;;\nesac\n",
            log_path.display()
        ),
    );

    let path = format!(
        "{}:{}",
        bin_dir.display(),
        std::env::var("PATH").expect("PATH should be set")
    );
    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.env("PATH", path);
    cmd.args([
        "new",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--description",
        "Shared repository infrastructure",
        "--github",
        "--github-owner",
        "example-org",
        "--yes",
    ]);

    cmd.assert()
        .failure()
        .stderr(contains(
            "GitHub repository creation failed after local project generation",
        ))
        .stderr(contains(format!("cd '{}'", project_path.display())))
        .stderr(contains("gh repo create example-org/repo-infra"))
        .stderr(contains("error_code: FORGE_E_ENV"));

    assert!(project_path.join("pyproject.toml").exists());
    assert!(project_path.join("uv.lock").exists());
    let log = fs::read_to_string(log_path).expect("command log should exist");
    assert!(log.contains("git commit -m chore: initialize project with forge"));
    assert!(log.contains("gh repo create example-org/repo-infra"));
}

#[test]
fn new_defaults_python_package_name_from_project_name() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-tools");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "python-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-tools",
        "--description",
        "Grid toolchain",
        "--author-name",
        "Ada Lovelace",
        "--author-email",
        "ada@example.com",
        "--yes",
    ]);

    cmd.assert().success();

    let pyproject = fs::read_to_string(project_path.join("pyproject.toml"))
        .expect("pyproject.toml should be generated");
    assert!(pyproject.contains("package_name = \"grid_tools\""));
    assert!(project_path.join("src/grid_tools/__init__.py").exists());
    assert!(project_path.join("tests/test_grid_tools.py").exists());
}

#[test]
fn new_yes_mode_uses_library_defaults_without_requiring_defaulted_fields() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-tools");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "python-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-tools",
        "--description",
        "Grid toolchain",
        "--yes",
    ]);

    cmd.assert().success();

    let pyproject = fs::read_to_string(project_path.join("pyproject.toml"))
        .expect("pyproject.toml should be generated");
    assert!(pyproject.contains("license = \"BSD-3-Clause\""));
    assert!(pyproject.contains("python_min = \"3.11\""));
    assert!(pyproject.contains("requires-python = \">=3.11,<3.15\""));
    assert!(!pyproject.contains("authors ="));
    assert!(!pyproject.contains("author_name ="));
    assert!(!pyproject.contains("author_email ="));
}

#[test]
fn new_explains_invalid_default_python_package_name() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("123-tools");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "python-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "123-tools",
        "--description",
        "Grid toolchain",
        "--author-name",
        "Ada Lovelace",
        "--author-email",
        "ada@example.com",
        "--yes",
    ]);

    cmd.assert()
        .failure()
        .stderr(contains("derived package name '123_tools' is invalid"))
        .stderr(contains("pass --package-name"));
    assert!(!project_path.exists());
}

#[test]
fn new_yes_mode_reports_all_missing_required_fields() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-tools");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "python-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-tools",
        "--yes",
    ]);

    cmd.assert()
        .failure()
        .stderr(contains("missing required options for --yes"))
        .stderr(contains("--description"))
        .stderr(predicates::str::contains("--author-name").not())
        .stderr(predicates::str::contains("--author-email").not())
        .stderr(contains("or run without --yes"));
    assert!(!project_path.exists());
}

#[test]
fn new_json_yes_mode_missing_required_keeps_stdout_empty() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-tools");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "python-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-tools",
        "--yes",
        "--json",
    ]);

    let output = cmd.output().expect("new should run");
    assert!(!output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.is_empty());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("missing required options for --yes"));
    assert!(stderr.contains("--description"));
    assert!(!stderr.contains("--author-name"));
    assert!(!stderr.contains("--author-email"));
    assert!(stderr.contains("error_code: FORGE_E_INPUT"));
    assert!(!project_path.exists());
}

#[test]
fn new_yes_mode_required_field_validation_uses_selected_blueprint() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo-infra");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--yes",
    ]);

    cmd.assert()
        .failure()
        .stderr(contains("missing required options for --yes"))
        .stderr(contains("--description"))
        .stderr(predicates::str::contains("--author-name").not());
    assert!(!project_path.exists());
}

#[test]
fn new_non_tty_dry_run_can_run_without_yes_when_flags_are_explicit() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo-infra");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--description",
        "Shared repository infrastructure",
        "--dry-run",
    ]);

    cmd.assert()
        .success()
        .stdout(contains("Project creation preview"))
        .stdout(contains("repo-infra"))
        .stderr(contains("interactive confirmation requires a terminal").not());
    assert!(!project_path.exists());
}

#[test]
fn new_non_tty_requires_blueprint_when_interactive_setup_is_unavailable() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo-infra");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--description",
        "Shared repository infrastructure",
        "--dry-run",
    ]);

    cmd.assert()
        .failure()
        .stderr(contains(
            "--blueprint is required when interactive setup is unavailable",
        ))
        .stderr(contains("error_code: FORGE_E_INPUT"));
    assert!(!project_path.exists());
}

#[test]
fn new_escapes_toml_metadata_without_losing_user_input() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("quote-tools");

    let description = "Grid \"toolchain\" with \\ paths";
    let author_name = "Ada \"Countess\" Lovelace";

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "python-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "quote-tools",
        "--package-name",
        "quote_tools",
        "--description",
        description,
        "--author-name",
        author_name,
        "--author-email",
        "ada@example.com",
        "--yes",
    ]);

    cmd.assert().success();

    let pyproject = fs::read_to_string(project_path.join("pyproject.toml"))
        .expect("pyproject.toml should be generated");
    let parsed: toml::Value = toml::from_str(&pyproject).expect("pyproject should be valid TOML");

    assert_eq!(
        parsed["project"]["description"]
            .as_str()
            .expect("project description should be a string"),
        description
    );
    assert_eq!(
        parsed["tool"]["forge"]["description"]
            .as_str()
            .expect("forge description should be a string"),
        description
    );
    assert_eq!(
        parsed["tool"]["forge"]["author_name"]
            .as_str()
            .expect("forge author_name should be a string"),
        author_name
    );

    let mut check = Command::cargo_bin("forge").expect("forge binary should build");
    check.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--check",
    ]);
    check.assert().success();
}

#[test]
fn new_escapes_rust_toml_metadata_without_losing_user_input() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("quote-rs");

    let description = "Grid \"toolchain\" with \\ paths";
    let author_name = "Ferris \"Engineer\"";

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "rust-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "quote-rs",
        "--description",
        description,
        "--author-name",
        author_name,
        "--author-email",
        "ferris@example.com",
        "--yes",
    ]);

    cmd.assert().success();

    let cargo_toml =
        fs::read_to_string(project_path.join("Cargo.toml")).expect("Cargo.toml should exist");
    let cargo: toml::Value = toml::from_str(&cargo_toml).expect("Cargo.toml should be valid TOML");
    assert_eq!(
        cargo["package"]["description"]
            .as_str()
            .expect("package description should be a string"),
        description
    );
    assert!(
        cargo["package"]["authors"]
            .as_array()
            .expect("authors should be an array")
            .iter()
            .any(|author| author.as_str() == Some(&format!("{author_name} <ferris@example.com>")))
    );

    let pyproject = fs::read_to_string(project_path.join("pyproject.toml"))
        .expect("pyproject.toml should be generated");
    let parsed: toml::Value = toml::from_str(&pyproject).expect("pyproject should be valid TOML");
    assert_eq!(
        parsed["tool"]["forge"]["description"]
            .as_str()
            .expect("forge description should be a string"),
        description
    );
    assert_eq!(
        parsed["tool"]["forge"]["author_name"]
            .as_str()
            .expect("forge author_name should be a string"),
        author_name
    );

    let mut check = Command::cargo_bin("forge").expect("forge binary should build");
    check.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--check",
    ]);
    check.assert().success();
}

#[test]
fn new_can_generate_prettier_component() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-tools");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "python-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-tools",
        "--package-name",
        "grid_tools",
        "--description",
        "Grid toolchain",
        "--author-name",
        "Ada Lovelace",
        "--author-email",
        "ada@example.com",
        "--prettier",
        "--yes",
    ]);

    cmd.assert().success();

    let pyproject = fs::read_to_string(project_path.join("pyproject.toml"))
        .expect("pyproject.toml should be generated");
    assert!(pyproject.contains("prettier = true"));

    let precommit = fs::read_to_string(project_path.join(".pre-commit-config.yaml"))
        .expect("pre-commit config should be generated");
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
fn new_accepts_explicit_false_for_prettier_component() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-tools");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "python-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-tools",
        "--package-name",
        "grid_tools",
        "--description",
        "Grid toolchain",
        "--author-name",
        "Ada Lovelace",
        "--author-email",
        "ada@example.com",
        "--prettier=false",
        "--yes",
    ]);

    cmd.assert().success();

    let pyproject = fs::read_to_string(project_path.join("pyproject.toml"))
        .expect("pyproject.toml should be generated");
    assert!(pyproject.contains("prettier = false"));
    assert!(!project_path.join(".prettierrc.json").exists());
    assert!(!project_path.join(".prettierignore").exists());
}

#[test]
fn new_can_generate_editorconfig_component() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-tools");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "python-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-tools",
        "--package-name",
        "grid_tools",
        "--description",
        "Grid toolchain",
        "--author-name",
        "Ada Lovelace",
        "--author-email",
        "ada@example.com",
        "--editorconfig",
        "--yes",
    ]);

    cmd.assert().success();

    let pyproject = fs::read_to_string(project_path.join("pyproject.toml"))
        .expect("pyproject.toml should be generated");
    assert!(pyproject.contains("editorconfig = true"));
    assert!(project_path.join(".editorconfig").exists());
}

#[test]
fn new_can_generate_markdownlint_component() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-tools");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "python-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-tools",
        "--package-name",
        "grid_tools",
        "--description",
        "Grid toolchain",
        "--author-name",
        "Ada Lovelace",
        "--author-email",
        "ada@example.com",
        "--markdownlint",
        "--yes",
    ]);

    cmd.assert().success();

    let pyproject = fs::read_to_string(project_path.join("pyproject.toml"))
        .expect("pyproject.toml should be generated");
    assert!(pyproject.contains("markdownlint = true"));
    assert!(project_path.join(".markdownlint.jsonc").exists());

    let precommit = fs::read_to_string(project_path.join(".pre-commit-config.yaml"))
        .expect("pre-commit config should be generated");
    assert!(precommit.contains("id: markdownlint"));
    assert!(precommit.contains("entry: npx --yes markdownlint-cli2@0.18.1"));

    let justfile =
        fs::read_to_string(project_path.join("justfile")).expect("justfile should exist");
    assert!(justfile.contains("npx --yes markdownlint-cli2@0.18.1 --fix \"**/*.md\""));
}

#[test]
fn new_accepts_value_less_optional_workflow_flags() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-tools");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "python-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-tools",
        "--package-name",
        "grid_tools",
        "--description",
        "Grid toolchain",
        "--author-name",
        "Ada Lovelace",
        "--author-email",
        "ada@example.com",
        "--pypi-publish",
        "--yes",
    ]);

    cmd.assert().success();

    let pyproject = fs::read_to_string(project_path.join("pyproject.toml"))
        .expect("pyproject.toml should be generated");
    assert!(pyproject.contains("pypi-publish = true"));
    assert!(
        project_path
            .join(".github/workflows/publish-pypi.yaml")
            .exists()
    );
    let publish_pypi = fs::read_to_string(project_path.join(".github/workflows/publish-pypi.yaml"))
        .expect("PyPI publish workflow should be generated");
    assert!(publish_pypi.contains(
        "concurrency:\n  group: ${{ github.workflow }}-${{ github.event.release.id }}\n  cancel-in-progress: false\n\njobs:"
    ));
    assert!(publish_pypi.contains("    environment:\n      name: pypi\n"));
    assert!(publish_pypi.contains("      url: https://pypi.org/p/<your-pypi-project-name>\n"));
    assert!(
        publish_pypi.contains("    permissions:\n      id-token: write\n      contents: read\n")
    );
    assert!(publish_pypi.contains(
        "# Register this workflow as a trusted publisher in PyPI before uncommenting the publish step."
    ));
    assert!(publish_pypi.contains("# - name: Publish package distributions to PyPI"));
    assert!(publish_pypi.contains("#   uses: pypa/gh-action-pypi-publish@release/v1"));
    assert!(!publish_pypi.contains("\npermissions:\n  id-token: write\n"));
}

#[test]
fn new_dry_run_warns_when_pypi_publishing_is_selected() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-tools");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "python-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-tools",
        "--package-name",
        "grid_tools",
        "--description",
        "Grid toolchain",
        "--author-name",
        "Ada Lovelace",
        "--author-email",
        "ada@example.com",
        "--pypi-publish",
        "--yes",
        "--dry-run",
    ]);

    let output = cmd.output().expect("new dry-run should run");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains("Project creation preview"));
    assert!(stdout.contains("pypi"));
    assert!(stdout.contains(
        "Register this workflow as a trusted publisher in PyPI before uncommenting the publish step."
    ));
}

#[test]
fn new_accepts_explicit_false_flags_for_default_enabled_components() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-tools");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "python-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-tools",
        "--package-name",
        "grid_tools",
        "--description",
        "Grid toolchain",
        "--author-name",
        "Ada Lovelace",
        "--author-email",
        "ada@example.com",
        "--docs=false",
        "--codecov=false",
        "--yes",
    ]);

    cmd.assert().success();

    let pyproject = fs::read_to_string(project_path.join("pyproject.toml"))
        .expect("pyproject.toml should be generated");
    assert!(pyproject.contains("docs = false"));
    assert!(pyproject.contains("codecov = false"));
    assert!(!pyproject.contains("@astrojs/starlight"));

    let justfile =
        fs::read_to_string(project_path.join("justfile")).expect("justfile should exist");
    assert!(!justfile.contains("\ndocs:\n"));
    assert!(!justfile.contains("npm run dev"));

    let ci = fs::read_to_string(project_path.join(".github/workflows/ci.yaml"))
        .expect("CI workflow should be generated");
    assert!(!ci.contains("codecov/codecov-action"));
    assert!(!project_path.join("docs/package.json").exists());
    assert!(
        !project_path
            .join("docs/src/content/docs/index.mdx")
            .exists()
    );
}

#[test]
fn new_rejects_python_only_managed_options_for_rust_blueprint() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-rs");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
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
        "--codecov",
        "--yes",
    ]);

    cmd.assert().failure().stderr(contains(
        "option 'codecov' is not supported by rust-library",
    ));
    assert!(!project_path.exists());
}

#[test]
fn new_rejects_python_only_managed_options_for_any_project_blueprint() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo-infra");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--description",
        "Shared repository infrastructure",
        "--pypi-publish",
        "--yes",
    ]);

    cmd.assert().failure().stderr(contains(
        "option 'pypi-publish' is not supported by any-project",
    ));
    assert!(!project_path.exists());
}

#[test]
fn new_rejects_language_specific_options_for_any_project_blueprint() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo-infra");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--package-name",
        "repo_infra",
        "--description",
        "Shared repository infrastructure",
        "--yes",
    ]);

    cmd.assert().failure().stderr(contains(
        "option 'package-name' is not supported by any-project",
    ));
    assert!(!project_path.exists());
}

#[test]
fn new_json_rejects_unsupported_option_keeps_stdout_empty() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo-infra");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--package-name",
        "repo_infra",
        "--description",
        "Shared repository infrastructure",
        "--yes",
        "--json",
    ]);

    let output = cmd.output().expect("new should run");
    assert!(!output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.is_empty());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("option 'package-name' is not supported by any-project"));
    assert!(!project_path.exists());
}

#[test]
fn new_file_path_explains_destination_directory_requirement() {
    let temp = TempDir::new().expect("temp dir should create");
    let destination_file = temp.path().join("not-a-directory");
    fs::write(&destination_file, "existing").expect("destination file should write");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "any-project",
        "--path",
        destination_file.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--description",
        "Shared repository infrastructure",
        "--yes",
    ]);

    cmd.assert()
        .failure()
        .stderr(contains("destination path is not a directory"))
        .stderr(contains(destination_file.display().to_string()))
        .stderr(contains("error_code: FORGE_E_INPUT"));
}

#[test]
fn new_json_file_path_keeps_stdout_empty() {
    let temp = TempDir::new().expect("temp dir should create");
    let destination_file = temp.path().join("not-a-directory");
    fs::write(&destination_file, "existing").expect("destination file should write");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "any-project",
        "--path",
        destination_file.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--description",
        "Shared repository infrastructure",
        "--yes",
        "--json",
    ]);

    let output = cmd.output().expect("new should run");
    assert!(!output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.is_empty());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("destination path is not a directory"));
    assert!(stderr.contains("error_code: FORGE_E_INPUT"));
}

#[test]
fn new_rejects_python_version_for_rust_blueprint() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-rs");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
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
        "--python-min",
        "3.12",
        "--yes",
    ]);

    cmd.assert().failure().stderr(contains(
        "option 'python-min' is not supported by rust-library",
    ));
    assert!(!project_path.exists());
}

#[test]
fn new_rejects_invalid_python_min_before_writing_files() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-tools");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "python-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-tools",
        "--package-name",
        "grid_tools",
        "--description",
        "Grid toolchain",
        "--author-name",
        "Ada Lovelace",
        "--author-email",
        "ada@example.com",
        "--python-min",
        "3.11\n3.12",
        "--yes",
    ]);

    cmd.assert()
        .failure()
        .stderr(contains("python-min must be between 3.8 and 3.14"));
    assert!(!project_path.exists());
}

#[test]
fn new_rejects_python_min_that_conflicts_with_generated_upper_bound() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-tools");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "python-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-tools",
        "--package-name",
        "grid_tools",
        "--description",
        "Grid toolchain",
        "--author-name",
        "Ada Lovelace",
        "--author-email",
        "ada@example.com",
        "--python-min",
        "3.15",
        "--yes",
    ]);

    cmd.assert()
        .failure()
        .stderr(contains("python-min must be between 3.8 and 3.14"));
    assert!(!project_path.exists());
}

#[test]
fn new_generates_python_ci_matrix_without_versions_below_python_min() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-tools");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "python-library",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "grid-tools",
        "--package-name",
        "grid_tools",
        "--description",
        "Grid toolchain",
        "--author-name",
        "Ada Lovelace",
        "--author-email",
        "ada@example.com",
        "--python-min",
        "3.14",
        "--yes",
    ]);

    cmd.assert().success();

    let workflow = fs::read_to_string(project_path.join(".github/workflows/ci.yaml"))
        .expect("CI workflow should exist");
    assert!(workflow.contains("python-version: [\"3.14\"]"));
    assert!(!workflow.contains("\"3.12\""));
    assert!(!workflow.contains("\"3.13\""));
}

#[test]
fn new_rejects_github_owner_without_github_creation() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo-infra");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--description",
        "Shared repository infrastructure",
        "--github-owner",
        "example-org",
        "--yes",
    ]);

    cmd.assert()
        .failure()
        .stderr(contains("option 'github-owner' requires --github"));
    assert!(!project_path.exists());
}

#[test]
fn new_rejects_github_visibility_without_github_creation() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo-infra");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
        "--blueprint",
        "any-project",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--project-name",
        "repo-infra",
        "--description",
        "Shared repository infrastructure",
        "--github-visibility",
        "private",
        "--yes",
    ]);

    cmd.assert()
        .failure()
        .stderr(contains("option 'github-visibility' requires --github"));
    assert!(!project_path.exists());
}

#[test]
fn new_generates_language_agnostic_infra_project() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo-infra");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
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
        .stdout(contains("Project created"))
        .stdout(contains("blueprint: any-project"));

    let pyproject = fs::read_to_string(project_path.join("pyproject.toml"))
        .expect("pyproject.toml should be generated");
    assert!(pyproject.contains("[project]"));
    assert!(pyproject.contains("requires-python = \">=3.11\""));
    assert!(pyproject.contains("blueprint = \"any-project\""));
    assert!(pyproject.contains("prettier = true"));

    assert!(project_path.join("AGENTS.md").exists());
    assert_eq!(
        fs::read_link(project_path.join("CLAUDE.md")).expect("CLAUDE.md should be a symlink"),
        std::path::PathBuf::from("AGENTS.md")
    );
    let ci = fs::read_to_string(project_path.join(".github/workflows/ci.yaml"))
        .expect("CI workflow should be generated");
    assert!(ci.contains("permissions:\n  contents: read\n\njobs:"));
    assert!(ci.contains("forge update --path . --check"));
    let precommit = fs::read_to_string(project_path.join(".pre-commit-config.yaml"))
        .expect("pre-commit config should be generated");
    assert!(precommit.contains(r#"-not -path "./.venv/*""#));
    let justfile =
        fs::read_to_string(project_path.join("justfile")).expect("justfile should exist");
    assert!(justfile.contains("uv run --locked prek run --all-files"));
    assert!(justfile.contains("uv lock --check"));
    assert!(justfile.contains("forge update --path . --check"));
    assert!(project_path.join(".prettierrc.json").exists());
    assert!(project_path.join("docs/package.json").exists());
    assert!(
        project_path
            .join("docs/src/content/docs/index.mdx")
            .exists()
    );
    assert!(!project_path.join("src").exists());

    let mut check = Command::cargo_bin("forge").expect("forge binary should build");
    check.args([
        "update",
        "--yes",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
        "--check",
    ]);
    check
        .assert()
        .success()
        .stdout(contains("changes: 0"))
        .stdout(contains("managed infrastructure is current"));
}

#[test]
fn new_generates_rust_library_project() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-rs");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
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
        "--license",
        "MIT",
        "--yes",
    ]);

    cmd.assert()
        .success()
        .stdout(contains("Project created"))
        .stdout(contains("blueprint: rust-library"));

    let forge_metadata = fs::read_to_string(project_path.join("pyproject.toml"))
        .expect("pyproject.toml should be generated");
    assert!(forge_metadata.contains("[project]"));
    assert!(forge_metadata.contains("requires-python = \">=3.11\""));
    assert!(forge_metadata.contains("blueprint = \"rust-library\""));
    assert!(forge_metadata.contains("crate_name = \"grid_rs\""));

    let cargo_toml =
        fs::read_to_string(project_path.join("Cargo.toml")).expect("Cargo.toml should exist");
    assert!(cargo_toml.contains("name = \"grid-rs\""));
    assert!(cargo_toml.contains("edition = \"2024\""));
    assert!(cargo_toml.contains("name = \"grid_rs\""));

    let precommit = fs::read_to_string(project_path.join(".pre-commit-config.yaml"))
        .expect("pre-commit config should be generated");
    assert!(precommit.contains("cargo fmt"));
    assert!(precommit.contains("cargo clippy"));
    let justfile =
        fs::read_to_string(project_path.join("justfile")).expect("justfile should exist");
    assert!(justfile.contains("cargo fmt --all --check"));
    assert!(justfile.contains("uv lock --check"));
    assert!(
        justfile.contains("cargo clippy --workspace --all-targets --all-features -- -D warnings")
    );
    assert!(justfile.contains("uv run --locked prek run --all-files"));
    assert!(justfile.contains("forge update --path . --check"));

    assert!(project_path.join("src/lib.rs").exists());
    let ci = fs::read_to_string(project_path.join(".github/workflows/ci.yaml"))
        .expect("CI workflow should be generated");
    assert!(ci.contains("permissions:\n  contents: read\n\njobs:"));
    assert!(ci.contains("uv lock --check"));
    assert!(ci.contains("cargo fmt --all --check"));
    assert!(ci.contains("cargo clippy --workspace --all-targets --all-features -- -D warnings"));
    assert!(ci.contains("forge update --path . --check"));
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
}
