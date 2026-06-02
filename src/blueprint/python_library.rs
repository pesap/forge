use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::errors::ErrorCode;
use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::blueprint::agents;
use crate::blueprint::components::{ComponentSelection, ManagedComponent};
use crate::blueprint::files::{GeneratedFile, GeneratedFiles, remove_managed_file_if_exists};
use crate::blueprint::github_actions;
use crate::blueprint::precommit;
use crate::blueprint::readme;
use crate::blueprint::template_engine;
use crate::blueprint::toml_value;
use crate::blueprint::{
    BlueprintName, BlueprintSpec, ManagedOption, managed_option_enabled,
    render_forge_overrides_table, validate_managed_overrides_from_metadata,
};

pub const BLUEPRINT_NAME: &str = "python-library";
pub const BLUEPRINT_VERSION: &str = "0.1.0";
pub const PYPI_PUBLISH_NOTICE: &str =
    "Register this workflow as a trusted publisher in PyPI before uncommenting the publish step.";
pub const CODECOV_NOTICE: &str =
    "Configure Codecov for this repository before uncommenting the upload step.";

#[derive(Clone, Debug)]
pub struct ProjectConfig {
    pub project_name: String,
    pub package_name: String,
    pub description: String,
    pub author_name: Option<String>,
    pub author_email: Option<String>,
    pub license: String,
    pub python_min: String,
    pub gitignore_profile: String,
    pub docs: bool,
    pub codecov: bool,
    pub pypi_publish: bool,
    pub python_rules: bool,
    pub components: ComponentSelection,
}

impl ProjectConfig {
    pub fn validate(&self) -> Result<()> {
        if !is_valid_project_name(&self.project_name) {
            bail!("invalid project name: {}", self.project_name);
        }
        if !is_valid_package_name(&self.package_name) {
            bail!("invalid package name: {}", self.package_name);
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
        if !matches!(self.license.as_str(), "BSD-3-Clause" | "MIT" | "Apache-2.0") {
            bail!("license must be BSD-3-Clause, MIT, or Apache-2.0");
        }
        if !is_valid_python_version(&self.python_min) {
            bail!("python-min must be between 3.8 and 3.14 as a major.minor Python 3 version");
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

pub fn is_valid_package_name(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

pub fn default_package_name(project_name: &str) -> String {
    project_name.replace(['-', '.'], "_")
}

pub fn is_valid_python_version(value: &str) -> bool {
    matches!(python_minor_version(value), Some(8..15))
}

fn python_minor_version(value: &str) -> Option<u16> {
    let (major, minor) = value.split_once('.')?;
    if major != "3" || minor.is_empty() || !minor.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }

    minor.parse::<u16>().ok()
}

pub fn render_project_files(config: &ProjectConfig) -> GeneratedFiles {
    let mut files = render_infrastructure_files(config);

    files.insert(
        PathBuf::from(format!("src/{}/__init__.py", config.package_name)),
        GeneratedFile::text(render_init_py(config)),
    );
    files.insert(
        PathBuf::from(format!("src/{}/core.py", config.package_name)),
        GeneratedFile::text(render_core_py(config)),
    );
    files.insert(
        PathBuf::from(format!("src/{}/py.typed", config.package_name)),
        GeneratedFile::text(template_engine::render_template("shared/py.typed.j2", ())),
    );
    files.insert(
        PathBuf::from(format!("tests/test_{}.py", config.package_name)),
        GeneratedFile::text(render_test_py(config)),
    );

    files
}

pub fn render_managed_files(config: &ProjectConfig) -> GeneratedFiles {
    render_infrastructure_files(config)
}

pub fn render_managed_files_from_pyproject(content: &str) -> Result<GeneratedFiles> {
    let config = config_from_pyproject(content)?;
    Ok(render_managed_files(&config))
}

fn render_infrastructure_files(config: &ProjectConfig) -> GeneratedFiles {
    let mut files = GeneratedFiles::new();

    files.insert(
        PathBuf::from("README.md"),
        GeneratedFile::text(render_readme(config)),
    );
    files.insert(
        PathBuf::from("LICENSE.txt"),
        GeneratedFile::text(render_license(config)),
    );
    files.insert(
        PathBuf::from(".gitignore"),
        GeneratedFile::text(render_gitignore(&config.gitignore_profile)),
    );
    files.insert(
        PathBuf::from(".python-version"),
        GeneratedFile::text(format!("{}\n", config.python_min)),
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
    files.insert(
        PathBuf::from(".cspell.json"),
        GeneratedFile::text(render_cspell_config()),
    );
    files.insert(
        PathBuf::from("CONTRIBUTING.md"),
        GeneratedFile::text(render_contributing()),
    );
    files.insert(
        PathBuf::from("CHANGELOG.md"),
        GeneratedFile::text(render_changelog()),
    );
    files.extend(agents::render_agent_files(&[
        "Preserve user-authored Python package code during managed infrastructure updates.",
    ]));
    files.insert(
        PathBuf::from(".github/workflows/ci.yaml"),
        GeneratedFile::text(render_ci_workflow(config)),
    );
    files.insert(
        PathBuf::from(".github/workflows/release-please.yaml"),
        GeneratedFile::text(render_release_please_workflow(config)),
    );
    files.insert(
        PathBuf::from(".github/workflows/workflow-quality.yaml"),
        GeneratedFile::text(render_workflow_quality_workflow()),
    );
    files.insert(
        PathBuf::from(".github/workflows/forge-update.yaml"),
        GeneratedFile::text(github_actions::render_forge_update_workflow()),
    );
    files.insert(
        PathBuf::from(".release-please-config.json"),
        GeneratedFile::text(render_release_please_config()),
    );
    files.insert(
        PathBuf::from(".release-please-manifest.json"),
        GeneratedFile::text(render_release_please_manifest()),
    );

    if config.docs {
        files.insert(
            PathBuf::from(".github/workflows/docs-pages.yaml"),
            GeneratedFile::text(render_docs_pages_workflow()),
        );
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
    files
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
                ".github/workflows/docs-pages.yaml",
                "docs/package.json",
                "docs/astro.config.mjs",
                "docs/tsconfig.json",
                "docs/src/content/docs/index.mdx",
            ]
            .into_iter()
            .map(PathBuf::from),
        );
    }

    files.push(PathBuf::from(".github/workflows/publish-pypi.yaml"));
    files.push(PathBuf::from(".github/workflows/publish.yaml"));

    files.extend(config.components.disabled_file_paths());
    files
}

pub fn optional_cleanup_paths_from_pyproject(content: &str) -> Result<Vec<PathBuf>> {
    let config = config_from_pyproject(content)?;
    Ok(optional_cleanup_paths(&config))
}

fn remove_if_exists(path: &Path) -> Result<()> {
    remove_managed_file_if_exists(path)
}

fn render_readme(config: &ProjectConfig) -> String {
    template_engine::render_template(
        "python_library/readme.md.j2",
        serde_json::json!({"project_name": config.project_name, "description": config.description, "automated_update_section": readme::automated_update_section(), "blueprint_name": BLUEPRINT_NAME}),
    )
}

fn render_license(config: &ProjectConfig) -> String {
    let template = match config.license.as_str() {
        "MIT" => "python_library/mit-license.j2",
        "Apache-2.0" => "python_library/apache-license.j2",
        _ => "python_library/bsd-license.j2",
    };
    template_engine::render_template(
        template,
        serde_json::json!({"year": "2026", "author": author_display_name(config)}),
    )
}

fn render_gitignore(gitignore_profile: &str) -> String {
    template_engine::render_template(
        "python_library/gitignore.j2",
        serde_json::json!({"gitignore_profile": gitignore_profile}),
    )
}

fn render_authors(config: &ProjectConfig) -> String {
    match (&config.author_name, &config.author_email) {
        (Some(author_name), Some(author_email)) => format!(
            "authors = [{{ name = {}, email = {} }}]\n",
            toml_value::string_literal(author_name),
            toml_value::string_literal(author_email)
        ),
        (Some(author_name), None) => format!(
            "authors = [{{ name = {} }}]\n",
            toml_value::string_literal(author_name)
        ),
        (None, Some(author_email)) => format!(
            "authors = [{{ email = {} }}]\n",
            toml_value::string_literal(author_email)
        ),
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

fn pytest_cache_dir(project_name: &str) -> PathBuf {
    cache_home_dir()
        .map(|cache_dir| cache_dir.join("pytest").join(project_name))
        .unwrap_or_else(|| PathBuf::from(".pytest_cache").join(project_name))
}

#[cfg(target_os = "windows")]
fn cache_home_dir() -> Option<PathBuf> {
    env_path("LOCALAPPDATA")
        .or_else(|| env_path("USERPROFILE").map(|home| home.join("AppData").join("Local")))
}

#[cfg(target_os = "macos")]
fn cache_home_dir() -> Option<PathBuf> {
    env_path("HOME").map(|home| home.join("Library").join("Caches"))
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
fn cache_home_dir() -> Option<PathBuf> {
    env_path("XDG_CACHE_HOME")
        .filter(|path| path.is_absolute())
        .or_else(|| env_path("HOME").map(|home| home.join(".cache")))
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn render_pyproject(config: &ProjectConfig) -> String {
    template_engine::render_template(
        "python_library/pyproject.toml.j2",
        serde_json::json!({
            "project_name": toml_value::string_literal(&config.project_name),
            "pytest_cache_dir": toml_value::string_literal(&pytest_cache_dir(&config.project_name).to_string_lossy()),
            "description": toml_value::string_literal(&config.description),
            "authors": render_authors(config),
            "requires_python": toml_value::string_literal(&format!(">={},<3.15", config.python_min)),
            "docs_group": render_docs_dependency_group(config.docs),
            "package_name": toml_value::string_literal(&config.package_name),
            "coverage_arg": toml_value::string_literal(&format!("--cov={}", config.package_name)),
            "blueprint_name": BLUEPRINT_NAME,
            "blueprint_version": BLUEPRINT_VERSION,
            "author_name": render_optional_forge_field("author_name", &config.author_name),
            "author_email": render_optional_forge_field("author_email", &config.author_email),
            "license": toml_value::string_literal(&config.license),
            "python_min": toml_value::string_literal(&config.python_min),
            "gitignore_profile": toml_value::string_literal(&config.gitignore_profile),
            "forge_overrides": render_forge_overrides_table(
                BlueprintName::PythonLibrary,
                &[
                    (ManagedOption::Docs, config.docs),
                    (ManagedOption::Codecov, config.codecov),
                    (ManagedOption::PypiPublish, config.pypi_publish),
                    (ManagedOption::PythonRules, config.python_rules),
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
    template_engine::render_template(
        "python_library/justfile.j2",
        serde_json::json!({"docs_recipe": if config.docs {"\ndocs:\n    cd docs && npm install\n    cd docs && npm run dev\n"} else {""}, "component_format_steps": render_component_format_steps(config), "package_name_unquoted": config.package_name}),
    )
}

fn render_component_format_steps(config: &ProjectConfig) -> String {
    config
        .components
        .format_commands()
        .into_iter()
        .map(|command| format!("    {command}\n"))
        .collect::<String>()
}

fn render_precommit_config(config: &ProjectConfig) -> String {
    template_engine::render_template(
        "python_library/pre-commit-config.yaml.j2",
        serde_json::json!({"component_hooks": config.components.pre_commit_hooks(), "python_rules": config.python_rules, "uv_lock_hook": precommit::uv_lock_hook()}),
    )
}

fn render_cspell_config() -> String {
    template_engine::render_template("python_library/cspell.json.j2", ())
}

fn render_contributing() -> String {
    template_engine::render_template("python_library/contributing.md.j2", ())
}

fn render_changelog() -> String {
    template_engine::render_template("python_library/changelog.md.j2", ())
}

fn render_ci_workflow(config: &ProjectConfig) -> String {
    template_engine::render_template(
        "python_library/ci.yaml.j2",
        serde_json::json!({
            "cancel_redundant_ci_concurrency": github_actions::cancel_redundant_ci_concurrency(),
            "read_only_permissions": github_actions::read_only_permissions(),
            "job_timeout": github_actions::job_timeout(),
            "python_matrix": render_python_matrix(&config.python_min),
            "package_name": config.package_name,
            "read_only_checkout_step": github_actions::read_only_checkout_step(),
            "setup_uv_step": github_actions::setup_uv_step(),
            "install_forge_step": github_actions::install_forge_step(),
            "uv_sync_locked_step": github_actions::uv_sync_locked_step(),
            "uv_lock_check_step": github_actions::uv_lock_check_step(),
            "ruff_format_step": github_actions::uv_run_locked_step("ruff format --check ."),
            "ruff_check_step": github_actions::uv_run_locked_step("ruff check ."),
            "ty_check_step": github_actions::uv_run_locked_step("ty check"),
            "prek_step": github_actions::uv_run_locked_step("prek run --all-files"),
            "forge_update_check_step": github_actions::forge_update_check_step(),
            "pytest_cov_step": github_actions::uv_run_locked_step("pytest --cov --cov-report=xml"),
            "codecov_step": if config.codecov {String::from("      - name: Upload coverage to Codecov\n        if: ${{ matrix.python-version == '3.14' }}\n        uses: codecov/codecov-action@e79a6962e0d4c0c17b229090214935d2e33f8354 # v6\n")} else {format!("      # {}\n      # - name: Upload coverage to Codecov\n      #   uses: codecov/codecov-action@e79a6962e0d4c0c17b229090214935d2e33f8354 # v6\n", CODECOV_NOTICE)}
        }),
    )
}

fn render_python_matrix(python_min: &str) -> String {
    ci_python_versions(python_min)
        .into_iter()
        .map(|version| format!("\"{version}\""))
        .collect::<Vec<_>>()
        .join(", ")
}

fn ci_python_versions(python_min: &str) -> Vec<String> {
    let Some(minor) = python_minor_version(python_min) else {
        return vec![python_min.to_string()];
    };
    let mut minors = vec![minor];
    for candidate in [11, 12, 13, 14] {
        if candidate >= minor && !minors.contains(&candidate) {
            minors.push(candidate);
        }
    }
    minors.sort_unstable();
    minors
        .into_iter()
        .map(|minor| format!("3.{minor}"))
        .collect()
}

fn render_workflow_quality_workflow() -> String {
    template_engine::render_template("python_library/workflow-quality.yaml.j2", ())
}

fn render_docs_pages_workflow() -> String {
    template_engine::render_template("python_library/docs-pages.yaml.j2", ())
}

fn render_release_please_workflow(config: &ProjectConfig) -> String {
    template_engine::render_template(
        "python_library/release-please.yaml.j2",
        serde_json::json!({
            "serialized_ref_concurrency": github_actions::serialized_ref_concurrency(),
            "job_timeout": github_actions::job_timeout(),
            "pypi_publish_job_block": if config.pypi_publish {
                format!(
                    "\n  publish-pypi:\n    runs-on: ubuntu-latest\n    needs: release-please\n    if: needs.release-please.outputs.release_created || (github.event_name == 'workflow_dispatch' && github.event.inputs.publish_pypi == 'true')\n    concurrency:\n      group: pypi-publish-${{{{ github.event_name == 'workflow_dispatch' && github.event.inputs.release_tag || needs.release-please.outputs.release_tag }}}}\n      cancel-in-progress: false\n    environment:\n      name: pypi\n      url: https://pypi.org/p/{}\n    permissions:\n      id-token: write\n      contents: read\n{}    steps:\n      - id: publish_ref\n        run: |\n          if [ \"${{{{ github.event_name }}}}\" = \"workflow_dispatch\" ] && [ \"${{{{ github.event.inputs.publish_pypi }}}}\" = \"true\" ] && [ -z \"${{{{ github.event.inputs.release_tag }}}}\" ]; then\n            echo \"release_tag input is required when publish_pypi=true\" >&2\n            exit 1\n          fi\n          REF=\"${{{{ github.event_name == 'workflow_dispatch' && github.event.inputs.release_tag || needs.release-please.outputs.release_tag }}}}\"\n          if [ -z \"$REF\" ]; then\n            echo \"No release tag resolved for publish step\" >&2\n            exit 1\n          fi\n          echo \"ref=$REF\" >> \"$GITHUB_OUTPUT\"\n      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4\n        with:\n          ref: ${{{{ steps.publish_ref.outputs.ref }}}}\n          persist-credentials: false\n      - name: Verify release tag exists\n        run: git rev-parse --verify \"refs/tags/${{{{ steps.publish_ref.outputs.ref }}}}\"\n      - uses: astral-sh/setup-uv@d0cc045d04ccac9d8b7881df0226f9e82c39688e # v6.6.0\n        with:\n          enable-cache: true\n          cache-dependency-glob: |\n            pyproject.toml\n            uv.lock\n      - run: uv build --locked\n      - run: uv publish --dry-run\n      # {}\n      # - name: Publish package distributions to PyPI\n      #   uses: pypa/gh-action-pypi-publish@cef221092ed1bacb1cc03d23a2d87d1d172e277b # release/v1\n      - name: Publish summary\n        run: |\n          echo \"### PyPI publish fallback dry-run\" >> \"$GITHUB_STEP_SUMMARY\"\n          echo \"- project: {}\" >> \"$GITHUB_STEP_SUMMARY\"\n          echo \"- tag: ${{{{ steps.publish_ref.outputs.ref }}}}\" >> \"$GITHUB_STEP_SUMMARY\"\n          echo \"- mode: uv publish --dry-run\" >> \"$GITHUB_STEP_SUMMARY\"\n          echo \"- to enable real publish: uncomment the trusted publishing step in this workflow\" >> \"$GITHUB_STEP_SUMMARY\"\n",
                    config.project_name,
                    github_actions::job_timeout(),
                    PYPI_PUBLISH_NOTICE,
                    config.project_name
                )
            } else {
                String::new()
            }
        }),
    )
}

fn render_release_please_config() -> String {
    template_engine::render_template("python_library/release-please-config.json.j2", ())
}

fn render_release_please_manifest() -> String {
    template_engine::render_template("python_library/release-please-manifest.json.j2", ())
}

fn render_docs_package_json(config: &ProjectConfig) -> String {
    template_engine::render_template(
        "python_library/docs-package.json.j2",
        serde_json::json!({"project_name": config.project_name}),
    )
}

fn render_docs_astro_config() -> String {
    template_engine::render_template("python_library/docs-astro.config.mjs.j2", ())
}

fn render_docs_tsconfig() -> String {
    template_engine::render_template("python_library/docs-tsconfig.json.j2", ())
}

fn render_docs_index(config: &ProjectConfig) -> String {
    template_engine::render_template(
        "python_library/docs-index.mdx.j2",
        serde_json::json!({"project_name": config.project_name, "description": config.description}),
    )
}

fn render_init_py(config: &ProjectConfig) -> String {
    template_engine::render_template(
        "python_library/__init__.py.j2",
        serde_json::json!({"package_name": config.package_name, "project_name": config.project_name}),
    )
}

fn render_core_py(config: &ProjectConfig) -> String {
    template_engine::render_template(
        "python_library/core.py.j2",
        serde_json::json!({"package_name": config.package_name}),
    )
}

fn render_test_py(config: &ProjectConfig) -> String {
    template_engine::render_template(
        "python_library/test.py.j2",
        serde_json::json!({"package_name": config.package_name, "project_name": config.project_name}),
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
    package_name: String,
    description: String,
    author_name: Option<String>,
    author_email: Option<String>,
    license: String,
    python_min: String,
    gitignore_profile: Option<String>,
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

    BlueprintSpec::parse_for(
        BlueprintName::PythonLibrary,
        &forge.blueprint,
        ErrorCode::Env,
    )?;

    let overrides = forge.overrides.unwrap_or_default();
    let options =
        validate_managed_overrides_from_metadata(BlueprintName::PythonLibrary, overrides)?;

    let config = ProjectConfig {
        project_name: forge.project_name,
        package_name: forge.package_name,
        description: forge.description,
        author_name: forge.author_name,
        author_email: forge.author_email,
        license: forge.license,
        python_min: forge.python_min,
        gitignore_profile: forge
            .gitignore_profile
            .unwrap_or_else(|| "python,macos,visualstudiocode,jetbrains,node".to_string()),
        docs: managed_option_enabled(&options, ManagedOption::Docs)?,
        codecov: managed_option_enabled(&options, ManagedOption::Codecov)?,
        pypi_publish: managed_option_enabled(&options, ManagedOption::PypiPublish)?,
        python_rules: managed_option_enabled(&options, ManagedOption::PythonRules)?,
        components: ComponentSelection::from_options(&options)?,
    };
    config.validate()?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_project_names_accepted() {
        assert!(is_valid_project_name("my-project"));
        assert!(is_valid_project_name("my_project"));
        assert!(is_valid_project_name("my.project"));
        assert!(is_valid_project_name("myProject123"));
        assert!(is_valid_project_name("a"));
    }

    #[test]
    fn invalid_project_names_rejected() {
        assert!(!is_valid_project_name("")); // Empty
        assert!(!is_valid_project_name(".hidden")); // Starts with dot
        assert!(!is_valid_project_name("my project")); // Contains space
        assert!(!is_valid_project_name("my@project")); // Special char
    }

    #[test]
    fn valid_package_names_accepted() {
        assert!(is_valid_package_name("my_package"));
        assert!(is_valid_package_name("my_package_123"));
        assert!(is_valid_package_name("_private"));
        assert!(is_valid_package_name("a"));
    }

    #[test]
    fn package_name_defaults_from_project_name() {
        assert_eq!(default_package_name("grid-tools"), "grid_tools");
        assert_eq!(default_package_name("grid.tools"), "grid_tools");
        assert_eq!(default_package_name("grid_tools"), "grid_tools");
    }

    #[test]
    fn invalid_package_names_rejected() {
        assert!(!is_valid_package_name("")); // Empty
        assert!(!is_valid_package_name("123abc")); // Starts with number
        assert!(!is_valid_package_name("my-package")); // Hyphen not allowed
        assert!(!is_valid_package_name("my.package")); // Dot not allowed
        assert!(!is_valid_package_name("my package")); // Space not allowed
    }

    #[test]
    fn python_versions_are_validated_for_generated_tooling() {
        assert!(is_valid_python_version("3.11"));
        assert!(is_valid_python_version("3.13"));
        assert!(is_valid_python_version("3.14"));
        assert!(!is_valid_python_version(""));
        assert!(!is_valid_python_version("3"));
        assert!(!is_valid_python_version("3.11.1"));
        assert!(!is_valid_python_version("3.15"));
        assert!(!is_valid_python_version("python3.11"));
        assert!(!is_valid_python_version("3.11\n3.12"));
    }

    #[test]
    fn ci_python_versions_include_minimum_and_supported_modern_versions() {
        assert_eq!(
            ci_python_versions("3.8"),
            vec!["3.8", "3.11", "3.12", "3.13", "3.14"]
        );
        assert_eq!(
            ci_python_versions("3.11"),
            vec!["3.11", "3.12", "3.13", "3.14"]
        );
    }

    #[test]
    fn ci_python_versions_never_include_versions_below_python_min() {
        assert_eq!(ci_python_versions("3.12"), vec!["3.12", "3.13", "3.14"]);
        assert_eq!(ci_python_versions("3.13"), vec!["3.13", "3.14"]);
        assert_eq!(ci_python_versions("3.14"), vec!["3.14"]);
    }

    #[test]
    fn render_ci_workflow_uses_deduplicated_python_matrix() {
        let config = ProjectConfig {
            project_name: "test-project".to_string(),
            package_name: "test_project".to_string(),
            description: "A test project".to_string(),
            author_name: Some("Test User".to_string()),
            author_email: Some("test@example.com".to_string()),
            license: "MIT".to_string(),
            python_min: "3.13".to_string(),
            gitignore_profile: "python,macos,visualstudiocode,jetbrains,node".to_string(),
            docs: true,
            codecov: false,
            pypi_publish: false,
            python_rules: true,
            components: ComponentSelection::default(),
        };

        let workflow = render_ci_workflow(&config);

        assert!(workflow.contains("fromJSON('[\"3.13\", \"3.14\"]')"));
        assert!(!workflow.contains("\"3.12\""));
    }

    #[test]
    fn just_verify_runs_non_mutating_python_quality_gate_explicitly() {
        let justfile = render_justfile(&test_config(true));

        assert!(justfile.contains("verify:\n    uv lock --check"));
        assert!(justfile.contains("uv run --locked ruff format --check ."));
        assert!(justfile.contains("uv run --locked ruff check ."));
        assert!(justfile.contains("uv run --locked ty check"));
        assert!(justfile.contains("forge update --path . --check"));
        assert!(justfile.contains("uv run --locked pytest --tb=short"));
        assert!(justfile.contains("uv build --locked"));
    }

    #[test]
    fn ci_workflow_runs_non_mutating_python_quality_gate_explicitly() {
        let mut config = test_config(true);
        config.codecov = true;
        let workflow = render_ci_workflow(&config);

        assert!(workflow.contains("uv run --locked pytest --cov --cov-report=xml"));
        assert!(workflow.contains("actions/setup-python@a309ff8b426b58ec0e2a45f0f869d46889d02405"));
        assert!(workflow.contains("name: Upload coverage to Codecov"));
        assert!(workflow.contains("if: ${{ matrix.python-version == '3.14' }}"));
    }

    #[test]
    fn prettier_component_formats_with_just_and_checks_in_hooks() {
        let mut config = test_config(true);
        config.components = ComponentSelection::from_prettier(true);

        let justfile = render_justfile(&config);
        let precommit = render_precommit_config(&config);

        assert!(justfile.contains("uv run ruff format ."));
        assert!(justfile.contains("npx --yes prettier@3.8.3 --write --ignore-unknown ."));
        assert!(precommit.contains("npx --yes prettier@3.8.3 --check --ignore-unknown"));
        assert!(!precommit.contains("npx --yes prettier@3.8.3 --write --ignore-unknown"));
    }

    #[test]
    fn precommit_config_keeps_uv_lock_current() {
        let precommit = render_precommit_config(&test_config(true));

        assert!(precommit.contains("repo: https://github.com/astral-sh/uv-pre-commit"));
        assert!(precommit.contains("id: uv-lock"));
    }

    #[test]
    fn precommit_config_uses_non_mutating_locked_python_checks() {
        let precommit = render_precommit_config(&test_config(true));

        assert!(precommit.contains("entry: uv run --locked ruff format --check"));
        assert!(precommit.contains("entry: uv run --locked ruff check"));
        assert!(precommit.contains("entry: uv run --locked ty check"));
        assert!(precommit.contains("entry: uv run --locked pytest -q --maxfail=1"));
        assert!(!precommit.contains("uv run ruff format\n"));
        assert!(!precommit.contains("uv run ruff check --fix"));
        assert!(!precommit.contains("uv run ty check\n"));
        assert!(!precommit.contains("uv run pytest -q --maxfail=1"));
    }

    #[test]
    fn starlight_config_uses_expected_site_title() {
        let package_json = render_docs_package_json(&test_config(true));

        assert!(package_json.contains("\"name\": \"test-project-docs\""));
        assert!(package_json.contains("\"@astrojs/starlight\""));
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

    fn test_config(docs: bool) -> ProjectConfig {
        ProjectConfig {
            project_name: "test-project".to_string(),
            package_name: "test_project".to_string(),
            description: "A test project".to_string(),
            author_name: Some("Test User".to_string()),
            author_email: Some("test@example.com".to_string()),
            license: "MIT".to_string(),
            python_min: "3.11".to_string(),
            gitignore_profile: "python,macos,visualstudiocode,jetbrains,node".to_string(),
            docs,
            codecov: false,
            pypi_publish: false,
            python_rules: true,
            components: ComponentSelection::default(),
        }
    }

    #[test]
    fn project_config_validates_correctly() {
        let config = ProjectConfig {
            project_name: "test-project".to_string(),
            package_name: "test_project".to_string(),
            description: "A test project".to_string(),
            author_name: Some("Test User".to_string()),
            author_email: Some("test@example.com".to_string()),
            license: "MIT".to_string(),
            python_min: "3.11".to_string(),
            gitignore_profile: "python,macos,visualstudiocode,jetbrains,node".to_string(),
            docs: true,
            codecov: false,
            pypi_publish: false,
            python_rules: true,
            components: ComponentSelection::default(),
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn project_config_rejects_invalid_email() {
        let config = ProjectConfig {
            project_name: "test-project".to_string(),
            package_name: "test_project".to_string(),
            description: "A test project".to_string(),
            author_name: Some("Test User".to_string()),
            author_email: Some("not-an-email".to_string()), // Invalid
            license: "MIT".to_string(),
            python_min: "3.11".to_string(),
            gitignore_profile: "python,macos,visualstudiocode,jetbrains,node".to_string(),
            docs: true,
            codecov: false,
            pypi_publish: false,
            python_rules: true,
            components: ComponentSelection::default(),
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn project_config_rejects_invalid_license() {
        let config = ProjectConfig {
            project_name: "test-project".to_string(),
            package_name: "test_project".to_string(),
            description: "A test project".to_string(),
            author_name: Some("Test User".to_string()),
            author_email: Some("test@example.com".to_string()),
            license: "GPL-3.0".to_string(), // Not in allowed list
            python_min: "3.11".to_string(),
            gitignore_profile: "python,macos,visualstudiocode,jetbrains,node".to_string(),
            docs: true,
            codecov: false,
            pypi_publish: false,
            python_rules: true,
            components: ComponentSelection::default(),
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn project_config_rejects_invalid_python_min() {
        let config = ProjectConfig {
            project_name: "test-project".to_string(),
            package_name: "test_project".to_string(),
            description: "A test project".to_string(),
            author_name: Some("Test User".to_string()),
            author_email: Some("test@example.com".to_string()),
            license: "MIT".to_string(),
            python_min: "3.11\n3.12".to_string(),
            gitignore_profile: "python,macos,visualstudiocode,jetbrains,node".to_string(),
            docs: true,
            codecov: false,
            pypi_publish: false,
            python_rules: true,
            components: ComponentSelection::default(),
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn config_from_pyproject_rejects_invalid_metadata() {
        let metadata = r#"[tool.forge]
blueprint = "python-library"
project_name = "test-project"
package_name = "test_project"
description = "A test project"
author_name = "Test User"
author_email = "test@example.com"
license = "MIT"
python_min = "3.11\n3.12"

[tool.forge.overrides]
codecov = false
prettier = false
editorconfig = false
markdownlint = false
"#;

        let error = config_from_pyproject(metadata).expect_err("invalid metadata should fail");

        assert!(error.to_string().contains("python-min"));
    }

    #[test]
    fn config_from_pyproject_rejects_unknown_forge_metadata() {
        let metadata = r#"[tool.forge]
blueprint = "python-library"
project_name = "grid-tools"
package_name = "grid_tools"
description = "Grid tooling"
author_name = "Ada Lovelace"
author_email = "ada@example.com"
license = "MIT"
python_min = "3.12"
unknown_option = true

[tool.forge.overrides]
prettier = false
editorconfig = false
markdownlint = false
"#;

        let error = config_from_pyproject(metadata).expect_err("unknown metadata should fail");

        assert!(error.to_string().contains("failed to parse pyproject.toml"));
    }

    #[test]
    fn config_from_pyproject_defaults_missing_overrides() {
        let metadata = r#"[tool.forge]
blueprint = "python-library"
project_name = "grid-tools"
package_name = "grid_tools"
description = "Grid tooling"
author_name = "Ada Lovelace"
author_email = "ada@example.com"
license = "MIT"
python_min = "3.12"

[tool.forge.overrides]
prettier = true
"#;

        let config =
            config_from_pyproject(metadata).expect("missing overrides should use defaults");

        assert!(config.docs);
        assert!(config.codecov);
        assert!(!config.pypi_publish);
        assert!(config.python_rules);
        assert!(config.components.is_enabled(ManagedComponent::Prettier));
    }

    #[test]
    fn render_creates_expected_files() {
        let config = ProjectConfig {
            project_name: "my-cool-lib".to_string(),
            package_name: "my_cool_lib".to_string(),
            description: "A cool library".to_string(),
            author_name: Some("Ada Lovelace".to_string()),
            author_email: Some("ada@example.com".to_string()),
            license: "MIT".to_string(),
            python_min: "3.11".to_string(),
            gitignore_profile: "python,macos,visualstudiocode,jetbrains,node".to_string(),
            docs: true,
            codecov: true,
            pypi_publish: false,
            python_rules: true,
            components: ComponentSelection::default(),
        };

        let files = render_project_files(&config);

        // Check core source files exist
        assert!(files.contains_key(&PathBuf::from("src/my_cool_lib/__init__.py")));
        assert!(files.contains_key(&PathBuf::from("src/my_cool_lib/core.py")));
        assert!(files.contains_key(&PathBuf::from("src/my_cool_lib/py.typed")));
        assert!(files.contains_key(&PathBuf::from("tests/test_my_cool_lib.py")));

        // Check infrastructure files
        assert!(files.contains_key(&PathBuf::from("README.md")));
        assert!(files.contains_key(&PathBuf::from("pyproject.toml")));
        assert!(files.contains_key(&PathBuf::from("justfile")));
        assert!(files.contains_key(&PathBuf::from(".gitignore")));
        assert!(files.contains_key(&PathBuf::from("LICENSE.txt")));
        assert_eq!(
            files
                .get(&PathBuf::from("CLAUDE.md"))
                .and_then(GeneratedFile::symlink_target),
            Some(Path::new("AGENTS.md"))
        );
    }

    #[test]
    fn ci_workflow_uses_read_only_permissions() {
        let workflow = render_ci_workflow(&test_config(true));

        assert!(workflow.contains("permissions:\n  contents: read\n\njobs:"));
        assert!(workflow.contains(github_actions::cancel_redundant_ci_concurrency()));
        assert!(workflow.contains(github_actions::job_timeout()));
        assert!(workflow.contains(github_actions::read_only_checkout_step()));
        assert!(workflow.contains("enable-cache: true"));
        assert!(workflow.contains(github_actions::uv_sync_locked_step()));
        assert!(workflow.contains(&github_actions::uv_run_locked_step("prek run --all-files")));
    }

    #[test]
    fn release_workflows_use_job_timeouts() {
        let release_please = render_release_please_workflow(&test_config(true));

        assert!(release_please.contains(github_actions::job_timeout()));
        assert!(
            release_please.contains(
                "googleapis/release-please-action@45996ed1f6d02564a971a2fa1b5860e934307cf7"
            )
        );
        assert!(!release_please.contains("publish is handled"));

        let mut with_publish = test_config(true);
        with_publish.pypi_publish = true;
        let release_please_with_publish = render_release_please_workflow(&with_publish);
        assert!(release_please_with_publish.contains("publish-pypi:"));
        assert!(release_please_with_publish.contains("needs: release-please"));
        assert!(release_please_with_publish.contains("if: needs.release-please.outputs.release_created || (github.event_name == 'workflow_dispatch' && github.event.inputs.publish_pypi == 'true')"));
        assert!(
            release_please_with_publish
                .contains("release_tag input is required when publish_pypi=true")
        );
        assert!(release_please_with_publish.contains("steps.publish_ref.outputs.ref"));
        assert!(release_please_with_publish.contains("concurrency:\n      group: pypi-publish-"));
        assert!(release_please_with_publish.contains("    environment:\n      name: pypi\n"));
        assert!(release_please_with_publish.contains("url: https://pypi.org/p/test-project"));
        assert!(release_please_with_publish.contains("uv build --locked"));
        assert!(release_please_with_publish.contains("uv publish --dry-run"));
        assert!(release_please_with_publish.contains(PYPI_PUBLISH_NOTICE));
        assert!(
            release_please_with_publish.contains("# - name: Publish package distributions to PyPI")
        );
    }

    #[test]
    fn release_workflows_serialize_duplicate_runs() {
        let release_please = render_release_please_workflow(&test_config(true));

        assert!(release_please.contains("concurrency:\n  group: ${{ github.workflow }}-${{ github.ref }}\n  cancel-in-progress: false\n\npermissions:"));
    }

    #[test]
    fn render_includes_forge_metadata() {
        let config = ProjectConfig {
            project_name: "meta-test".to_string(),
            package_name: "meta_test".to_string(),
            description: "Testing metadata".to_string(),
            author_name: Some("Grace Hopper".to_string()),
            author_email: Some("grace@example.com".to_string()),
            license: "BSD-3-Clause".to_string(),
            python_min: "3.12".to_string(),
            gitignore_profile: "python,macos,visualstudiocode,jetbrains,node".to_string(),
            docs: false,
            codecov: false,
            pypi_publish: true,
            python_rules: true,
            components: ComponentSelection::default(),
        };

        let files = render_project_files(&config);
        let pyproject = files
            .get(&PathBuf::from("pyproject.toml"))
            .and_then(GeneratedFile::as_text)
            .unwrap();

        // Verify forge metadata is embedded
        assert!(pyproject.contains("[tool.forge]"));
        assert!(pyproject.contains("blueprint = \"python-library>=0.1.0\""));
        assert!(pyproject.contains("project_name = \"meta-test\""));
    }
}
