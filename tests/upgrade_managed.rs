use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn generate_project(project_path: &std::path::Path) {
    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");
    cmd.args([
        "new",
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

#[test]
fn upgrade_only_rewrites_managed_infra_files() {
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

    let mut upgrade = Command::cargo_bin("forge").expect("forge binary should build");
    upgrade.args([
        "upgrade",
        "--path",
        project_path.to_str().expect("valid UTF-8 path"),
    ]);
    upgrade.assert().success();

    let src_after = fs::read_to_string(src_file).expect("source should remain readable");
    assert!(src_after.contains("custom user code"));

    let just_after = fs::read_to_string(justfile).expect("justfile should remain readable");
    assert!(!just_after.contains("BROKEN"));
    assert!(just_after.contains("verify"));
}
