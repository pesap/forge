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

pub const BLUEPRINT_NAME: &str = "rust-library";
pub const BLUEPRINT_VERSION: &str = "0.1.0";

#[derive(Clone, Debug)]
pub struct ProjectConfig {
    pub project_name: String,
    pub crate_name: String,
    pub description: String,
    pub author_name: String,
    pub author_email: String,
    pub license: String,
    pub rust_edition: String,
    pub docs: bool,
    pub components: ComponentSelection,
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
        if !self.author_email.contains('@') {
            bail!("invalid author email: {}", self.author_email);
        }
        if !matches!(self.license.as_str(), "BSD-3-Clause" | "MIT" | "Apache-2.0") {
            bail!("license must be BSD-3-Clause, MIT, or Apache-2.0");
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
        "Preserve user-authored Rust source during managed infrastructure updates.",
    ]));
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
    format!(
        "# {}\n\n{}\n\n## Development\n\n```bash\nuv sync --all-groups\njust hooks-install\njust verify\n```\n\n{}## Forge Metadata\n\nThis project is managed with `forge` blueprint `{}`.\n",
        config.project_name,
        config.description,
        readme::automated_update_section(),
        BLUEPRINT_NAME
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
    "target/\nCargo.lock\n.venv/\n.cache/\n.DS_Store\n".to_string()
}

fn render_cargo_toml(config: &ProjectConfig) -> String {
    format!(
        "[package]\nname = {}\nversion = \"0.1.0\"\nedition = {}\ndescription = {}\nlicense = {}\nauthors = [{}]\n\n[lib]\nname = {}\npath = \"src/lib.rs\"\n\n[dependencies]\n",
        toml_value::string_literal(&config.project_name),
        toml_value::string_literal(&config.rust_edition),
        toml_value::string_literal(&config.description),
        toml_value::string_literal(&config.license),
        toml_value::string_literal(&format!("{} <{}>", config.author_name, config.author_email)),
        toml_value::string_literal(&config.crate_name)
    )
}

fn render_pyproject(config: &ProjectConfig) -> String {
    let docs_group = render_docs_dependency_group(config.docs);
    format!(
        r#"[project]
name = {project_name}
version = "0.1.0"
description = {description}
requires-python = ">=3.11"
dependencies = []

[dependency-groups]
dev = [
    "prek>=0.3.5,<0.4.0",
]
{docs_group}

[tool.forge]
blueprint = "{blueprint_name}"
blueprint_version = "{blueprint_version}"
project_name = {project_name}
crate_name = {crate_name}
description = {description}
author_name = {author_name}
author_email = {author_email}
license = {license}
rust_edition = {rust_edition}

[tool.forge.options]
docs = {docs}
prettier = {prettier}
editorconfig = {editorconfig}
markdownlint = {markdownlint}
"#,
        blueprint_name = BLUEPRINT_NAME,
        blueprint_version = BLUEPRINT_VERSION,
        project_name = toml_value::string_literal(&config.project_name),
        crate_name = toml_value::string_literal(&config.crate_name),
        description = toml_value::string_literal(&config.description),
        author_name = toml_value::string_literal(&config.author_name),
        author_email = toml_value::string_literal(&config.author_email),
        license = toml_value::string_literal(&config.license),
        rust_edition = toml_value::string_literal(&config.rust_edition),
        docs_group = docs_group,
        docs = config.docs,
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
        "set dotenv-load := false\n\ndefault:\n    @just --list\n\nsync:\n    uv sync --all-groups\n\nhooks-install:\n    uv run prek install\n{docs_recipe}\nformat:\n    cargo fmt --all\n{component_format_steps}\nlint:\n    cargo clippy --workspace --all-targets --all-features -- -D warnings\n\ntest:\n    cargo test\n\nverify:\n    uv lock --check\n    cargo fmt --all --check\n    cargo clippy --workspace --all-targets --all-features -- -D warnings\n    uv run --locked prek run --all-files\n    forge update --path . --check\n    cargo test\n"
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
        "default_install_hook_types:\n  - pre-commit\n  - pre-push\nrepos:\n  - repo: local\n    hooks:\n      - id: cargo-fmt\n        name: cargo fmt\n        entry: cargo fmt --all --check\n        language: system\n        pass_filenames: false\n      - id: cargo-clippy\n        name: cargo clippy\n        entry: cargo clippy --workspace --all-targets --all-features -- -D warnings\n        language: system\n        pass_filenames: false\n        stages: [pre-push]\n{component_hooks}{}{}",
        precommit::forge_update_check_hook(),
        precommit::uv_lock_hook()
    )
}

fn render_ci_workflow() -> String {
    format!(
        "name: CI\n\non:\n  push:\n    branches: [main]\n  pull_request:\n\n{}{}jobs:\n  verify:\n    runs-on: ubuntu-latest\n{}    steps:\n{}      - name: Install Rust\n        uses: dtolnay/rust-toolchain@stable\n{}{}{}{}      - run: cargo fmt --all --check\n      - run: cargo clippy --workspace --all-targets --all-features -- -D warnings\n{}{}      - run: cargo test\n",
        github_actions::cancel_redundant_ci_concurrency(),
        github_actions::read_only_permissions(),
        github_actions::job_timeout(),
        github_actions::read_only_checkout_step(),
        github_actions::setup_uv_step(),
        github_actions::install_forge_step(),
        github_actions::uv_sync_locked_step(),
        github_actions::uv_lock_check_step(),
        github_actions::uv_run_locked_step("prek run --all-files"),
        github_actions::forge_update_check_step()
    )
}

fn render_lib_rs(config: &ProjectConfig) -> String {
    format!(
        "pub fn hello() -> &'static str {{\n    \"hello from {}\"\n}}\n\n#[cfg(test)]\nmod tests {{\n    use crate::hello;\n\n    #[test]\n    fn hello_returns_project_message() {{\n        assert_eq!(hello(), \"hello from {}\");\n    }}\n}}\n",
        config.crate_name, config.crate_name
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
        "# {}\n\n{}\n\n## Development\n\n```bash\nuv sync --all-groups\njust verify\n```\n\n## API Documentation\n\n```bash\ncargo doc --open\n```\n",
        config.project_name, config.description
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
    author_name: String,
    author_email: String,
    license: String,
    rust_edition: String,
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
    let options = validate_managed_options_from_metadata(BlueprintName::RustLibrary, options)?;

    let config = ProjectConfig {
        project_name: forge.project_name,
        crate_name: forge.crate_name,
        description: forge.description,
        author_name: forge.author_name,
        author_email: forge.author_email,
        license: forge.license,
        rust_edition: forge.rust_edition,
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
    fn just_verify_runs_full_rust_quality_gate_explicitly() {
        let justfile = render_justfile(&test_config(true));

        assert!(justfile.contains("verify:\n    uv lock --check"));
        assert!(justfile.contains("cargo fmt --all --check"));
        assert!(
            justfile
                .contains("cargo clippy --workspace --all-targets --all-features -- -D warnings")
        );
        assert!(justfile.contains("uv run --locked prek run --all-files"));
        assert!(justfile.contains("forge update --path . --check"));
        assert!(justfile.contains("cargo test"));
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

    #[test]
    fn prettier_component_formats_with_just_and_checks_in_hooks() {
        let mut config = test_config(true);
        config.components = ComponentSelection::from_prettier(true);

        let justfile = render_justfile(&config);
        let precommit = render_precommit_config(&config);

        assert!(justfile.contains("cargo fmt --all"));
        assert!(justfile.contains("npx --yes prettier@3.8.3 --write --ignore-unknown ."));
        assert!(precommit.contains("npx --yes prettier@3.8.3 --check --ignore-unknown"));
        assert!(!precommit.contains("npx --yes prettier@3.8.3 --write --ignore-unknown"));
    }

    fn test_config(docs: bool) -> ProjectConfig {
        ProjectConfig {
            project_name: "test-rs".to_string(),
            crate_name: "test_rs".to_string(),
            description: "A test project".to_string(),
            author_name: "Test User".to_string(),
            author_email: "test@example.com".to_string(),
            license: "MIT".to_string(),
            rust_edition: "2024".to_string(),
            docs,
            components: ComponentSelection::default(),
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
        assert!(workflow.contains("run: forge update --path . --check"));
        assert!(workflow.contains("run: cargo test"));
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

[tool.forge.options]
docs = true
prettier = false
editorconfig = false
markdownlint = false
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
blueprint = "rust-library"
project_name = "test-rs"
crate_name = "test_rs"
description = "A test project"
author_name = "Test User"
author_email = "test@example.com"
license = "MIT"
rust_edition = "2024"

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
