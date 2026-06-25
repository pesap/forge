use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::ValueEnum;
use serde::Serialize;
use toml::Value;

use crate::blueprint::components::ManagedComponent;
use crate::blueprint::files::GeneratedFiles;
use crate::errors::{ErrorCode, coded_error};

pub mod agents;
pub mod any_project;
pub mod components;
pub mod files;
pub mod gitattributes;
pub mod github_actions;
pub mod precommit;
pub mod python_library;
pub mod rust_library;
pub mod template_engine;
pub mod toml_value;

pub const DEFAULT_LICENSE: &str = "BSD-3-Clause";
pub const SUPPORTED_LICENSES: [&str; 5] =
    [DEFAULT_LICENSE, "MIT", "Apache-2.0", "BSD-2-Clause", "ISC"];
pub const DEFAULT_BRANCH: &str = "main";

pub fn is_supported_license(value: &str) -> bool {
    SUPPORTED_LICENSES.contains(&value)
}

pub fn supported_license_message() -> String {
    format!("license must be one of: {}", SUPPORTED_LICENSES.join(", "))
}

pub fn is_valid_default_branch(value: &str) -> bool {
    !value.is_empty()
        && value == value.trim()
        && !matches!(value.as_bytes().first(), Some(b'-' | b'/' | b'.'))
        && !matches!(value.as_bytes().last(), Some(b'/' | b'.'))
        && !value.ends_with(".lock")
        && value != "@"
        && !value.contains("..")
        && !value.contains("//")
        && !value.contains("@{")
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/'))
}

pub fn default_branch_message() -> &'static str {
    "default branch must be a valid branch name using letters, numbers, '-', '_', '.', or '/'"
}

const ANY_PROJECT_FIELDS: &[BlueprintField] = &[
    BlueprintField::required(
        "project-name",
        "Project distribution name, for example my-project",
    ),
    BlueprintField::required("description", "Short project description"),
];

const PYTHON_LIBRARY_FIELDS: &[BlueprintField] = &[
    BlueprintField::required(
        "project-name",
        "Project distribution name, for example my-library",
    ),
    BlueprintField::defaulted(
        "package-name",
        "derived from project-name",
        "Python import package name",
    ),
    BlueprintField::required("description", "Short project description"),
    BlueprintField::optional("author-name", "Package author name"),
    BlueprintField::optional("author-email", "Package author email"),
    BlueprintField::defaulted("license", "BSD-3-Clause", "SPDX license identifier"),
    BlueprintField::defaulted("python-min", "3.11", "Minimum supported Python version"),
    BlueprintField::defaulted(
        "gitignore-profile",
        "python,macos,visualstudiocode,jetbrains,node",
        "Comma-separated Toptal gitignore profile",
    ),
];

const RUST_LIBRARY_FIELDS: &[BlueprintField] = &[
    BlueprintField::required("project-name", "Cargo package name, for example my-library"),
    BlueprintField::defaulted(
        "package-name",
        "derived from project-name",
        "Rust crate library name",
    ),
    BlueprintField::required("description", "Short project description"),
    BlueprintField::optional("author-name", "Package author name"),
    BlueprintField::optional("author-email", "Package author email"),
    BlueprintField::defaulted("license", "BSD-3-Clause", "SPDX license identifier"),
];

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum BlueprintName {
    AnyProject,
    PythonLibrary,
    RustLibrary,
}

impl BlueprintName {
    pub const ALL: [Self; 3] = [Self::AnyProject, Self::PythonLibrary, Self::RustLibrary];

    pub fn definition(self) -> &'static BlueprintDefinition {
        BLUEPRINT_REGISTRY
            .iter()
            .find(|blueprint| blueprint.id == self)
            .expect("blueprint enum variant must be present in registry")
    }

    pub fn as_str(self) -> &'static str {
        self.definition().name
    }

    pub fn description(self) -> &'static str {
        self.definition().description
    }

    pub fn summary(self) -> &'static str {
        self.definition().summary
    }

    pub fn version(self) -> &'static str {
        self.definition().version
    }

    pub fn supported_options(self) -> &'static [ManagedOption] {
        self.definition().options
    }

    pub fn creation_fields(self) -> &'static [BlueprintField] {
        self.definition().fields
    }

    pub fn supports_option(self, option: ManagedOption) -> bool {
        self.supported_options().contains(&option)
    }

    pub fn option_default_enabled(self, option: ManagedOption) -> bool {
        matches!(
            (self, option),
            (_, ManagedOption::Docs)
                | (_, ManagedOption::Ci)
                | (_, ManagedOption::ForgeSync)
                | (_, ManagedOption::Editorconfig)
                | (Self::PythonLibrary, ManagedOption::DocsPages)
                | (Self::PythonLibrary, ManagedOption::Codecov)
                | (Self::PythonLibrary, ManagedOption::PythonRules)
                | (Self::PythonLibrary, ManagedOption::WorkflowQuality)
                | (Self::RustLibrary, ManagedOption::RustRules)
        )
    }

    pub fn from_metadata(value: &str) -> Result<Self> {
        Self::from_metadata_with_error_code(value, ErrorCode::Env)
    }

    pub fn from_metadata_with_error_code(value: &str, error_code: ErrorCode) -> Result<Self> {
        Ok(BlueprintSpec::parse(value, error_code)?.name)
    }

    pub fn render_managed_files_from_pyproject(self, content: &str) -> Result<GeneratedFiles> {
        (self.definition().render_managed_files)(content)
    }

    pub fn clean_optional_files_from_pyproject(self, root: &Path, content: &str) -> Result<()> {
        (self.definition().clean_optional_files)(root, content)
    }

    pub fn optional_cleanup_paths_from_pyproject(self, content: &str) -> Result<Vec<PathBuf>> {
        (self.definition().optional_cleanup_paths)(content)
    }
}

pub struct BlueprintDefinition {
    pub id: BlueprintName,
    pub name: &'static str,
    pub version: &'static str,
    pub summary: &'static str,
    pub description: &'static str,
    pub fields: &'static [BlueprintField],
    pub options: &'static [ManagedOption],
    pub required_tools: &'static [&'static str],
    render_managed_files: fn(&str) -> Result<GeneratedFiles>,
    clean_optional_files: fn(&Path, &str) -> Result<()>,
    optional_cleanup_paths: fn(&str) -> Result<Vec<PathBuf>>,
}

pub const BLUEPRINT_REGISTRY: [BlueprintDefinition; 3] = [
    BlueprintDefinition {
        id: BlueprintName::AnyProject,
        name: any_project::BLUEPRINT_NAME,
        version: any_project::BLUEPRINT_VERSION,
        summary: "managed infrastructure for any repository",
        description: "any-project - managed infrastructure for any repository",
        fields: ANY_PROJECT_FIELDS,
        options: &[
            ManagedOption::Docs,
            ManagedOption::Ci,
            ManagedOption::ForgeSync,
            ManagedOption::Prettier,
            ManagedOption::Editorconfig,
            ManagedOption::Markdownlint,
        ],
        required_tools: &["uv", "just"],
        render_managed_files: any_project::render_managed_files_from_pyproject,
        clean_optional_files: any_project::clean_optional_files_from_pyproject,
        optional_cleanup_paths: any_project::optional_cleanup_paths_from_pyproject,
    },
    BlueprintDefinition {
        id: BlueprintName::PythonLibrary,
        name: python_library::BLUEPRINT_NAME,
        version: python_library::BLUEPRINT_VERSION,
        summary: "Python package with uv, pytest, Ruff, and CI",
        description: "python-library - Python package with uv, pytest, Ruff, and CI",
        fields: PYTHON_LIBRARY_FIELDS,
        options: &[
            ManagedOption::Docs,
            ManagedOption::Ci,
            ManagedOption::ForgeSync,
            ManagedOption::DocsPages,
            ManagedOption::WorkflowQuality,
            ManagedOption::Codecov,
            ManagedOption::PypiPublish,
            ManagedOption::PythonRules,
            ManagedOption::Prettier,
            ManagedOption::Editorconfig,
            ManagedOption::Markdownlint,
        ],
        required_tools: &["uv", "just"],
        render_managed_files: python_library::render_managed_files_from_pyproject,
        clean_optional_files: python_library::clean_optional_files_from_pyproject,
        optional_cleanup_paths: python_library::optional_cleanup_paths_from_pyproject,
    },
    BlueprintDefinition {
        id: BlueprintName::RustLibrary,
        name: rust_library::BLUEPRINT_NAME,
        version: rust_library::BLUEPRINT_VERSION,
        summary: "Rust library with Cargo, fmt, clippy, and CI",
        description: "rust-library - Rust library with Cargo, fmt, clippy, and CI",
        fields: RUST_LIBRARY_FIELDS,
        options: &[
            ManagedOption::Docs,
            ManagedOption::Ci,
            ManagedOption::ForgeSync,
            ManagedOption::RustRules,
            ManagedOption::Prettier,
            ManagedOption::Editorconfig,
            ManagedOption::Markdownlint,
        ],
        required_tools: &["cargo", "uv", "just"],
        render_managed_files: rust_library::render_managed_files_from_pyproject,
        clean_optional_files: rust_library::clean_optional_files_from_pyproject,
        optional_cleanup_paths: rust_library::optional_cleanup_paths_from_pyproject,
    },
];

#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BlueprintField {
    pub name: &'static str,
    pub required: bool,
    pub default: Option<&'static str>,
    pub description: &'static str,
}

impl BlueprintField {
    pub const fn required(name: &'static str, description: &'static str) -> Self {
        Self {
            name,
            required: true,
            default: None,
            description,
        }
    }

    pub const fn defaulted(
        name: &'static str,
        default: &'static str,
        description: &'static str,
    ) -> Self {
        Self {
            name,
            required: false,
            default: Some(default),
            description,
        }
    }

    pub const fn optional(name: &'static str, description: &'static str) -> Self {
        Self {
            name,
            required: false,
            default: None,
            description,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ManagedOption {
    Docs,
    Ci,
    ForgeSync,
    DocsPages,
    WorkflowQuality,
    Prettier,
    Editorconfig,
    Markdownlint,
    Codecov,
    PypiPublish,
    PythonRules,
    RustRules,
}

impl ManagedOption {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Docs => "docs",
            Self::Ci => "ci",
            Self::ForgeSync => "forge-sync",
            Self::DocsPages => "docs-pages",
            Self::WorkflowQuality => "workflow-quality",
            Self::Prettier => ManagedComponent::Prettier.option_name(),
            Self::Editorconfig => ManagedComponent::Editorconfig.option_name(),
            Self::Markdownlint => ManagedComponent::Markdownlint.option_name(),
            Self::Codecov => "codecov",
            Self::PypiPublish => "pypi-publish",
            Self::PythonRules => "python-rules",
            Self::RustRules => "rust-rules",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Docs => "Starlight documentation site and docs index",
            Self::Ci => "GitHub Actions CI workflow",
            Self::ForgeSync => "Scheduled Forge-managed infrastructure sync workflow",
            Self::DocsPages => "GitHub Pages documentation deployment workflow",
            Self::WorkflowQuality => "GitHub Actions workflow linting workflow",
            Self::Prettier => ManagedComponent::Prettier.description(),
            Self::Editorconfig => ManagedComponent::Editorconfig.description(),
            Self::Markdownlint => ManagedComponent::Markdownlint.description(),
            Self::Codecov => "Codecov coverage upload step in CI",
            Self::PypiPublish => "Trusted PyPI publishing workflow for releases",
            Self::PythonRules => "Python-specific pre-commit hooks (ruff, ty, pytest)",
            Self::RustRules => "Rust-specific pre-commit hooks (cargo fmt, cargo clippy)",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        Self::parse_with_error_code(value, ErrorCode::Input)
    }

    pub fn parse_with_error_code(value: &str, error_code: ErrorCode) -> Result<Self> {
        match value {
            "docs" => Ok(Self::Docs),
            "ci" => Ok(Self::Ci),
            "forge-sync" => Ok(Self::ForgeSync),
            "docs-pages" => Ok(Self::DocsPages),
            "workflow-quality" => Ok(Self::WorkflowQuality),
            "prettier" => Ok(Self::Prettier),
            "editorconfig" => Ok(Self::Editorconfig),
            "markdownlint" => Ok(Self::Markdownlint),
            "codecov" => Ok(Self::Codecov),
            "pypi-publish" => Ok(Self::PypiPublish),
            "python-rules" => Ok(Self::PythonRules),
            "rust-rules" => Ok(Self::RustRules),
            other => Err(coded_error(
                error_code,
                format!("unsupported managed option '{other}'"),
            )),
        }
    }
}

pub type ManagedOptionValues = BTreeMap<ManagedOption, bool>;

pub fn validate_managed_options(
    blueprint: BlueprintName,
    options: BTreeMap<String, bool>,
) -> Result<ManagedOptionValues> {
    validate_managed_options_with_error_code(blueprint, options, ErrorCode::Input, false)
}

pub fn validate_managed_options_from_metadata(
    blueprint: BlueprintName,
    options: BTreeMap<String, bool>,
) -> Result<ManagedOptionValues> {
    validate_managed_options_with_error_code(blueprint, options, ErrorCode::Env, false)
}

pub fn validate_managed_overrides_from_metadata(
    blueprint: BlueprintName,
    overrides: BTreeMap<String, bool>,
) -> Result<ManagedOptionValues> {
    validate_managed_options_with_error_code(blueprint, overrides, ErrorCode::Env, true)
}

pub fn apply_ignored_files(files: &mut GeneratedFiles, ignored_files: &[String]) {
    files.retain(|path, _| {
        let path = path.to_string_lossy();
        !ignored_files.iter().any(|ignored| {
            path == ignored.as_str()
                || ignored
                    .strip_suffix('/')
                    .is_some_and(|prefix| path.starts_with(&format!("{prefix}/")))
        })
    });
}

pub fn render_forge_ignore(ignored_files: &[String]) -> String {
    if ignored_files.is_empty() {
        return String::new();
    }

    render_toml_string_array_assignment("ignore", ignored_files)
}

pub(crate) fn render_toml_string_array_assignment(key: &str, values: &[String]) -> String {
    let rendered_values = values
        .iter()
        .map(|path| toml_value::string_literal(path))
        .collect::<Vec<_>>()
        .join(", ");
    let inline = format!("{key} = [{rendered_values}]");
    if inline.len() <= 80 {
        return format!("{inline}\n");
    }

    let entries = values
        .iter()
        .map(|path| format!("  {},\n", toml_value::string_literal(path)))
        .collect::<String>();
    format!("{key} = [\n{entries}]\n")
}

pub fn render_forge_overrides_table(
    blueprint: BlueprintName,
    options: &[(ManagedOption, bool)],
) -> String {
    let overrides = options
        .iter()
        .filter(|(option, enabled)| blueprint.option_default_enabled(*option) != *enabled)
        .map(|(option, enabled)| {
            format!(
                "{} = {}\n",
                option.as_str(),
                if *enabled { "true" } else { "false" }
            )
        })
        .collect::<String>();

    if overrides.is_empty() {
        String::new()
    } else {
        format!("\n[tool.forge.overrides]\n{overrides}")
    }
}

pub(crate) fn forge_metadata_blueprint(forge_metadata: &str) -> Option<String> {
    let forge_table = forge_metadata
        .find("\n[tool.forge.overrides]")
        .map(|index| &forge_metadata[..index])
        .unwrap_or(forge_metadata);
    toml::from_str::<Value>(forge_table)
        .ok()
        .and_then(|parsed| {
            parsed
                .get("tool")
                .and_then(Value::as_table)
                .and_then(|tool| tool.get("forge"))
                .and_then(Value::as_table)
                .and_then(|forge| forge.get("blueprint"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
}

pub(crate) fn forge_metadata_is_python_library(forge_metadata: &str) -> bool {
    forge_metadata_blueprint(forge_metadata)
        .as_deref()
        .is_some_and(|blueprint| {
            blueprint == "python-library" || blueprint.starts_with("python-library>=")
        })
}

pub(crate) fn minimal_external_pyproject_metadata(forge_metadata: &str) -> String {
    let overrides = forge_metadata
        .find("\n[tool.forge.overrides]")
        .map(|index| forge_metadata.split_at(index).1)
        .unwrap_or("");
    let blueprint = forge_metadata_blueprint(forge_metadata)
        .unwrap_or_else(|| "python-library>=0.1.0".to_string());
    let mut metadata = format!(
        "[tool.forge]\nblueprint = {}\npyproject = \"external\"\n",
        toml_value::string_literal(&blueprint)
    );
    if let Ok(parsed) = toml::from_str::<Value>(forge_metadata)
        && let Some(forge) = parsed
            .get("tool")
            .and_then(Value::as_table)
            .and_then(|tool| tool.get("forge"))
            .and_then(Value::as_table)
    {
        if let Some(license) = forge.get("license").and_then(Value::as_str)
            && license != "BSD-3-Clause"
        {
            metadata.push_str(&format!(
                "license = {}\n",
                toml_value::string_literal(license)
            ));
        }
        if let Some(default_branch) = forge.get("default_branch").and_then(Value::as_str)
            && default_branch != DEFAULT_BRANCH
        {
            metadata.push_str(&format!(
                "default_branch = {}\n",
                toml_value::string_literal(default_branch)
            ));
        }
        if let Some(profile) = forge.get("gitignore_profile") {
            if let Some(profile) = profile.as_str() {
                metadata.push_str(&format!(
                    "gitignore_profile = {}\n",
                    toml_value::string_literal(profile)
                ));
            } else if let Some(entries) = profile.as_array() {
                let entries = entries
                    .iter()
                    .filter_map(Value::as_str)
                    .map(toml_value::string_literal)
                    .collect::<Vec<_>>()
                    .join(", ");
                if !entries.is_empty() {
                    metadata.push_str(&format!("gitignore_profile = [{entries}]\n"));
                }
            }
        }
        if let Some(ignore) = forge.get("ignore").and_then(Value::as_array) {
            let ignored = ignore
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>();
            if !ignored.is_empty() {
                metadata.push_str(&render_toml_string_array_assignment("ignore", &ignored));
            }
        }
    }
    let overrides = overrides.trim_start_matches('\n');
    if !overrides.is_empty() && !metadata.ends_with("\n\n") {
        metadata.push('\n');
    }
    metadata.push_str(overrides);
    metadata
}

fn validate_managed_options_with_error_code(
    blueprint: BlueprintName,
    options: BTreeMap<String, bool>,
    error_code: ErrorCode,
    fill_missing_defaults: bool,
) -> Result<ManagedOptionValues> {
    let mut values = ManagedOptionValues::new();

    for (name, enabled) in options {
        let option = ManagedOption::parse_with_error_code(&name, error_code)?;
        if !blueprint.supports_option(option) {
            return Err(coded_error(
                error_code,
                format!(
                    "option '{}' is not supported by {}",
                    option.as_str(),
                    blueprint.as_str()
                ),
            ));
        }
        values.insert(option, enabled);
    }

    for option in blueprint.supported_options() {
        if !values.contains_key(option) {
            if fill_missing_defaults {
                values.insert(*option, blueprint.option_default_enabled(*option));
            } else {
                return Err(coded_error(
                    error_code,
                    format!("missing tool.forge.options.{}", option.as_str()),
                ));
            }
        }
    }

    Ok(values)
}

pub fn managed_option_enabled(
    options: &ManagedOptionValues,
    option: ManagedOption,
) -> Result<bool> {
    options
        .get(&option)
        .copied()
        .with_context(|| format!("missing tool.forge.options.{}", option.as_str()))
}

pub fn detect_blueprint_from_pyproject(content: &str) -> Result<BlueprintName> {
    Ok(detect_blueprint_metadata_from_pyproject(content)?.name)
}

pub fn detect_blueprint_metadata_from_pyproject(content: &str) -> Result<BlueprintMetadata> {
    let parsed: Value = toml::from_str(content).map_err(|error| {
        coded_error(
            ErrorCode::Env,
            format!("failed to parse pyproject.toml: {error}"),
        )
    })?;
    let forge = parsed
        .get("tool")
        .and_then(Value::as_table)
        .and_then(|tool| tool.get("forge"))
        .and_then(Value::as_table)
        .ok_or_else(|| coded_error(ErrorCode::Env, "missing [tool.forge] blueprint metadata"))?;

    let blueprint = forge
        .get("blueprint")
        .and_then(Value::as_str)
        .ok_or_else(|| coded_error(ErrorCode::Env, "missing tool.forge.blueprint"))?;
    let spec = BlueprintSpec::parse(blueprint, ErrorCode::Env)?;

    // Read version: spec first, then legacy blueprint_version key.
    let version = spec
        .version
        .map(|v| v.to_string())
        .or_else(|| forge.get("blueprint_version").and_then(Value::as_str).map(str::to_string))
        .ok_or_else(|| coded_error(ErrorCode::Env, "missing tool.forge.blueprint version; use 'blueprint = \"name>=version\"' or add blueprint_version"))?;
    validate_blueprint_version_compatibility(spec.name, &version)?;

    Ok(BlueprintMetadata {
        name: spec.name,
        version: Some(version),
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlueprintMetadata {
    pub name: BlueprintName,
    pub version: Option<String>,
}

fn validate_blueprint_version_compatibility(blueprint: BlueprintName, version: &str) -> Result<()> {
    let parsed = BlueprintVersion::parse(version, ErrorCode::Env)?;
    let supported = BlueprintVersion::parse(blueprint.version(), ErrorCode::Internal)?;
    if parsed > supported {
        return Err(coded_error(
            ErrorCode::Env,
            format!(
                "blueprint version {version} for {} is newer than this forge supports ({supported}); upgrade forge before running managed commands",
                blueprint.as_str()
            ),
        ));
    }
    Ok(())
}

/// Parsed blueprint specification like "python-library>=0.1.0".
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlueprintSpec {
    pub name: BlueprintName,
    pub version: Option<BlueprintVersion>,
}

impl BlueprintSpec {
    /// Parse "python-library>=0.1.0".
    pub fn parse(raw: &str, error_code: ErrorCode) -> Result<Self> {
        let (name_str, version_str) = raw
            .split_once(">=")
            .or_else(|| raw.split_once("=="))
            .map(|(name, version)| (name, Some(version)))
            .unwrap_or((raw, None));

        let name = BLUEPRINT_REGISTRY
            .iter()
            .find(|bp| bp.name == name_str.trim())
            .map(|bp| bp.id)
            .ok_or_else(|| {
                coded_error(error_code, format!("unsupported blueprint '{name_str}'"))
            })?;

        let version = version_str
            .map(|v| BlueprintVersion::parse(v.trim(), error_code))
            .transpose()?;

        Ok(Self { name, version })
    }

    /// Parse and validate the blueprint type matches `expected`.
    pub fn parse_for(expected: BlueprintName, raw: &str, error_code: ErrorCode) -> Result<Self> {
        let spec = Self::parse(raw, error_code)?;
        if spec.name != expected {
            return Err(coded_error(
                error_code,
                format!(
                    "unsupported blueprint '{}' (expected '{}')",
                    raw,
                    expected.as_str()
                ),
            ));
        }
        Ok(spec)
    }
}

/// Semantic version for blueprint compatibility checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub struct BlueprintVersion {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl BlueprintVersion {
    /// Parse "X.Y.Z" version string.
    pub fn parse(value: &str, error_code: ErrorCode) -> Result<Self> {
        let mut parts = value.split('.');
        let major = next_part(&mut parts, value, error_code)?;
        let minor = next_part(&mut parts, value, error_code)?;
        let patch = next_part(&mut parts, value, error_code)?;
        if parts.next().is_some() {
            return Err(coded_error(
                error_code,
                format!("invalid blueprint version '{value}'"),
            ));
        }
        Ok(Self {
            major,
            minor,
            patch,
        })
    }
}

impl fmt::Display for BlueprintVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

fn next_part(
    parts: &mut dyn Iterator<Item = &str>,
    value: &str,
    error_code: ErrorCode,
) -> Result<u64> {
    parts
        .next()
        .ok_or_else(|| coded_error(error_code, format!("invalid blueprint version '{value}'")))?
        .parse::<u64>()
        .map_err(|_| coded_error(error_code, format!("invalid blueprint version '{value}'")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::{CodedError, ErrorCode};
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn detects_supported_blueprint_from_metadata() {
        let metadata = "[tool.forge]\nblueprint = \"any-project\"\nblueprint_version = \"0.1.0\"\n";

        let blueprint =
            detect_blueprint_from_pyproject(metadata).expect("metadata should be supported");

        assert_eq!(blueprint, BlueprintName::AnyProject);
    }

    #[test]
    fn detects_blueprint_version_when_present() {
        let metadata = "[tool.forge]\nblueprint = \"any-project\"\nblueprint_version = \"0.1.0\"\n";

        let parsed =
            detect_blueprint_metadata_from_pyproject(metadata).expect("metadata should parse");

        assert_eq!(parsed.name, BlueprintName::AnyProject);
        assert_eq!(parsed.version.as_deref(), Some("0.1.0"));
    }

    #[test]
    fn rejects_missing_blueprint_version_from_metadata() {
        let metadata = "[tool.forge]\nblueprint = \"python-library\"\n";

        let error = detect_blueprint_metadata_from_pyproject(metadata)
            .expect_err("missing blueprint version should fail");
        assert!(
            error
                .to_string()
                .contains("missing tool.forge.blueprint version")
        );
    }

    #[test]
    fn rejects_invalid_blueprint_version_format() {
        let metadata = "[tool.forge]\nblueprint = \"python-library\"\nblueprint_version = \"1\"\n";

        let error =
            detect_blueprint_metadata_from_pyproject(metadata).expect_err("invalid version fails");

        assert!(error.to_string().contains("invalid blueprint version '1'"));
    }

    #[test]
    fn rejects_newer_blueprint_version_than_supported() {
        let metadata =
            "[tool.forge]\nblueprint = \"python-library\"\nblueprint_version = \"9.0.0\"\n";

        let error =
            detect_blueprint_metadata_from_pyproject(metadata).expect_err("future version fails");

        assert!(error.to_string().contains("newer than this forge supports"));
        assert!(error.to_string().contains("upgrade forge"));
    }

    #[test]
    fn rejects_non_string_blueprint_version() {
        // Legacy blueprint_version as integer is rejected because Value::as_str returns None.
        let metadata = "[tool.forge]\nblueprint = \"python-library\"\nblueprint_version = 1\n";

        let error = detect_blueprint_metadata_from_pyproject(metadata)
            .expect_err("non-string blueprint_version fails");

        assert!(
            error
                .to_string()
                .contains("missing tool.forge.blueprint version")
        );
    }

    #[test]
    fn registry_has_one_definition_for_each_blueprint() {
        let registry_ids = BLUEPRINT_REGISTRY
            .iter()
            .map(|blueprint| blueprint.id.as_str())
            .collect::<BTreeSet<_>>();
        let all_ids = BlueprintName::ALL
            .iter()
            .map(|blueprint| blueprint.as_str())
            .collect::<BTreeSet<_>>();

        assert_eq!(registry_ids, all_ids);
    }

    #[test]
    fn forge_ignore_uses_multiline_toml_for_long_arrays() {
        let ignored_files = vec![
            "codecov".to_string(),
            "ci".to_string(),
            "forge-sync".to_string(),
            "workflow-quality".to_string(),
            "docs-pages".to_string(),
            ".github/workflows/release-please.yaml".to_string(),
        ];

        let rendered = render_forge_ignore(&ignored_files);

        assert!(rendered.starts_with("ignore = [\n"));
        assert!(rendered.contains("  \"workflow-quality\",\n"));
        assert!(rendered.ends_with("]\n"));
        toml::from_str::<Value>(&rendered).expect("ignore array should be valid TOML");
    }

    #[test]
    fn definition_lookup_matches_registry_metadata() {
        for blueprint in BlueprintName::ALL {
            let definition = blueprint.definition();
            assert_eq!(definition.id, blueprint);
            assert_eq!(definition.name, blueprint.as_str());
        }
    }

    #[test]
    fn registry_metadata_names_are_unique_and_resolvable() {
        let mut names = BTreeSet::new();

        for definition in &BLUEPRINT_REGISTRY {
            assert!(names.insert(definition.name));
            assert_eq!(
                BlueprintName::from_metadata(definition.name).expect("blueprint should resolve"),
                definition.id
            );
            assert!(!definition.fields.is_empty());
            assert!(!definition.required_tools.is_empty());
        }
    }

    #[test]
    fn rejects_unknown_blueprint_from_metadata() {
        let metadata = "[tool.forge]\nblueprint = \"web-app\"\n";

        let error = detect_blueprint_from_pyproject(metadata).expect_err("blueprint should fail");

        assert!(error.to_string().contains("unsupported blueprint"));
    }

    #[test]
    fn metadata_option_validation_reports_env_error_code() {
        let options = BTreeMap::from([(String::from("prettier_typo"), true)]);
        let error = validate_managed_options_from_metadata(BlueprintName::AnyProject, options)
            .expect_err("unknown metadata option should fail");

        let coded = error
            .downcast_ref::<CodedError>()
            .expect("metadata option errors should be typed");
        assert_eq!(coded.code(), ErrorCode::Env);
    }

    #[test]
    fn metadata_option_validation_rejects_missing_supported_options() {
        let options = BTreeMap::from([
            (String::from("docs"), false),
            (String::from("ci"), true),
            (String::from("forge-sync"), true),
        ]);

        let error = validate_managed_options_from_metadata(BlueprintName::AnyProject, options)
            .expect_err("missing supported options should fail");
        assert!(
            error
                .to_string()
                .contains("missing tool.forge.options.prettier")
        );
    }

    #[test]
    fn metadata_blueprint_detection_reports_env_error_code() {
        let error = detect_blueprint_metadata_from_pyproject("[tool.forge]\n")
            .expect_err("missing blueprint should fail");

        let coded = error
            .downcast_ref::<CodedError>()
            .expect("metadata detection errors should be typed");
        assert_eq!(coded.code(), ErrorCode::Env);
    }
}
