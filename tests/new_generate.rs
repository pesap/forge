use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

#[test]
fn new_generates_python_project_with_metadata() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-tools");

    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
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

    cmd.assert().success();

    let pyproject = fs::read_to_string(project_path.join("pyproject.toml"))
        .expect("pyproject.toml should be generated");
    assert!(pyproject.contains("[tool.forge]"));
    assert!(pyproject.contains("blueprint = \"python-library\""));
    assert!(pyproject.contains("project_name = \"grid-tools\""));

    assert!(project_path.join("src/grid_tools/__init__.py").exists());
    assert!(project_path.join(".github/workflows/ci.yaml").exists());
}
