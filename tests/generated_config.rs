use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use yaml_rust2::{Yaml, YamlLoader};

#[test]
fn generated_python_project_configs_are_structurally_valid() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("grid-tools");

    generate_project(
        &project_path,
        &[
            "--blueprint",
            "python-library",
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
            "3.12",
            "--pypi-publish",
        ],
    );

    assert_toml_file(project_path.join("pyproject.toml"));
    assert_yaml_file(project_path.join(".pre-commit-config.yaml"), &["repos"]);
    assert_yaml_file(
        project_path.join(".github/workflows/ci.yaml"),
        &["name", "on", "jobs"],
    );
    assert_yaml_file(
        project_path.join(".github/workflows/forge-sync.yaml"),
        &["name", "on", "jobs"],
    );
    assert_yaml_file(
        project_path.join(".github/workflows/release-please.yaml"),
        &["name", "on", "jobs"],
    );
    assert_json_file(
        project_path.join("docs/package.json"),
        &["name", "scripts", "dependencies"],
    );
    assert_file_contains(
        project_path.join("docs/astro.config.mjs"),
        &["starlight(", "title:", "head: []"],
    );
    assert_file_contains(
        project_path.join("docs/src/content.config.ts"),
        &[
            "docsLoader",
            "docsSchema",
            "defineCollection",
            "export const collections",
        ],
    );
    assert_file_contains(
        project_path.join("docs/src/content/docs/index.mdx"),
        &["---\ntitle: grid-tools", "head: []"],
    );
    assert_json_file(
        project_path.join(".release-please-config.json"),
        &["release-type", "packages"],
    );
    assert_json_file(project_path.join(".release-please-manifest.json"), &["."]);
    assert_update_check_is_current(&project_path);
}

#[test]
fn generated_rust_project_configs_are_structurally_valid() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("rust-tools");

    generate_project(
        &project_path,
        &[
            "--blueprint",
            "rust-library",
            "--project-name",
            "rust-tools",
            "--package-name",
            "rust_tools",
            "--description",
            "Rust toolchain",
            "--author-name",
            "Ada Lovelace",
            "--author-email",
            "ada@example.com",
            "--prettier",
        ],
    );

    assert_toml_file(project_path.join("pyproject.toml"));
    assert_toml_file(project_path.join("Cargo.toml"));
    assert_yaml_file(project_path.join(".pre-commit-config.yaml"), &["repos"]);
    assert_yaml_file(
        project_path.join(".github/workflows/ci.yaml"),
        &["name", "on", "jobs"],
    );
    assert_yaml_file(
        project_path.join(".github/workflows/forge-sync.yaml"),
        &["name", "on", "jobs"],
    );
    assert_json_file(
        project_path.join("docs/package.json"),
        &["name", "scripts", "dependencies"],
    );
    assert_file_contains(
        project_path.join("docs/astro.config.mjs"),
        &["starlight(", "title:", "head: []"],
    );
    assert_json_file(
        project_path.join(".prettierrc.json"),
        &["printWidth", "proseWrap", "singleQuote"],
    );
    assert_file_contains(
        project_path.join(".prettierignore"),
        &["dist/", "build/", "site/", ".venv/", ".coverage", "uv.lock"],
    );
    assert_update_check_is_current(&project_path);
}

#[test]
fn generated_any_project_configs_are_structurally_valid() {
    let temp = TempDir::new().expect("temp dir should create");
    let project_path = temp.path().join("repo-infra");

    generate_project(
        &project_path,
        &[
            "--blueprint",
            "any-project",
            "--project-name",
            "repo-infra",
            "--description",
            "Shared repo infrastructure",
            "--prettier",
        ],
    );

    assert_toml_file(project_path.join("pyproject.toml"));
    assert_yaml_file(project_path.join(".pre-commit-config.yaml"), &["repos"]);
    assert_yaml_file(
        project_path.join(".github/workflows/ci.yaml"),
        &["name", "on", "jobs"],
    );
    assert_yaml_file(
        project_path.join(".github/workflows/forge-sync.yaml"),
        &["name", "on", "jobs"],
    );
    assert_json_file(
        project_path.join("docs/package.json"),
        &["name", "scripts", "dependencies"],
    );
    assert_file_contains(
        project_path.join("docs/astro.config.mjs"),
        &["starlight(", "title:", "head: []"],
    );
    assert_json_file(
        project_path.join(".prettierrc.json"),
        &["printWidth", "proseWrap", "singleQuote"],
    );
    assert_file_contains(
        project_path.join(".prettierignore"),
        &["dist/", "build/", "site/", ".venv/", ".coverage", "uv.lock"],
    );
    assert_update_check_is_current(&project_path);
}

fn generate_project(project_path: &Path, args: &[&str]) {
    let mut command = Command::cargo_bin("forge").expect("forge binary should build");
    command
        .arg("init")
        .arg("--path")
        .arg(project_path)
        .args(args)
        .arg("--yes");

    command.assert().success();
}

fn assert_toml_file(path: PathBuf) {
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} should be readable: {error}", path.display()));
    toml::from_str::<toml::Value>(&content)
        .unwrap_or_else(|error| panic!("{} should parse as TOML: {error}", path.display()));
}

fn assert_json_file(path: PathBuf, required_keys: &[&str]) {
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} should be readable: {error}", path.display()));
    let value: Value = serde_json::from_str(&content)
        .unwrap_or_else(|error| panic!("{} should parse as JSON: {error}", path.display()));
    let object = value
        .as_object()
        .unwrap_or_else(|| panic!("{} should contain a JSON object", path.display()));

    for key in required_keys {
        assert!(
            object.contains_key(*key),
            "{} should contain top-level JSON key {key}",
            path.display()
        );
    }
}

fn assert_file_contains(path: PathBuf, snippets: &[&str]) {
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} should be readable: {error}", path.display()));
    for snippet in snippets {
        assert!(
            content.contains(snippet),
            "{} should contain {snippet:?}",
            path.display()
        );
    }
}

fn assert_update_check_is_current(project_path: &Path) {
    let mut command = Command::cargo_bin("forge").expect("forge binary should build");
    command
        .arg("sync")
        .arg("--path")
        .arg(project_path)
        .arg("--check")
        .assert()
        .success();
}

fn assert_yaml_file(path: PathBuf, required_keys: &[&str]) {
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} should be readable: {error}", path.display()));
    let documents = YamlLoader::load_from_str(&content)
        .unwrap_or_else(|error| panic!("{} should parse as YAML: {error}", path.display()));
    assert_eq!(
        documents.len(),
        1,
        "{} should contain one YAML document",
        path.display()
    );

    let root = documents
        .first()
        .expect("one YAML document should have a root value");
    for key in required_keys {
        assert!(
            yaml_mapping_contains_key(root, key),
            "{} should contain top-level YAML key {key}",
            path.display()
        );
    }
}

fn yaml_mapping_contains_key(value: &Yaml, key: &str) -> bool {
    value.as_hash().is_some_and(|mapping| {
        mapping
            .keys()
            .any(|candidate| candidate.as_str() == Some(key))
    })
}
