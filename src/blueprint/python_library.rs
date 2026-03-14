use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

pub const BLUEPRINT_NAME: &str = "python-library";
pub const BLUEPRINT_VERSION: &str = "0.1.0";

#[derive(Clone, Debug)]
pub struct ProjectConfig {
    pub project_name: String,
    pub package_name: String,
    pub description: String,
    pub author_name: String,
    pub author_email: String,
    pub license: String,
    pub python_min: String,
    pub docs: bool,
    pub codecov: bool,
    pub pypi_publish: bool,
}

impl ProjectConfig {
    pub fn validate(&self) -> Result<()> {
        if !is_valid_project_name(&self.project_name) {
            bail!("invalid project name: {}", self.project_name);
        }
        if !is_valid_package_name(&self.package_name) {
            bail!("invalid package name: {}", self.package_name);
        }
        if !self.author_email.contains('@') {
            bail!("invalid author email: {}", self.author_email);
        }
        if !matches!(self.license.as_str(), "BSD-3-Clause" | "MIT" | "Apache-2.0") {
            bail!("license must be BSD-3-Clause, MIT, or Apache-2.0");
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

pub fn render_project_files(config: &ProjectConfig) -> BTreeMap<PathBuf, String> {
    let mut files = BTreeMap::new();

    files.insert(PathBuf::from("README.md"), render_readme(config));
    files.insert(PathBuf::from("LICENSE"), render_license(config));
    files.insert(PathBuf::from(".gitignore"), render_gitignore());
    files.insert(
        PathBuf::from(".python-version"),
        format!("{}\n", config.python_min),
    );
    files.insert(PathBuf::from("pyproject.toml"), render_pyproject(config));
    files.insert(PathBuf::from("justfile"), render_justfile());
    files.insert(
        PathBuf::from(".pre-commit-config.yaml"),
        render_precommit_config(),
    );
    files.insert(PathBuf::from("CHANGELOG.md"), render_changelog());
    files.insert(PathBuf::from("AGENTS.md"), render_agents());
    files.insert(
        PathBuf::from(".github/workflows/ci.yaml"),
        render_ci_workflow(config),
    );
    files.insert(
        PathBuf::from(".github/workflows/release-please.yaml"),
        render_release_please_workflow(),
    );
    files.insert(
        PathBuf::from(".release-please-config.json"),
        render_release_please_config(),
    );
    files.insert(
        PathBuf::from(".release-please-manifest.json"),
        render_release_please_manifest(),
    );

    if config.pypi_publish {
        files.insert(
            PathBuf::from(".github/workflows/publish-pypi.yaml"),
            render_publish_pypi(),
        );
    }

    if config.docs {
        files.insert(PathBuf::from("mkdocs.yml"), render_mkdocs(config));
        files.insert(PathBuf::from("docs/index.md"), render_docs_index(config));
    }

    files.insert(
        PathBuf::from(format!("src/{}/__init__.py", config.package_name)),
        render_init_py(config),
    );
    files.insert(
        PathBuf::from(format!("src/{}/core.py", config.package_name)),
        render_core_py(config),
    );
    files.insert(
        PathBuf::from(format!("src/{}/py.typed", config.package_name)),
        "\n".to_string(),
    );
    files.insert(
        PathBuf::from(format!("tests/test_{}.py", config.package_name)),
        render_test_py(config),
    );

    files
}

pub fn render_managed_files(config: &ProjectConfig) -> BTreeMap<PathBuf, String> {
    let mut files = BTreeMap::new();

    files.insert(PathBuf::from("README.md"), render_readme(config));
    files.insert(PathBuf::from("LICENSE"), render_license(config));
    files.insert(PathBuf::from(".gitignore"), render_gitignore());
    files.insert(
        PathBuf::from(".python-version"),
        format!("{}\n", config.python_min),
    );
    files.insert(PathBuf::from("pyproject.toml"), render_pyproject(config));
    files.insert(PathBuf::from("justfile"), render_justfile());
    files.insert(
        PathBuf::from(".pre-commit-config.yaml"),
        render_precommit_config(),
    );
    files.insert(PathBuf::from("CHANGELOG.md"), render_changelog());
    files.insert(PathBuf::from("AGENTS.md"), render_agents());
    files.insert(
        PathBuf::from(".github/workflows/ci.yaml"),
        render_ci_workflow(config),
    );
    files.insert(
        PathBuf::from(".github/workflows/release-please.yaml"),
        render_release_please_workflow(),
    );
    files.insert(
        PathBuf::from(".release-please-config.json"),
        render_release_please_config(),
    );
    files.insert(
        PathBuf::from(".release-please-manifest.json"),
        render_release_please_manifest(),
    );

    if config.pypi_publish {
        files.insert(
            PathBuf::from(".github/workflows/publish-pypi.yaml"),
            render_publish_pypi(),
        );
    }

    if config.docs {
        files.insert(PathBuf::from("mkdocs.yml"), render_mkdocs(config));
        files.insert(PathBuf::from("docs/index.md"), render_docs_index(config));
    }

    files
}

pub fn clean_optional_files(root: &Path, config: &ProjectConfig) -> Result<()> {
    if !config.docs {
        remove_if_exists(&root.join("mkdocs.yml"))?;
        remove_if_exists(&root.join("docs/index.md"))?;
    }

    if !config.pypi_publish {
        remove_if_exists(&root.join(".github/workflows/publish-pypi.yaml"))?;
    }

    Ok(())
}

fn remove_if_exists(path: &Path) -> Result<()> {
    if path.is_file() {
        std::fs::remove_file(path)
            .with_context(|| format!("failed to remove {}", path.display()))?;
    }
    Ok(())
}

fn render_readme(config: &ProjectConfig) -> String {
    format!(
        "# {}\n\n{}\n\n## Development\n\n```bash\nuv sync --all-groups\njust hooks-install\njust verify\n```\n\n## Forge Metadata\n\nThis project was generated with `forge` blueprint `{}` version `{}`.\n",
        config.project_name, config.description, BLUEPRINT_NAME, BLUEPRINT_VERSION
    )
}

fn render_license(config: &ProjectConfig) -> String {
    let year = "2026";
    match config.license.as_str() {
        "MIT" => format!(
            "MIT License\n\nCopyright (c) {year} {}\n\nPermission is hereby granted, free of charge, to any person obtaining a copy\nof this software and associated documentation files (the \"Software\"), to deal\nin the Software without restriction, including without limitation the rights\nto use, copy, modify, merge, publish, distribute, sublicense, and/or sell\ncopies of the Software, and to permit persons to whom the Software is\nfurnished to do so, subject to the following conditions:\n\nThe above copyright notice and this permission notice shall be included in all\ncopies or substantial portions of the Software.\n\nTHE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR\nIMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,\nFITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE\nAUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER\nLIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,\nOUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE\nSOFTWARE.\n",
            config.author_name
        ),
        "Apache-2.0" => format!(
            "Apache License\nVersion 2.0, January 2004\nhttp://www.apache.org/licenses/\n\nCopyright {year} {}\n\nLicensed under the Apache License, Version 2.0 (the \"License\");\nyou may not use this file except in compliance with the License.\nYou may obtain a copy of the License at\n\n    http://www.apache.org/licenses/LICENSE-2.0\n\nUnless required by applicable law or agreed to in writing, software\ndistributed under the License is distributed on an \"AS IS\" BASIS,\nWITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.\nSee the License for the specific language governing permissions and\nlimitations under the License.\n",
            config.author_name
        ),
        _ => format!(
            "BSD 3-Clause License\n\nCopyright (c) {year}, {}\n\nRedistribution and use in source and binary forms, with or without\nmodification, are permitted provided that the following conditions are met:\n\n1. Redistributions of source code must retain the above copyright notice, this\n   list of conditions and the following disclaimer.\n\n2. Redistributions in binary form must reproduce the above copyright notice,\n   this list of conditions and the following disclaimer in the documentation\n   and/or other materials provided with the distribution.\n\n3. Neither the name of the copyright holder nor the names of its\n   contributors may be used to endorse or promote products derived from\n   this software without specific prior written permission.\n\nTHIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS \"AS IS\"\nAND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE\nIMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE\nDISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE\nFOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL\nDAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR\nSERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER\nCAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,\nOR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE\nOF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.\n",
            config.author_name
        ),
    }
}

fn render_gitignore() -> String {
    "__pycache__/\n*.py[cod]\n.venv/\n.pytest_cache/\n.ruff_cache/\n.mypy_cache/\n.coverage\n.coverage.*\nhtmlcov/\nbuild/\ndist/\n*.egg-info/\nsite/\n".to_string()
}

fn render_pyproject(config: &ProjectConfig) -> String {
    format!(
        r#"[project]
name = "{project_name}"
version = "0.1.0"
description = "{description}"
authors = [{{ name = "{author_name}", email = "{author_email}" }}]
license = {{ file = "LICENSE" }}
readme = "README.md"
requires-python = ">={python_min},<3.15"
dependencies = []

[dependency-groups]
dev = [
    "prek>=0.3.5,<0.4.0",
    "pytest>=8.4.2,<9.0.0",
    "pytest-cov>=7.0.0,<8.0.0",
    "ruff>=0.14.0,<0.15.0",
]
docs = ["mkdocs-material>=9.7.0,<10.0.0"]

[build-system]
requires = ["uv_build>=0.9.5,<0.10.0"]
build-backend = "uv_build"

[tool.uv.build-backend]
module-name = "{package_name}"
module-root = "src"

[tool.pytest.ini_options]
pythonpath = ["src"]
testpaths = ["tests"]
addopts = ["--cov={package_name}", "--cov-report=term-missing:skip-covered"]

[tool.ruff]
line-length = 100

[tool.ruff.lint]
select = ["E", "F", "I", "UP", "B", "SIM", "RUF"]
ignore = ["E501"]

[tool.forge]
blueprint = "{blueprint_name}"
blueprint_version = "{blueprint_version}"
project_name = "{project_name}"
package_name = "{package_name}"
description = "{description}"
author_name = "{author_name}"
author_email = "{author_email}"
license = "{license}"
python_min = "{python_min}"

[tool.forge.options]
docs = {docs}
codecov = {codecov}
pypi_publish = {pypi_publish}
"#,
        project_name = config.project_name,
        description = config.description.replace('"', "'"),
        author_name = config.author_name,
        author_email = config.author_email,
        python_min = config.python_min,
        package_name = config.package_name,
        blueprint_name = BLUEPRINT_NAME,
        blueprint_version = BLUEPRINT_VERSION,
        license = config.license,
        docs = config.docs,
        codecov = config.codecov,
        pypi_publish = config.pypi_publish,
    )
}

fn render_justfile() -> String {
    "set dotenv-load := false\n\ndefault:\n    @just --list\n\nsync:\n    uv sync --all-groups\n\nhooks-install:\n    uv run prek install\n\nformat:\n    uv run ruff format .\n\nlint:\n    uv run ruff check --fix .\n\ntest:\n    uv run pytest -q\n\nbuild:\n    uv build\n\nverify:\n    uv run prek run --all-files\n    uv run pytest --tb=short\n".to_string()
}

fn render_precommit_config() -> String {
    "default_install_hook_types:\n  - pre-commit\n  - pre-push\nrepos:\n  - repo: local\n    hooks:\n      - id: ruff-format\n        name: ruff format\n        entry: uv run ruff format\n        language: system\n        types_or: [python, pyi]\n      - id: ruff-check\n        name: ruff check\n        entry: uv run ruff check --fix\n        language: system\n        types_or: [python, pyi]\n      - id: pytest\n        name: pytest\n        entry: uv run pytest -q --maxfail=1\n        language: system\n        pass_filenames: false\n        stages: [pre-push]\n  - repo: https://github.com/astral-sh/uv-pre-commit\n    rev: 0.9.4\n    hooks:\n      - id: uv-lock\n".to_string()
}

fn render_changelog() -> String {
    "# Changelog\n\nAll notable changes to this project are documented here.\n".to_string()
}

fn render_agents() -> String {
    "# AGENTS\n\nShared project-level instructions for coding agents.\n\n- Follow TDD for feature and bug changes.\n- Keep infrastructure scripts and CI deterministic.\n".to_string()
}

fn render_ci_workflow(config: &ProjectConfig) -> String {
    let mut codecov_step = String::new();
    if config.codecov {
        codecov_step =
            "      - name: Upload coverage to Codecov\n        uses: codecov/codecov-action@v5\n"
                .to_string();
    }
    format!(
        "name: CI\n\non:\n  push:\n    branches: [main]\n  pull_request:\n\njobs:\n  test:\n    runs-on: ubuntu-latest\n    strategy:\n      matrix:\n        python-version: [\"{}\", \"3.12\", \"3.13\"]\n    steps:\n      - uses: actions/checkout@v4\n      - uses: astral-sh/setup-uv@v6\n      - uses: actions/setup-python@v5\n        with:\n          python-version: ${{{{ matrix.python-version }}}}\n      - run: uv sync --all-groups\n      - run: uv run prek run --all-files\n      - run: uv run pytest --cov --cov-report=xml\n{}      - run: uv build\n",
        config.python_min, codecov_step
    )
}

fn render_release_please_workflow() -> String {
    "name: release-please\n\non:\n  push:\n    branches: [main]\n\npermissions:\n  contents: write\n  pull-requests: write\n  issues: write\n\njobs:\n  release-please:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: googleapis/release-please-action@v4\n".to_string()
}

fn render_release_please_config() -> String {
    "{\n  \"release-type\": \"simple\",\n  \"packages\": {\n    \".\": {\n      \"changelog-path\": \"CHANGELOG.md\"\n    }\n  }\n}\n"
        .to_string()
}

fn render_release_please_manifest() -> String {
    "{\n  \".\": \"0.1.0\"\n}\n".to_string()
}

fn render_publish_pypi() -> String {
    "name: publish-pypi\n\non:\n  release:\n    types: [published]\n\npermissions:\n  id-token: write\n  contents: read\n\njobs:\n  publish:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - uses: astral-sh/setup-uv@v6\n      - run: uv build\n      - run: uv publish\n".to_string()
}

fn render_mkdocs(config: &ProjectConfig) -> String {
    format!(
        "site_name: {}\nsite_url: https://{}/{}.github.io\ntheme:\n  name: material\nnav:\n  - Home: index.md\n",
        config.project_name, config.project_name, config.project_name
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
struct ForgeSection {
    blueprint: String,
    blueprint_version: String,
    project_name: String,
    package_name: String,
    description: String,
    author_name: String,
    author_email: String,
    license: String,
    python_min: String,
    options: ForgeOptions,
}

#[derive(Clone, Debug, Deserialize)]
struct ForgeOptions {
    docs: bool,
    codecov: bool,
    pypi_publish: bool,
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

    if forge.blueprint_version.is_empty() {
        bail!("tool.forge.blueprint_version cannot be empty");
    }

    Ok(ProjectConfig {
        project_name: forge.project_name,
        package_name: forge.package_name,
        description: forge.description,
        author_name: forge.author_name,
        author_email: forge.author_email,
        license: forge.license,
        python_min: forge.python_min,
        docs: forge.options.docs,
        codecov: forge.options.codecov,
        pypi_publish: forge.options.pypi_publish,
    })
}
