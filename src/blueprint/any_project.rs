use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::errors::ErrorCode;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

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
    BlueprintName, BlueprintSpec, ManagedOption, apply_ignored_files, managed_option_enabled,
    render_forge_ignore, render_forge_overrides_table, validate_managed_overrides_from_metadata,
};

pub const BLUEPRINT_NAME: &str = "any-project";
pub const BLUEPRINT_VERSION: &str = "0.1.0";

#[derive(Clone, Debug)]
pub struct ProjectConfig {
    pub project_name: String,
    pub description: String,
    pub docs: bool,
    pub components: ComponentSelection,
    pub ignored_files: Vec<String>,
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
        PathBuf::from(".gitattributes"),
        GeneratedFile::text(gitattributes::render_line_ending_policy()),
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
        forge_ignore: String,
        forge_overrides: String,
    }

    template_engine::render_template(
        "any_project/pyproject.toml.j2",
        Context {
            blueprint_name: BLUEPRINT_NAME,
            blueprint_version: BLUEPRINT_VERSION,
            project_name: toml_value::string_literal(&config.project_name),
            description: toml_value::string_literal(&config.description),
            docs_group: render_docs_dependency_group(config.docs),
            forge_ignore: render_forge_ignore(&config.ignored_files),
            forge_overrides: render_forge_overrides_table(
                BlueprintName::AnyProject,
                &[
                    (ManagedOption::Docs, config.docs),
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
            ),
        },
    )
}

fn render_docs_dependency_group(_enabled: bool) -> &'static str {
    ""
}

fn render_justfile(config: &ProjectConfig) -> String {
    #[derive(Serialize)]
    struct Context {
        docs_recipe: &'static str,
        format_steps: String,
    }

    let mut justfile = template_engine::render_template(
        "any_project/justfile.j2",
        Context {
            docs_recipe: if config.docs {
                "docs:\n    cd docs && npm install\n    cd docs && npm run dev\n"
            } else {
                ""
            },
            format_steps: render_component_format_steps(config),
        },
    );
    justfile.push('\n');
    justfile
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
        uv_lock_hook: &'a str,
    }

    template_engine::render_template(
        "any_project/pre-commit-config.yaml.j2",
        Context {
            component_hooks: config.components.pre_commit_hooks(),
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
        forge_sync_check_step: &'a str,
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
            forge_sync_check_step: github_actions::forge_sync_check_step(),
        },
    )
}

fn render_docs_package_json(config: &ProjectConfig) -> String {
    #[derive(Serialize)]
    struct Context<'a> {
        project_name: &'a str,
    }

    template_engine::render_template(
        "any_project/docs-package.json.j2",
        Context {
            project_name: &config.project_name,
        },
    )
}

fn render_docs_astro_config() -> String {
    template_engine::render_template("any_project/docs-astro.config.mjs.j2", ())
}

fn render_docs_tsconfig() -> String {
    template_engine::render_template("any_project/docs-tsconfig.json.j2", ())
}

fn render_docs_index(config: &ProjectConfig) -> String {
    #[derive(Serialize)]
    struct Context<'a> {
        project_name: &'a str,
        description: &'a str,
    }

    template_engine::render_template(
        "any_project/docs-index.mdx.j2",
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

    BlueprintSpec::parse_for(BlueprintName::AnyProject, &forge.blueprint, ErrorCode::Env)?;

    let overrides = forge.overrides.unwrap_or_default();
    let options = validate_managed_overrides_from_metadata(BlueprintName::AnyProject, overrides)?;

    let config = ProjectConfig {
        project_name: forge.project_name,
        description: forge.description,
        docs: managed_option_enabled(&options, ManagedOption::Docs)?,
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
    fn disabled_docs_remove_docs_dependency_and_recipe() {
        let config = ProjectConfig {
            project_name: "repo-infra".to_string(),
            description: "A test project".to_string(),
            docs: false,
            components: ComponentSelection::default(),
            ignored_files: Vec::new(),
        };

        let pyproject = render_pyproject(&config);
        let justfile = render_justfile(&config);

        assert!(!pyproject.contains("@astrojs/starlight"));
        assert!(!pyproject.contains("docs = ["));
        assert!(!justfile.contains("\ndocs:\n"));
        assert!(!justfile.contains("npm run dev"));
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
        assert!(workflow.contains("  windows-smoke:\n    runs-on: windows-latest"));
        assert!(workflow.contains("uv sync --all-groups --locked"));
    }

    #[test]
    fn render_creates_gitattributes_line_ending_policy() {
        let files = render_project_files(&ProjectConfig {
            project_name: "repo-infra".to_string(),
            description: "A test project".to_string(),
            docs: true,
            components: ComponentSelection::default(),
            ignored_files: Vec::new(),
        });

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
    fn just_verify_uses_locked_uv_commands() {
        let justfile = render_justfile(&ProjectConfig {
            project_name: "repo-infra".to_string(),
            description: "A test project".to_string(),
            docs: true,
            components: ComponentSelection::default(),
            ignored_files: Vec::new(),
        });

        assert!(
            justfile
                .contains("set windows-shell := [\"powershell.exe\", \"-NoLogo\", \"-Command\"]")
        );
        assert!(!justfile.contains("windows-powershell"));
        assert!(justfile.contains("verify:\n    uv lock --check"));
        assert!(justfile.contains("uv run --locked prek run --all-files"));
        assert!(!justfile.contains("forge sync --path . --check"));
    }

    #[test]
    fn prettier_component_formats_with_just_and_checks_in_hooks() {
        let config = ProjectConfig {
            project_name: "repo-infra".to_string(),
            description: "A test project".to_string(),
            docs: true,
            components: ComponentSelection::from_prettier(true),
            ignored_files: Vec::new(),
        };

        let justfile = render_justfile(&config);
        let precommit = render_precommit_config(&config);

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

    #[test]
    fn precommit_config_keeps_uv_lock_current() {
        let config = ProjectConfig {
            project_name: "repo-infra".to_string(),
            description: "A test project".to_string(),
            docs: true,
            components: ComponentSelection::default(),
            ignored_files: Vec::new(),
        };

        let precommit = render_precommit_config(&config);

        assert!(precommit.contains("repo: https://github.com/astral-sh/uv-pre-commit"));
        assert!(precommit.contains("id: uv-lock"));
    }

    #[test]
    fn config_from_pyproject_rejects_invalid_metadata() {
        let metadata = r#"[tool.forge]
blueprint = "any-project"
project_name = ".hidden"
description = "A test project"

[tool.forge.overrides]
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
blueprint = "any-project"
project_name = "repo-infra"
description = "A test project"

[tool.forge.overrides]
prettier = true
"#;

        let config =
            config_from_pyproject(metadata).expect("missing overrides should use defaults");

        assert!(config.docs);
        assert!(config.components.is_enabled(ManagedComponent::Prettier));
        assert!(config.components.is_enabled(ManagedComponent::Editorconfig));
    }

    #[test]
    fn config_from_pyproject_preserves_explicit_editorconfig_false_override() {
        let metadata = r#"[tool.forge]
blueprint = "any-project"
project_name = "repo-infra"
description = "A test project"

[tool.forge.overrides]
editorconfig = false
"#;

        let config = config_from_pyproject(metadata).expect("metadata should parse");

        assert!(!config.components.is_enabled(ManagedComponent::Editorconfig));
    }
}
