use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::errors::ErrorCode;
use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::blueprint::agents;
use crate::blueprint::components::{ComponentSelection, ManagedComponent};
use crate::blueprint::files::{GeneratedFile, GeneratedFiles, remove_managed_file_if_exists};
use crate::blueprint::gitattributes;
use crate::blueprint::github_actions;
use crate::blueprint::precommit;
use crate::blueprint::readme;
use crate::blueprint::template_engine;
use crate::blueprint::toml_value;
use crate::blueprint::{
    BlueprintName, BlueprintSpec, ManagedOption, apply_ignored_files, is_supported_license,
    managed_option_enabled, render_forge_ignore, render_forge_overrides_table,
    supported_license_message, validate_managed_overrides_from_metadata,
};

pub const BLUEPRINT_NAME: &str = "rust-library";
pub const BLUEPRINT_VERSION: &str = "0.1.0";

#[derive(Clone, Debug)]
pub struct ProjectConfig {
    pub project_name: String,
    pub crate_name: String,
    pub description: String,
    pub author_name: Option<String>,
    pub author_email: Option<String>,
    pub license: String,
    pub rust_edition: String,
    pub docs: bool,
    pub rust_rules: bool,
    pub components: ComponentSelection,
    pub ignored_files: Vec<String>,
}

impl ProjectConfig {
    pub fn validate(&self) -> Result<()> {
        if !is_valid_package_name(&self.project_name) {
            bail!("invalid Rust package name: {}", self.project_name);
        }
        if !is_valid_crate_name(&self.crate_name) {
            bail!("invalid Rust crate name: {}", self.crate_name);
        }
        if self.description.trim().is_empty() {
            bail!("description cannot be empty");
        }
        if let Some(author_name) = &self.author_name
            && author_name.trim().is_empty()
        {
            bail!("author name cannot be empty");
        }
        if let Some(author_email) = &self.author_email
            && !author_email.contains('@')
        {
            bail!("invalid author email: {}", author_email);
        }
        if !is_supported_license(&self.license) {
            bail!(supported_license_message());
        }
        if !matches!(self.rust_edition.as_str(), "2021" | "2024") {
            bail!("rust edition must be 2021 or 2024");
        }
        Ok(())
    }
}

pub fn default_crate_name(project_name: &str) -> String {
    project_name.replace('-', "_")
}

pub fn is_valid_package_name(value: &str) -> bool {
    let valid_chars = value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'));
    !value.is_empty() && !value.starts_with('-') && valid_chars
}

pub fn is_valid_crate_name(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

pub fn render_project_files(config: &ProjectConfig) -> GeneratedFiles {
    let mut files = render_managed_files(config);
    files.insert(
        PathBuf::from("src/lib.rs"),
        GeneratedFile::text(render_lib_rs(config)),
    );
    files
}

pub fn render_managed_files(config: &ProjectConfig) -> GeneratedFiles {
    let mut files = GeneratedFiles::new();

    files.insert(
        PathBuf::from("README.md"),
        GeneratedFile::text(render_readme(config)),
    );
    files.insert(
        PathBuf::from("LICENSE"),
        GeneratedFile::text(render_license(config)),
    );
    files.insert(
        PathBuf::from(".gitignore"),
        GeneratedFile::text(render_gitignore()),
    );
    files.insert(
        PathBuf::from(".gitattributes"),
        GeneratedFile::text(gitattributes::render_line_ending_policy()),
    );
    files.insert(
        PathBuf::from("Cargo.toml"),
        GeneratedFile::text(render_cargo_toml(config)),
    );
    files.insert(
        PathBuf::from("pyproject.toml"),
        GeneratedFile::text(render_pyproject(config)),
    );
    files.insert(
        PathBuf::from("justfile"),
        GeneratedFile::text(render_justfile(config)),
    );
    files.insert(
        PathBuf::from(".pre-commit-config.yaml"),
        GeneratedFile::text(render_precommit_config(config)),
    );
    files.extend(agents::render_agent_files(&[
        "Run `cargo fmt --all`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, and `cargo test` before handoff.",
        "Preserve user-authored Rust source during managed infrastructure syncs.",
    ]));
    files.insert(
        PathBuf::from(".github/workflows/ci.yaml"),
        GeneratedFile::text(render_ci_workflow()),
    );
    files.insert(
        PathBuf::from(".github/workflows/forge-sync.yaml"),
        GeneratedFile::text(github_actions::render_forge_sync_workflow()),
    );
    if config.docs {
        files.insert(
            PathBuf::from("docs/package.json"),
            GeneratedFile::text(render_docs_package_json(config)),
        );
        files.insert(
            PathBuf::from("docs/astro.config.mjs"),
            GeneratedFile::text(render_docs_astro_config()),
        );
        files.insert(
            PathBuf::from("docs/tsconfig.json"),
            GeneratedFile::text(render_docs_tsconfig()),
        );
        files.insert(
            PathBuf::from("docs/src/content/docs/index.mdx"),
            GeneratedFile::text(render_docs_index(config)),
        );
    }
    files.extend(config.components.render_files());
    apply_ignored_files(&mut files, &config.ignored_files);

    files
}

pub fn render_managed_files_from_pyproject(content: &str) -> Result<GeneratedFiles> {
    let config = config_from_pyproject(content)?;
    Ok(render_managed_files(&config))
}

pub fn clean_optional_files(root: &Path, config: &ProjectConfig) -> Result<()> {
    for file in optional_cleanup_paths(config) {
        remove_if_exists(&root.join(file))?;
    }

    Ok(())
}

pub fn clean_optional_files_from_pyproject(root: &Path, content: &str) -> Result<()> {
    let config = config_from_pyproject(content)?;
    clean_optional_files(root, &config)
}

pub fn optional_cleanup_paths(config: &ProjectConfig) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if !config.docs {
        files.extend(
            [
                "docs/package.json",
                "docs/astro.config.mjs",
                "docs/tsconfig.json",
                "docs/src/content/docs/index.mdx",
            ]
            .into_iter()
            .map(PathBuf::from),
        );
    }
    files.extend(config.components.disabled_file_paths());
    files
}

pub fn optional_cleanup_paths_from_pyproject(content: &str) -> Result<Vec<PathBuf>> {
    let config = config_from_pyproject(content)?;
    Ok(optional_cleanup_paths(&config))
}

fn render_readme(config: &ProjectConfig) -> String {
    template_engine::render_template(
        "rust_library/readme.md.j2",
        serde_json::json!({"project_name": config.project_name, "description": config.description, "automated_update_section": readme::automated_update_section(), "blueprint_name": BLUEPRINT_NAME}),
    )
}

fn render_license(config: &ProjectConfig) -> String {
    let template = match config.license.as_str() {
        "MIT" => "rust_library/mit-license.j2",
        "Apache-2.0" => "rust_library/apache-license.j2",
        "BSD-2-Clause" => "rust_library/bsd-2-clause-license.j2",
        "ISC" => "rust_library/isc-license.j2",
        _ => "rust_library/bsd-license.j2",
    };
    template_engine::render_template(
        template,
        serde_json::json!({"year": "2026", "author": author_display_name(config)}),
    )
}

fn render_gitignore() -> String {
    template_engine::render_template("rust_library/gitignore.j2", ())
}

fn render_cargo_authors(config: &ProjectConfig) -> String {
    match (&config.author_name, &config.author_email) {
        (Some(author_name), Some(author_email)) => {
            toml_value::string_literal(&format!("{author_name} <{author_email}>"))
        }
        (Some(author_name), None) => toml_value::string_literal(author_name),
        (None, Some(author_email)) => toml_value::string_literal(author_email),
        (None, None) => String::new(),
    }
}

fn render_optional_forge_field(name: &str, value: &Option<String>) -> String {
    value
        .as_ref()
        .map(|value| format!("{name} = {}\n", toml_value::string_literal(value)))
        .unwrap_or_default()
}

fn author_display_name(config: &ProjectConfig) -> String {
    config
        .author_name
        .clone()
        .or_else(|| config.author_email.clone())
        .unwrap_or_else(|| "the authors".to_string())
}

fn render_cargo_toml(config: &ProjectConfig) -> String {
    template_engine::render_template(
        "rust_library/cargo.toml.j2",
        serde_json::json!({"project_name": toml_value::string_literal(&config.project_name), "rust_edition": toml_value::string_literal(&config.rust_edition), "description": toml_value::string_literal(&config.description), "license": toml_value::string_literal(&config.license), "cargo_authors": render_cargo_authors(config), "crate_name": toml_value::string_literal(&config.crate_name)}),
    )
}

fn render_pyproject(config: &ProjectConfig) -> String {
    template_engine::render_template(
        "rust_library/pyproject.toml.j2",
        serde_json::json!({
            "blueprint_name": BLUEPRINT_NAME,
            "blueprint_version": BLUEPRINT_VERSION,
            "project_name": toml_value::string_literal(&config.project_name),
            "crate_name": toml_value::string_literal(&config.crate_name),
            "description": toml_value::string_literal(&config.description),
            "author_name": render_optional_forge_field("author_name", &config.author_name),
            "author_email": render_optional_forge_field("author_email", &config.author_email),
            "license": toml_value::string_literal(&config.license),
            "rust_edition": toml_value::string_literal(&config.rust_edition),
            "docs_group": render_docs_dependency_group(config.docs),
            "forge_ignore": render_forge_ignore(&config.ignored_files),
            "forge_overrides": render_forge_overrides_table(
                BlueprintName::RustLibrary,
                &[
                    (ManagedOption::Docs, config.docs),
                    (ManagedOption::RustRules, config.rust_rules),
                    (
                        ManagedOption::Prettier,
                        config.components.is_enabled(ManagedComponent::Prettier),
                    ),
                    (
                        ManagedOption::Editorconfig,
                        config.components.is_enabled(ManagedComponent::Editorconfig),
                    ),
                    (
                        ManagedOption::Markdownlint,
                        config.components.is_enabled(ManagedComponent::Markdownlint),
                    ),
                ],
            )
        }),
    )
}

fn render_docs_dependency_group(_enabled: bool) -> &'static str {
    ""
}

fn render_justfile(config: &ProjectConfig) -> String {
    let mut justfile = template_engine::render_template(
        "rust_library/justfile.j2",
        serde_json::json!({"docs_recipe": if config.docs {"docs:\n    cd docs && npm install\n    cd docs && npm run dev\n"} else {""}, "component_format_steps": render_component_format_steps(config)}),
    );
    justfile.push('\n');
    justfile
}

fn render_component_format_steps(config: &ProjectConfig) -> String {
    let commands = config.components.format_commands();
    if commands.is_empty() {
        return "\n".to_string();
    }

    let mut output = commands
        .into_iter()
        .map(|command| format!("    {command}\n"))
        .collect::<String>();
    output.push('\n');
    output
}

fn render_precommit_config(config: &ProjectConfig) -> String {
    template_engine::render_template(
        "rust_library/pre-commit-config.yaml.j2",
        serde_json::json!({"component_hooks": config.components.pre_commit_hooks(), "install_commit_msg_hook": false, "rust_rules": config.rust_rules, "uv_lock_hook": precommit::uv_lock_hook()}),
    )
}

fn render_ci_workflow() -> String {
    template_engine::render_template(
        "rust_library/ci.yaml.j2",
        serde_json::json!({"cancel_redundant_ci_concurrency": github_actions::cancel_redundant_ci_concurrency(), "read_only_permissions": github_actions::read_only_permissions(), "job_timeout": github_actions::job_timeout(), "read_only_checkout_step": github_actions::read_only_checkout_step(), "setup_uv_step": github_actions::setup_uv_step(), "install_forge_step": github_actions::install_forge_step(), "uv_sync_locked_step": github_actions::uv_sync_locked_step(), "uv_lock_check_step": github_actions::uv_lock_check_step(), "prek_step": github_actions::uv_run_locked_step("prek run --all-files"), "forge_sync_check_step": github_actions::forge_sync_check_step()}),
    )
}

fn render_lib_rs(config: &ProjectConfig) -> String {
    template_engine::render_template(
        "rust_library/lib.rs.j2",
        serde_json::json!({"crate_name": config.crate_name}),
    )
}

fn render_docs_package_json(config: &ProjectConfig) -> String {
    template_engine::render_template(
        "rust_library/docs-package.json.j2",
        serde_json::json!({"project_name": config.project_name}),
    )
}

fn render_docs_astro_config() -> String {
    template_engine::render_template("rust_library/docs-astro.config.mjs.j2", ())
}

fn render_docs_tsconfig() -> String {
    template_engine::render_template("rust_library/docs-tsconfig.json.j2", ())
}

fn render_docs_index(config: &ProjectConfig) -> String {
    template_engine::render_template(
        "rust_library/docs-index.mdx.j2",
        serde_json::json!({"project_name": config.project_name, "description": config.description}),
    )
}

#[derive(Debug, Deserialize)]
struct PyprojectFile {
    tool: Option<ToolSection>,
}

#[derive(Debug, Deserialize)]
struct ToolSection {
    forge: Option<ForgeSection>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ForgeSection {
    blueprint: String,
    #[serde(rename = "blueprint_version")]
    _blueprint_version: Option<String>,
    project_name: String,
    crate_name: String,
    description: String,
    author_name: Option<String>,
    author_email: Option<String>,
    license: String,
    rust_edition: String,
    ignore: Option<Vec<String>>,
    #[serde(alias = "options")]
    overrides: Option<BTreeMap<String, bool>>,
}

pub fn config_from_pyproject(content: &str) -> Result<ProjectConfig> {
    let parsed: PyprojectFile =
        toml::from_str(content).context("failed to parse pyproject.toml")?;
    let forge = parsed
        .tool
        .and_then(|tool| tool.forge)
        .context("missing [tool.forge] metadata")?;

    BlueprintSpec::parse_for(BlueprintName::RustLibrary, &forge.blueprint, ErrorCode::Env)?;

    let overrides = forge.overrides.unwrap_or_default();
    let options = validate_managed_overrides_from_metadata(BlueprintName::RustLibrary, overrides)?;

    let config = ProjectConfig {
        project_name: forge.project_name,
        crate_name: forge.crate_name,
        description: forge.description,
        author_name: forge.author_name,
        author_email: forge.author_email,
        license: forge.license,
        rust_edition: forge.rust_edition,
        docs: managed_option_enabled(&options, ManagedOption::Docs)?,
        rust_rules: managed_option_enabled(&options, ManagedOption::RustRules)?,
        components: ComponentSelection::from_options(&options)?,
        ignored_files: forge.ignore.unwrap_or_default(),
    };
    config.validate()?;
    Ok(config)
}

fn remove_if_exists(path: &Path) -> Result<()> {
    remove_managed_file_if_exists(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn just_verify_runs_full_rust_quality_gate_explicitly() {
        let justfile = render_justfile(&test_config(true));

        assert!(
            justfile
                .contains("set windows-shell := [\"powershell.exe\", \"-NoLogo\", \"-Command\"]")
        );
        assert!(!justfile.contains("windows-powershell"));
        assert!(justfile.contains("verify:\n    uv lock --check"));
        assert!(justfile.contains("cargo fmt --all --check"));
        assert!(
            justfile
                .contains("cargo clippy --workspace --all-targets --all-features -- -D warnings")
        );
        assert!(justfile.contains("uv run --locked prek run --all-files"));
        assert!(!justfile.contains("forge sync --path . --check"));
        assert!(justfile.contains("cargo test"));
    }

    #[test]
    fn disabled_docs_remove_docs_dependency_and_recipe() {
        let config = test_config(false);
        let pyproject = render_pyproject(&config);
        let justfile = render_justfile(&config);

        assert!(!pyproject.contains("@astrojs/starlight"));
        assert!(!pyproject.contains("docs = ["));
        assert!(!justfile.contains("\ndocs:\n"));
        assert!(!justfile.contains("npm run dev"));
    }

    #[test]
    fn prettier_component_formats_with_just_and_checks_in_hooks() {
        let mut config = test_config(true);
        config.components = ComponentSelection::from_prettier(true);

        let justfile = render_justfile(&config);
        let precommit = render_precommit_config(&config);

        assert!(justfile.contains("cargo fmt --all"));
        assert!(justfile.contains(
            "npx --yes prettier@3.8.3 --write --ignore-path .prettierignore --ignore-unknown ."
        ));
        assert!(precommit.contains(
            "npx --yes prettier@3.8.3 --check --ignore-path .prettierignore --ignore-unknown"
        ));
        assert!(!precommit.contains(
            "npx --yes prettier@3.8.3 --write --ignore-path .prettierignore --ignore-unknown"
        ));
    }

    fn test_config(docs: bool) -> ProjectConfig {
        ProjectConfig {
            project_name: "test-rs".to_string(),
            crate_name: "test_rs".to_string(),
            description: "A test project".to_string(),
            author_name: Some("Test User".to_string()),
            author_email: Some("test@example.com".to_string()),
            license: "MIT".to_string(),
            rust_edition: "2024".to_string(),
            docs,
            rust_rules: true,
            components: ComponentSelection::default(),
            ignored_files: Vec::new(),
        }
    }

    #[test]
    fn render_creates_gitattributes_line_ending_policy() {
        let files = render_project_files(&test_config(true));

        let policy = files
            .get(&PathBuf::from(".gitattributes"))
            .and_then(GeneratedFile::as_text)
            .expect(".gitattributes should be generated");

        assert!(policy.contains("* text=auto eol=lf"));
        assert!(policy.contains("*.bat text eol=crlf"));
        assert!(policy.contains("*.cmd text eol=crlf"));
        assert!(policy.contains("*.png binary"));
        assert!(policy.contains("*.zip binary"));
    }

    #[test]
    fn renders_supported_license_templates() {
        let expected_headings = [
            ("BSD-3-Clause", "BSD 3-Clause License"),
            ("MIT", "MIT License"),
            ("Apache-2.0", "Apache License"),
            ("BSD-2-Clause", "BSD 2-Clause License"),
            ("ISC", "ISC License"),
        ];

        for (license, heading) in expected_headings {
            let mut config = test_config(true);
            config.license = license.to_string();
            config.validate().expect("license should be supported");
            assert!(render_license(&config).contains(heading));
        }
    }

    #[test]
    fn ci_workflow_runs_full_rust_quality_gate_explicitly() {
        let workflow = render_ci_workflow();

        assert!(workflow.contains("permissions:\n  contents: read\n\njobs:"));
        assert!(workflow.contains(github_actions::cancel_redundant_ci_concurrency()));
        assert!(workflow.contains(github_actions::job_timeout()));
        assert!(workflow.contains(github_actions::read_only_checkout_step()));
        assert!(workflow.contains("enable-cache: true"));
        assert!(workflow.contains(github_actions::uv_sync_locked_step()));
        assert!(workflow.contains(github_actions::uv_lock_check_step()));
        assert!(workflow.contains(&github_actions::uv_run_locked_step("prek run --all-files")));
        assert!(workflow.contains("run: cargo fmt --all --check"));
        assert!(
            workflow.contains(
                "run: cargo clippy --workspace --all-targets --all-features -- -D warnings"
            )
        );
        assert!(workflow.contains("run: forge sync --path . --check"));
        assert!(workflow.contains("  windows-smoke:\n    runs-on: windows-latest"));
        assert!(workflow.contains("run: cargo test"));
    }

    #[test]
    fn precommit_config_keeps_uv_lock_current() {
        let precommit = render_precommit_config(&test_config(true));

        assert!(precommit.contains("repo: https://github.com/astral-sh/uv-pre-commit"));
        assert!(precommit.contains("id: uv-lock"));
    }

    #[test]
    fn config_from_pyproject_rejects_invalid_metadata() {
        let metadata = r#"[tool.forge]
blueprint = "rust-library"
project_name = "test-rs"
crate_name = "123_bad"
description = "A test project"
author_name = "Test User"
author_email = "test@example.com"
license = "MIT"
rust_edition = "2024"

[tool.forge.overrides]
"#;

        let error = config_from_pyproject(metadata).expect_err("invalid metadata should fail");

        assert!(error.to_string().contains("invalid Rust crate name"));
    }

    #[test]
    fn config_from_pyproject_rejects_unknown_forge_options() {
        let metadata = r#"[tool.forge]
blueprint = "rust-library"
project_name = "test-rs"
crate_name = "test_rs"
description = "A test project"
author_name = "Test User"
author_email = "test@example.com"
license = "MIT"
rust_edition = "2024"

[tool.forge.overrides]
prettier_typo = true
"#;

        let error = config_from_pyproject(metadata).expect_err("unknown option should fail");

        assert!(
            error
                .to_string()
                .contains("unsupported managed option 'prettier_typo'")
        );
    }

    #[test]
    fn config_from_pyproject_defaults_missing_overrides() {
        let metadata = r#"[tool.forge]
blueprint = "rust-library"
project_name = "test-rs"
crate_name = "test_rs"
description = "A test project"
author_name = "Test User"
author_email = "test@example.com"
license = "MIT"
rust_edition = "2024"

[tool.forge.overrides]
prettier = true
"#;

        let config =
            config_from_pyproject(metadata).expect("missing overrides should use defaults");

        assert!(config.docs);
        assert!(config.rust_rules);
        assert!(config.components.is_enabled(ManagedComponent::Prettier));
        assert!(config.components.is_enabled(ManagedComponent::Editorconfig));
    }
}
