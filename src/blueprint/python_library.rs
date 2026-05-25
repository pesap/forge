use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::blueprint::agents;
use crate::blueprint::components::{ComponentSelection, ManagedComponent};
use crate::blueprint::files::{GeneratedFile, GeneratedFiles, remove_managed_file_if_exists};
use crate::blueprint::github_actions;
use crate::blueprint::precommit;
use crate::blueprint::readme;
use crate::blueprint::toml_value;
use crate::blueprint::{
    BlueprintName, ManagedOption, managed_option_enabled, validate_managed_options_from_metadata,
};

pub const BLUEPRINT_NAME: &str = "python-library";
pub const BLUEPRINT_VERSION: &str = "0.1.0";
pub const PYPI_PUBLISH_NOTICE: &str =
    "Register this workflow as a trusted publisher in PyPI before uncommenting the publish step.";

#[derive(Clone, Debug)]
pub struct ProjectConfig {
    pub project_name: String,
    pub package_name: String,
    pub description: String,
    pub author_name: Option<String>,
    pub author_email: Option<String>,
    pub license: String,
    pub python_min: String,
    pub docs: bool,
    pub codecov: bool,
    pub pypi_publish: bool,
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
        GeneratedFile::text("\n"),
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
        PathBuf::from("LICENSE"),
        GeneratedFile::text(render_license(config)),
    );
    files.insert(
        PathBuf::from(".gitignore"),
        GeneratedFile::text(render_gitignore()),
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
        GeneratedFile::text(render_release_please_workflow()),
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

    if config.pypi_publish {
        files.insert(
            PathBuf::from(".github/workflows/publish-pypi.yaml"),
            GeneratedFile::text(render_publish_pypi()),
        );
    }

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

    if !config.pypi_publish {
        files.push(PathBuf::from(".github/workflows/publish-pypi.yaml"));
    }

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
    format!(
        "# {}\n\n{}\n\n## Development\n\n```bash\nuv sync --all-groups\njust hooks-install\njust verify\n```\n\n{}## Forge Metadata\n\nThis project was generated with `forge` blueprint `{}`.\n",
        config.project_name,
        config.description,
        readme::automated_update_section(),
        BLUEPRINT_NAME
    )
}

fn render_license(config: &ProjectConfig) -> String {
    let year = "2026";
    let author = author_display_name(config);
    match config.license.as_str() {
        "MIT" => format!(
            "MIT License\n\nCopyright (c) {year} {author}\n\nPermission is hereby granted, free of charge, to any person obtaining a copy\nof this software and associated documentation files (the \"Software\"), to deal\nin the Software without restriction, including without limitation the rights\nto use, copy, modify, merge, publish, distribute, sublicense, and/or sell\ncopies of the Software, and to permit persons to whom the Software is\nfurnished to do so, subject to the following conditions:\n\nThe above copyright notice and this permission notice shall be included in all\ncopies or substantial portions of the Software.\n\nTHE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR\nIMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,\nFITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE\nAUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER\nLIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,\nOUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE\nSOFTWARE.\n",
        ),
        "Apache-2.0" => format!(
            "Apache License\nVersion 2.0, January 2004\nhttp://www.apache.org/licenses/\n\nCopyright {year} {author}\n\nLicensed under the Apache License, Version 2.0 (the \"License\");\nyou may not use this file except in compliance with the License.\nYou may obtain a copy of the License at\n\n    http://www.apache.org/licenses/LICENSE-2.0\n\nUnless required by applicable law or agreed to in writing, software\ndistributed under the License is distributed on an \"AS IS\" BASIS,\nWITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.\nSee the License for the specific language governing permissions and\nlimitations under the License.\n",
        ),
        _ => format!(
            "BSD 3-Clause License\n\nCopyright (c) {year}, {author}\n\nRedistribution and use in source and binary forms, with or without\nmodification, are permitted provided that the following conditions are met:\n\n1. Redistributions of source code must retain the above copyright notice, this\n   list of conditions and the following disclaimer.\n\n2. Redistributions in binary form must reproduce the above copyright notice,\n   this list of conditions and the following disclaimer in the documentation\n   and/or other materials provided with the distribution.\n\n3. Neither the name of the copyright holder nor the names of its\n   contributors may be used to endorse or promote products derived from\n   this software without specific prior written permission.\n\nTHIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS \"AS IS\"\nAND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE\nIMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE\nDISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE\nFOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL\nDAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR\nSERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER\nCAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,\nOR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE\nOF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.\n",
        ),
    }
}

fn render_gitignore() -> String {
    "__pycache__/\n*.py[cod]\n.venv/\n.pytest_cache/\n.ruff_cache/\n.mypy_cache/\n.coverage\n.coverage.*\nhtmlcov/\nbuild/\ndist/\n*.egg-info/\nsite/\n".to_string()
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

fn render_pyproject(config: &ProjectConfig) -> String {
    let authors = render_authors(config);
    let docs_group = render_docs_dependency_group(config.docs);
    format!(
        r#"[project]
name = {project_name}
version = "0.1.0"
description = {description}
{authors}license = {{ file = "LICENSE" }}
readme = "README.md"
requires-python = {requires_python}
dependencies = []

[dependency-groups]
dev = [
    "prek>=0.3.5,<0.4.0",
    "pytest>=8.4.2,<9.0.0",
    "pytest-cov>=7.0.0,<8.0.0",
    "ruff>=0.14.0,<0.15.0",
]
{docs_group}

[build-system]
requires = ["uv_build>=0.9.5,<0.10.0"]
build-backend = "uv_build"

[tool.uv.build-backend]
module-name = {package_name}
module-root = "src"

[tool.pytest.ini_options]
pythonpath = ["src"]
testpaths = ["tests"]
addopts = [{coverage_arg}, "--cov-report=term-missing:skip-covered"]

[tool.ruff]
line-length = 100

[tool.ruff.lint]
select = ["E", "F", "I", "UP", "B", "SIM", "RUF"]
ignore = ["E501"]

[tool.forge]
blueprint = "{blueprint_name}"
blueprint_version = "{blueprint_version}"
project_name = {project_name}
package_name = {package_name}
description = {description}
{author_name}{author_email}license = {license}
python_min = {python_min}

[tool.forge.options]
docs = {docs}
codecov = {codecov}
pypi-publish = {pypi_publish}
prettier = {prettier}
editorconfig = {editorconfig}
markdownlint = {markdownlint}
"#,
        project_name = toml_value::string_literal(&config.project_name),
        description = toml_value::string_literal(&config.description),
        authors = authors,
        requires_python = toml_value::string_literal(&format!(">={},<3.15", config.python_min)),
        docs_group = docs_group,
        package_name = toml_value::string_literal(&config.package_name),
        coverage_arg = toml_value::string_literal(&format!("--cov={}", config.package_name)),
        blueprint_name = BLUEPRINT_NAME,
        blueprint_version = BLUEPRINT_VERSION,
        author_name = render_optional_forge_field("author_name", &config.author_name),
        author_email = render_optional_forge_field("author_email", &config.author_email),
        license = toml_value::string_literal(&config.license),
        python_min = toml_value::string_literal(&config.python_min),
        docs = config.docs,
        codecov = config.codecov,
        pypi_publish = config.pypi_publish,
        prettier = config.components.is_enabled(ManagedComponent::Prettier),
        editorconfig = config.components.is_enabled(ManagedComponent::Editorconfig),
        markdownlint = config.components.is_enabled(ManagedComponent::Markdownlint),
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
    let docs_recipe = if config.docs {
        "\ndocs:\n    uv run mkdocs serve\n"
    } else {
        ""
    };
    let component_format_steps = render_component_format_steps(config);

    format!(
        "set dotenv-load := false\n\ndefault:\n    @just --list\n\nsync:\n    uv sync --all-groups\n\nhooks-install:\n    uv run prek install\n{docs_recipe}\nformat:\n    uv run ruff format .\n{component_format_steps}\nlint:\n    uv run ruff check --fix .\n\ntest:\n    uv run pytest -q\n\nbuild:\n    uv build\n\nverify:\n    uv lock --check\n    uv run --locked ruff format --check .\n    uv run --locked ruff check .\n    uv run --locked prek run --all-files\n    forge update --path . --check\n    uv run --locked pytest --tb=short\n    uv build --locked\n"
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
    let component_hooks = config.components.pre_commit_hooks();

    format!(
        "default_install_hook_types:\n  - pre-commit\n  - pre-push\nrepos:\n  - repo: local\n    hooks:\n      - id: ruff-format\n        name: ruff format check\n        entry: uv run --locked ruff format --check\n        language: system\n        types_or: [python, pyi]\n      - id: ruff-check\n        name: ruff check\n        entry: uv run --locked ruff check\n        language: system\n        types_or: [python, pyi]\n{component_hooks}      - id: pytest\n        name: pytest\n        entry: uv run --locked pytest -q --maxfail=1\n        language: system\n        pass_filenames: false\n        stages: [pre-push]\n{}{}",
        precommit::forge_update_check_hook(),
        precommit::uv_lock_hook()
    )
}

fn render_changelog() -> String {
    "# Changelog\n\nAll notable changes to this project are documented here.\n".to_string()
}

fn render_ci_workflow(config: &ProjectConfig) -> String {
    let codecov_step = if config.codecov {
        "      - name: Upload coverage to Codecov\n        uses: codecov/codecov-action@v6\n"
    } else {
        ""
    };
    let python_matrix = render_python_matrix(&config.python_min);
    format!(
        "name: CI\n\non:\n  push:\n    branches: [main]\n  pull_request:\n\n{}{}jobs:\n  test:\n    runs-on: ubuntu-latest\n{}    strategy:\n      matrix:\n        python-version: [{}]\n    steps:\n{}      - name: Install Rust\n        uses: dtolnay/rust-toolchain@stable\n{}      - uses: actions/setup-python@v6\n        with:\n          python-version: ${{{{ matrix.python-version }}}}\n{}{}{}{}{}{}{}{}{}      - run: uv build --locked\n",
        github_actions::cancel_redundant_ci_concurrency(),
        github_actions::read_only_permissions(),
        github_actions::job_timeout(),
        python_matrix,
        github_actions::read_only_checkout_step(),
        github_actions::setup_uv_step(),
        github_actions::install_forge_step(),
        github_actions::uv_sync_locked_step(),
        github_actions::uv_lock_check_step(),
        github_actions::uv_run_locked_step("ruff format --check ."),
        github_actions::uv_run_locked_step("ruff check ."),
        github_actions::uv_run_locked_step("prek run --all-files"),
        github_actions::forge_update_check_step(),
        github_actions::uv_run_locked_step("pytest --cov --cov-report=xml"),
        codecov_step
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

fn render_release_please_workflow() -> String {
    format!(
        "name: release-please\n\non:\n  push:\n    branches: [main]\n\n{}permissions:\n  contents: write\n  pull-requests: write\n  issues: write\n\njobs:\n  release-please:\n    runs-on: ubuntu-latest\n{}    steps:\n      - uses: googleapis/release-please-action@v5\n",
        github_actions::serialized_ref_concurrency(),
        github_actions::job_timeout()
    )
}

fn render_release_please_config() -> String {
    "{\n  \"release-type\": \"simple\",\n  \"packages\": {\n    \".\": {\n      \"changelog-path\": \"CHANGELOG.md\"\n    }\n  }\n}\n"
        .to_string()
}

fn render_release_please_manifest() -> String {
    "{\n  \".\": \"0.1.0\"\n}\n".to_string()
}

fn render_publish_pypi() -> String {
    format!(
        "name: publish-pypi\n\non:\n  release:\n    types: [published]\n\n{}jobs:\n  publish:\n    runs-on: ubuntu-latest\n    environment:\n      name: pypi\n      url: https://pypi.org/p/<your-pypi-project-name>\n    permissions:\n      id-token: write\n      contents: read\n{}    steps:\n{}{}      - run: uv build --locked\n      # {}\n      # - name: Publish package distributions to PyPI\n      #   uses: pypa/gh-action-pypi-publish@release/v1\n",
        github_actions::serialized_release_concurrency(),
        github_actions::job_timeout(),
        github_actions::read_only_checkout_step(),
        github_actions::setup_uv_step(),
        PYPI_PUBLISH_NOTICE,
    )
}

fn render_mkdocs(config: &ProjectConfig) -> String {
    format!(
        "site_name: {}\ntheme:\n  name: material\nnav:\n  - Home: index.md\n",
        config.project_name
    )
}

fn render_docs_index(config: &ProjectConfig) -> String {
    format!(
        "# {}\n\n{}\n\n## Quickstart\n\n```bash\nuv sync --all-groups\njust verify\n```\n",
        config.project_name, config.description
    )
}

fn render_init_py(config: &ProjectConfig) -> String {
    format!(
        "from importlib.metadata import PackageNotFoundError, version\n\nfrom {}.core import hello\n\ntry:\n    __version__ = version(\"{}\")\nexcept PackageNotFoundError:\n    __version__ = \"0.0.0\"\n\n__all__ = [\"__version__\", \"hello\"]\n",
        config.package_name, config.project_name
    )
}

fn render_core_py(config: &ProjectConfig) -> String {
    format!(
        "def hello() -> str:\n    return \"hello from {}\"\n",
        config.package_name
    )
}

fn render_test_py(config: &ProjectConfig) -> String {
    format!(
        "from importlib import metadata\n\nimport {}\n\n\ndef test_package_imports() -> None:\n    assert {}.__name__ == \"{}\"\n\n\ndef test_package_version_matches_metadata() -> None:\n    assert {}.__version__ == metadata.version(\"{}\")\n\n\ndef test_starter_symbol_exists() -> None:\n    assert {}.hello() == \"hello from {}\"\n",
        config.package_name,
        config.package_name,
        config.package_name,
        config.package_name,
        config.project_name,
        config.package_name,
        config.package_name
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
    let options = validate_managed_options_from_metadata(BlueprintName::PythonLibrary, options)?;

    let config = ProjectConfig {
        project_name: forge.project_name,
        package_name: forge.package_name,
        description: forge.description,
        author_name: forge.author_name,
        author_email: forge.author_email,
        license: forge.license,
        python_min: forge.python_min,
        docs: managed_option_enabled(&options, ManagedOption::Docs)?,
        codecov: managed_option_enabled(&options, ManagedOption::Codecov)?,
        pypi_publish: managed_option_enabled(&options, ManagedOption::PypiPublish)?,
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
            docs: true,
            codecov: false,
            pypi_publish: false,
            components: ComponentSelection::default(),
        };

        let workflow = render_ci_workflow(&config);

        assert!(workflow.contains("python-version: [\"3.13\", \"3.14\"]"));
        assert!(!workflow.contains("\"3.12\""));
    }

    #[test]
    fn just_verify_runs_non_mutating_python_quality_gate_explicitly() {
        let justfile = render_justfile(&test_config(true));

        assert!(justfile.contains("verify:\n    uv lock --check"));
        assert!(justfile.contains("uv run --locked ruff format --check ."));
        assert!(justfile.contains("uv run --locked ruff check ."));
        assert!(justfile.contains("forge update --path . --check"));
        assert!(justfile.contains("uv run --locked pytest --tb=short"));
        assert!(justfile.contains("uv build --locked"));
    }

    #[test]
    fn ci_workflow_runs_non_mutating_python_quality_gate_explicitly() {
        let mut config = test_config(true);
        config.codecov = true;
        let workflow = render_ci_workflow(&config);

        assert!(workflow.contains(&github_actions::uv_run_locked_step("ruff format --check .")));
        assert!(workflow.contains(github_actions::uv_lock_check_step()));
        assert!(workflow.contains(&github_actions::uv_run_locked_step("ruff check .")));
        assert!(workflow.contains("run: forge update --path . --check"));
        assert!(workflow.contains(&github_actions::uv_run_locked_step(
            "pytest --cov --cov-report=xml"
        )));
        assert!(workflow.contains("actions/setup-python@v6"));
        assert!(workflow.contains("codecov/codecov-action@v6"));
        assert!(workflow.contains("run: uv build --locked"));
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
        assert!(precommit.contains("id: forge-update-check"));
        assert!(precommit.contains("forge update --path . --check"));
    }

    #[test]
    fn precommit_config_uses_non_mutating_locked_python_checks() {
        let precommit = render_precommit_config(&test_config(true));

        assert!(precommit.contains("entry: uv run --locked ruff format --check"));
        assert!(precommit.contains("entry: uv run --locked ruff check"));
        assert!(precommit.contains("entry: uv run --locked pytest -q --maxfail=1"));
        assert!(!precommit.contains("uv run ruff format\n"));
        assert!(!precommit.contains("uv run ruff check --fix"));
        assert!(!precommit.contains("uv run pytest -q --maxfail=1"));
    }

    #[test]
    fn mkdocs_config_does_not_invent_site_url() {
        let mkdocs = render_mkdocs(&test_config(true));

        assert!(mkdocs.contains("site_name: test-project"));
        assert!(!mkdocs.contains("site_url:"));
    }

    #[test]
    fn disabled_docs_remove_docs_dependency_and_recipe() {
        let config = test_config(false);

        let pyproject = render_pyproject(&config);
        let justfile = render_justfile(&config);

        assert!(!pyproject.contains("mkdocs-material"));
        assert!(!pyproject.contains("docs = ["));
        assert!(!justfile.contains("\ndocs:\n"));
        assert!(!justfile.contains("mkdocs serve"));
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
            docs,
            codecov: false,
            pypi_publish: false,
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
            docs: true,
            codecov: false,
            pypi_publish: false,
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
            docs: true,
            codecov: false,
            pypi_publish: false,
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
            docs: true,
            codecov: false,
            pypi_publish: false,
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
            docs: true,
            codecov: false,
            pypi_publish: false,
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

[tool.forge.options]
docs = true
codecov = false
pypi-publish = false
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

[tool.forge.options]
docs = true
codecov = true
pypi-publish = false
prettier = false
editorconfig = false
markdownlint = false
"#;

        let error = config_from_pyproject(metadata).expect_err("unknown metadata should fail");

        assert!(error.to_string().contains("failed to parse pyproject.toml"));
    }

    #[test]
    fn config_from_pyproject_rejects_missing_supported_options() {
        let metadata = r#"[tool.forge]
blueprint = "python-library"
project_name = "grid-tools"
package_name = "grid_tools"
description = "Grid tooling"
author_name = "Ada Lovelace"
author_email = "ada@example.com"
license = "MIT"
python_min = "3.12"

[tool.forge.options]
docs = true
codecov = true
pypi-publish = false
"#;

        let error =
            config_from_pyproject(metadata).expect_err("missing supported options should fail");
        assert!(
            error
                .to_string()
                .contains("missing tool.forge.options.prettier")
        );
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
            docs: true,
            codecov: true,
            pypi_publish: false,
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
        assert!(files.contains_key(&PathBuf::from("LICENSE")));
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
        let release_please = render_release_please_workflow();
        let publish_pypi = render_publish_pypi();

        assert!(release_please.contains(github_actions::job_timeout()));
        assert!(release_please.contains("googleapis/release-please-action@v5"));
        assert!(publish_pypi.contains(github_actions::job_timeout()));
        assert!(publish_pypi.contains(github_actions::read_only_checkout_step()));
        assert!(publish_pypi.contains("enable-cache: true"));
        assert!(publish_pypi.contains("run: uv build --locked"));
        assert!(publish_pypi.contains(PYPI_PUBLISH_NOTICE));
        assert!(publish_pypi.contains("# - name: Publish package distributions to PyPI"));
        assert!(publish_pypi.contains("#   uses: pypa/gh-action-pypi-publish@release/v1"));
    }

    #[test]
    fn release_workflows_serialize_duplicate_runs() {
        let release_please = render_release_please_workflow();
        let publish_pypi = render_publish_pypi();

        assert!(release_please.contains("concurrency:\n  group: ${{ github.workflow }}-${{ github.ref }}\n  cancel-in-progress: false\n\npermissions:"));
        assert!(publish_pypi.contains("concurrency:\n  group: ${{ github.workflow }}-${{ github.event.release.id }}\n  cancel-in-progress: false\n\njobs:"));
    }

    #[test]
    fn publish_pypi_workflow_uses_dedicated_environment_and_job_permissions() {
        let publish_pypi = render_publish_pypi();

        assert!(publish_pypi.contains("    environment:\n      name: pypi\n"));
        assert!(publish_pypi.contains("      url: https://pypi.org/p/<your-pypi-project-name>\n"));
        assert!(
            publish_pypi
                .contains("    permissions:\n      id-token: write\n      contents: read\n")
        );
        assert!(!publish_pypi.contains("\npermissions:\n  id-token: write\n"));
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
            docs: false,
            codecov: false,
            pypi_publish: true,
            components: ComponentSelection::default(),
        };

        let files = render_project_files(&config);
        let pyproject = files
            .get(&PathBuf::from("pyproject.toml"))
            .and_then(GeneratedFile::as_text)
            .unwrap();

        // Verify forge metadata is embedded
        assert!(pyproject.contains("[tool.forge]"));
        assert!(pyproject.contains("blueprint = \"python-library\""));
        assert!(pyproject.contains("project_name = \"meta-test\""));
    }
}
