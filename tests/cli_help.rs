use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn top_level_help_lists_expected_commands() {
    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");

    cmd.arg("--help")
        .assert()
        .success()
        .stdout(contains("new"))
        .stdout(contains("upgrade"))
        .stdout(contains("self"));
}

#[test]
fn self_update_help_is_exposed() {
    let mut cmd = Command::cargo_bin("forge").expect("forge binary should build");

    cmd.args(["self", "update", "--help"])
        .assert()
        .success()
        .stdout(contains("Update forge itself"));
}
