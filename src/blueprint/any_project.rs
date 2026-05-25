use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::blueprint::agents;
use crate::blueprint::components::{ComponentSelection, ManagedComponent};
use crate::blueprint::files::{GeneratedFile, GeneratedFiles, remove_managed_file_if_exists};
use crate::blueprint::github_actions;
use crate::blueprint::precommit;
use crate::blueprint::readme;
use crate::blueprint::template_engine;
use crate::blueprint::toml_value;
use crate::blueprint::{
    BlueprintName, ManagedOption, managed_option_enabled, validate_managed_options_from_metadata,
};

pub const BLUEPRINT_NAME: &str = "any-project";
pub const BLUEPRINT_VERSION: &str = "0.1.0";

#[derive(Clone, Debug)]
pub struct ProjectConfig {
    pub project_name: String,
    pub description: String,
    pub docs: bool,
    pub components: ComponentSelection,
}

impl ProjectConfig {
    pub fn validate(&self) -> Result<()> {
        if !is_valid_project_name(&self.project_name) {
            bail!("invalid project name: {}", self.project_name);
        }
        if self.description.trim().is_empty() {
            bail!("description cannot be empty");
        }
        Ok(())
    }
}

pub fn is_valid_project_name(value: &str) -> bool {
    let valid_chars = value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'));
    !value.is_empty() && !value.starts_with('.') && valid_chars
}

pub fn render_project_files(config: &ProjectConfig) -> GeneratedFiles {
    render_managed_files(config)
}

pub fn render_managed_files(config: &ProjectConfig) -> GeneratedFiles {
    let mut files = GeneratedFiles::new();

    files.insert(
        PathBuf::from("README.md"),
        GeneratedFile::text(render_readme(config)),
    );
    files.insert(
        PathBuf::from(".gitignore"),
        GeneratedFile::text(render_gitignore()),
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
    files.extend(agents::render_agent_files(&[]));
    files.insert(
        PathBuf::from(".github/workflows/ci.yaml"),
        GeneratedFile::text(render_ci_workflow()),
    );
    files.insert(
        PathBuf::from(".github/workflows/forge-update.yaml"),
        GeneratedFile::text(github_actions::render_forge_update_workflow()),
    );
    if config.docs {
        files.insert(
            PathBuf::from("mkdocs.yml"),
            GeneratedFile::text(render_mkdocs(config)),
        );
        files.insert(
            PathBuf::from("docs/index.md"),
            GeneratedFile::text(render_docs_index(config)),
        );
    }
    files.extend(config.components.render_files());

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
            ["mkdocs.yml", "docs/index.md"]
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
    #[derive(Serialize)]
    struct Context<'a> {
        project_name: &'a str,
        description: &'a str,
        automated_update_section: &'a str,
        blueprint_name: &'a str,
    }

    template_engine::render_template(
        "any_project/readme.md.j2",
        Context {
            project_name: &config.project_name,
            description: &config.description,
            automated_update_section: readme::automated_update_section(),
            blueprint_name: BLUEPRINT_NAME,
        },
    )
}

fn render_gitignore() -> String {
    template_engine::render_template("any_project/gitignore.j2", ())
}

fn render_pyproject(config: &ProjectConfig) -> String {
    #[derive(Serialize)]
    struct Context<'a> {
        blueprint_name: &'a str,
        blueprint_version: &'a str,
        project_name: String,
        description: String,
        docs_group: &'a str,
        docs: bool,
        prettier: bool,
        editorconfig: bool,
        markdownlint: bool,
    }

    template_engine::render_template(
        "any_project/pyproject.toml.j2",
        Context {
            blueprint_name: BLUEPRINT_NAME,
            blueprint_version: BLUEPRINT_VERSION,
            project_name: toml_value::string_literal(&config.project_name),
            description: toml_value::string_literal(&config.description),
            docs_group: render_docs_dependency_group(config.docs),
            docs: config.docs,
            prettier: config.components.is_enabled(ManagedComponent::Prettier),
            editorconfig: config.components.is_enabled(ManagedComponent::Editorconfig),
            markdownlint: config.components.is_enabled(ManagedComponent::Markdownlint),
        },
    )
}

fn render_docs_dependency_group(enabled: bool) -> &'static str {
    if enabled {
        "docs = [\"mkdocs-material>=9.7.0,<10.0.0\"]\n\n"
    } else {
        ""
    }
}

fn render_justfile(config: &ProjectConfig) -> String {
    #[derive(Serialize)]
    struct Context {
        docs_recipe: &'static str,
        format_steps: String,
    }

    template_engine::render_template(
        "any_project/justfile.j2",
        Context {
            docs_recipe: if config.docs {
                "\ndocs:\n    uv run mkdocs serve\n"
            } else {
                ""
            },
            format_steps: render_component_format_steps(config),
        },
    )
}

fn render_component_format_steps(config: &ProjectConfig) -> String {
    let commands = config.components.format_commands();
    if commands.is_empty() {
        return "    uv run prek run --all-files\n\n".to_string();
    }

    commands
        .into_iter()
        .map(|command| format!("    {command}\n"))
        .collect::<String>()
        + "\n"
}

fn render_precommit_config(config: &ProjectConfig) -> String {
    #[derive(Serialize)]
    struct Context<'a> {
        component_hooks: String,
        forge_update_check_hook: &'a str,
        uv_lock_hook: &'a str,
    }

    template_engine::render_template(
        "any_project/pre-commit-config.yaml.j2",
        Context {
            component_hooks: config.components.pre_commit_hooks(),
            forge_update_check_hook: precommit::forge_update_check_hook(),
            uv_lock_hook: precommit::uv_lock_hook(),
        },
    )
}

fn render_ci_workflow() -> String {
    #[derive(Serialize)]
    struct Context<'a> {
        cancel_redundant_ci_concurrency: &'a str,
        read_only_permissions: &'a str,
        job_timeout: &'a str,
        read_only_checkout_step: &'a str,
        setup_uv_step: &'a str,
        install_forge_step: &'a str,
        uv_sync_locked_step: &'a str,
        uv_lock_check_step: &'a str,
        uv_run_locked_step: String,
        forge_update_check_step: &'a str,
    }

    template_engine::render_template(
        "any_project/ci.yaml.j2",
        Context {
            cancel_redundant_ci_concurrency: github_actions::cancel_redundant_ci_concurrency(),
            read_only_permissions: github_actions::read_only_permissions(),
            job_timeout: github_actions::job_timeout(),
            read_only_checkout_step: github_actions::read_only_checkout_step(),
            setup_uv_step: github_actions::setup_uv_step(),
            install_forge_step: github_actions::install_forge_step(),
            uv_sync_locked_step: github_actions::uv_sync_locked_step(),
            uv_lock_check_step: github_actions::uv_lock_check_step(),
            uv_run_locked_step: github_actions::uv_run_locked_step("prek run --all-files"),
            forge_update_check_step: github_actions::forge_update_check_step(),
        },
    )
}

fn render_mkdocs(config: &ProjectConfig) -> String {
    #[derive(Serialize)]
    struct Context<'a> {
        project_name: &'a str,
    }

    template_engine::render_template(
        "any_project/mkdocs.yml.j2",
        Context {
            project_name: &config.project_name,
        },
    )
}

fn render_docs_index(config: &ProjectConfig) -> String {
    #[derive(Serialize)]
    struct Context<'a> {
        project_name: &'a str,
        description: &'a str,
    }

    template_engine::render_template(
        "any_project/docs-index.md.j2",
        Context {
            project_name: &config.project_name,
            description: &config.description,
        },
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
    description: String,
    options: Option<BTreeMap<String, bool>>,
}

pub fn config_from_pyproject(content: &str) -> Result<ProjectConfig> {
    let parsed: PyprojectFile =
        toml::from_str(content).context("failed to parse pyproject.toml")?;
    let forge = parsed
        .tool
        .and_then(|tool| tool.forge)
        .context("missing [tool.forge] metadata")?;

    if forge.blueprint != BLUEPRINT_NAME {
        bail!(
            "unsupported blueprint '{}' (expected '{}')",
            forge.blueprint,
            BLUEPRINT_NAME
        );
    }

    let options = forge.options.unwrap_or_default();
    let options = validate_managed_options_from_metadata(BlueprintName::AnyProject, options)?;

    let config = ProjectConfig {
        project_name: forge.project_name,
        description: forge.description,
        docs: managed_option_enabled(&options, ManagedOption::Docs)?,
        components: ComponentSelection::from_options(&options)?,
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
    fn disabled_docs_remove_docs_dependency_and_recipe() {
        let config = ProjectConfig {
            project_name: "repo-infra".to_string(),
            description: "A test project".to_string(),
            docs: false,
            components: ComponentSelection::default(),
        };

        let pyproject = render_pyproject(&config);
        let justfile = render_justfile(&config);

        assert!(!pyproject.contains("mkdocs-material"));
        assert!(!pyproject.contains("docs = ["));
        assert!(!justfile.contains("\ndocs:\n"));
        assert!(!justfile.contains("mkdocs serve"));
    }

    #[test]
    fn ci_workflow_uses_read_only_permissions() {
        let workflow = render_ci_workflow();

        assert!(workflow.contains("permissions:\n  contents: read\n\njobs:"));
        assert!(workflow.contains(github_actions::cancel_redundant_ci_concurrency()));
        assert!(workflow.contains(github_actions::job_timeout()));
        assert!(workflow.contains(github_actions::read_only_checkout_step()));
        assert!(workflow.contains("enable-cache: true"));
        assert!(workflow.contains(github_actions::uv_sync_locked_step()));
        assert!(workflow.contains(github_actions::uv_lock_check_step()));
        assert!(workflow.contains(&github_actions::uv_run_locked_step("prek run --all-files")));
    }

    #[test]
    fn just_verify_uses_locked_uv_commands() {
        let justfile = render_justfile(&ProjectConfig {
            project_name: "repo-infra".to_string(),
            description: "A test project".to_string(),
            docs: true,
            components: ComponentSelection::default(),
        });

        assert!(justfile.contains("verify:\n    uv lock --check"));
        assert!(justfile.contains("uv run --locked prek run --all-files"));
    }

    #[test]
    fn prettier_component_formats_with_just_and_checks_in_hooks() {
        let config = ProjectConfig {
            project_name: "repo-infra".to_string(),
            description: "A test project".to_string(),
            docs: true,
            components: ComponentSelection::from_prettier(true),
        };

        let justfile = render_justfile(&config);
        let precommit = render_precommit_config(&config);

        assert!(justfile.contains("npx --yes prettier@3.8.3 --write --ignore-unknown ."));
        assert!(precommit.contains("npx --yes prettier@3.8.3 --check --ignore-unknown"));
        assert!(!precommit.contains("npx --yes prettier@3.8.3 --write --ignore-unknown"));
    }

    #[test]
    fn precommit_config_keeps_uv_lock_current() {
        let config = ProjectConfig {
            project_name: "repo-infra".to_string(),
            description: "A test project".to_string(),
            docs: true,
            components: ComponentSelection::default(),
        };

        let precommit = render_precommit_config(&config);

        assert!(precommit.contains("repo: https://github.com/astral-sh/uv-pre-commit"));
        assert!(precommit.contains("id: uv-lock"));
        assert!(precommit.contains("id: forge-update-check"));
        assert!(precommit.contains("forge update --path . --check"));
    }

    #[test]
    fn config_from_pyproject_rejects_invalid_metadata() {
        let metadata = r#"[tool.forge]
blueprint = "any-project"
project_name = ".hidden"
description = "A test project"

[tool.forge.options]
docs = true
prettier = false
editorconfig = false
markdownlint = false
"#;

        let error = config_from_pyproject(metadata).expect_err("invalid metadata should fail");

        assert!(error.to_string().contains("invalid project name"));
    }

    #[test]
    fn config_from_pyproject_rejects_unknown_forge_options() {
        let metadata = r#"[tool.forge]
blueprint = "any-project"
project_name = "repo-infra"
description = "A test project"

[tool.forge.options]
docs = true
prettier = false
editorconfig = false
markdownlint = false
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
    fn config_from_pyproject_rejects_missing_supported_options() {
        let metadata = r#"[tool.forge]
blueprint = "any-project"
project_name = "repo-infra"
description = "A test project"

[tool.forge.options]
docs = true
"#;

        let error =
            config_from_pyproject(metadata).expect_err("missing supported options should fail");
        assert!(
            error
                .to_string()
                .contains("missing tool.forge.options.prettier")
        );
    }
}
