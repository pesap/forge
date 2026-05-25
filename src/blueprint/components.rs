use std::collections::BTreeSet;
use std::path::PathBuf;

use anyhow::Result;

use crate::blueprint::files::{GeneratedFile, GeneratedFiles};
use crate::blueprint::{ManagedOption, ManagedOptionValues, managed_option_enabled};

const PRETTIER_CLEANUP_PATHS: &[&str] = &[".prettierrc.json", ".prettierignore"];
const PRETTIER_FORMAT_COMMAND: &str = "npx --yes prettier@3.8.3 --write --ignore-unknown .";
const PRETTIER_CHECK_COMMAND: &str = "npx --yes prettier@3.8.3 --check --ignore-unknown .";
const PRETTIER_PRE_COMMIT_HOOK: &str = "      - id: prettier\n        name: prettier check\n        entry: npx --yes prettier@3.8.3 --check --ignore-unknown\n        language: system\n        types_or: [json, yaml, markdown]\n";
const PRETTIER_REQUIRED_TOOLS: &[&str] = &["npx"];
const EDITORCONFIG_CLEANUP_PATHS: &[&str] = &[".editorconfig"];
const EDITORCONFIG_REQUIRED_TOOLS: &[&str] = &[];
const MARKDOWNLINT_CLEANUP_PATHS: &[&str] = &[".markdownlint.jsonc"];
const MARKDOWNLINT_FORMAT_COMMAND: &str = "npx --yes markdownlint-cli2@0.18.1 --fix \"**/*.md\"";
const MARKDOWNLINT_CHECK_COMMAND: &str = "npx --yes markdownlint-cli2@0.18.1 \"**/*.md\"";
const MARKDOWNLINT_PRE_COMMIT_HOOK: &str = "      - id: markdownlint\n        name: markdownlint check\n        entry: npx --yes markdownlint-cli2@0.18.1\n        language: system\n        files: \\.(md|markdown)$\n";
const MARKDOWNLINT_REQUIRED_TOOLS: &[&str] = &["npx"];

#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ManagedComponent {
    Prettier,
    Editorconfig,
    Markdownlint,
}

impl ManagedComponent {
    pub const ALL: [Self; 3] = [Self::Prettier, Self::Editorconfig, Self::Markdownlint];

    pub fn definition(self) -> &'static ComponentDefinition {
        match self {
            Self::Prettier => &COMPONENT_REGISTRY[0],
            Self::Editorconfig => &COMPONENT_REGISTRY[1],
            Self::Markdownlint => &COMPONENT_REGISTRY[2],
        }
    }

    pub fn option_name(self) -> &'static str {
        self.definition().option_name
    }

    pub fn option(self) -> ManagedOption {
        self.definition().option
    }

    pub fn description(self) -> &'static str {
        self.definition().description
    }

    fn render_files(self) -> GeneratedFiles {
        (self.definition().render_files)()
    }

    pub fn cleanup_paths(self) -> &'static [&'static str] {
        self.definition().cleanup_paths
    }

    pub fn pre_commit_hook(self) -> Option<&'static str> {
        self.definition().pre_commit_hook
    }

    pub fn required_tools(self) -> &'static [&'static str] {
        self.definition().required_tools
    }

    pub fn format_command(self) -> Option<&'static str> {
        self.definition().format_command
    }

    pub fn check_command(self) -> Option<&'static str> {
        self.definition().check_command
    }
}

pub struct ComponentDefinition {
    pub id: ManagedComponent,
    pub option: ManagedOption,
    pub option_name: &'static str,
    pub description: &'static str,
    cleanup_paths: &'static [&'static str],
    pre_commit_hook: Option<&'static str>,
    format_command: Option<&'static str>,
    check_command: Option<&'static str>,
    required_tools: &'static [&'static str],
    render_files: fn() -> GeneratedFiles,
}

pub const COMPONENT_REGISTRY: [ComponentDefinition; 3] = [
    ComponentDefinition {
        id: ManagedComponent::Prettier,
        option: ManagedOption::Prettier,
        option_name: "prettier",
        description: "Prettier formatting for JSON, YAML, and Markdown",
        cleanup_paths: PRETTIER_CLEANUP_PATHS,
        pre_commit_hook: Some(PRETTIER_PRE_COMMIT_HOOK),
        format_command: Some(PRETTIER_FORMAT_COMMAND),
        check_command: Some(PRETTIER_CHECK_COMMAND),
        required_tools: PRETTIER_REQUIRED_TOOLS,
        render_files: render_prettier_files,
    },
    ComponentDefinition {
        id: ManagedComponent::Editorconfig,
        option: ManagedOption::Editorconfig,
        option_name: "editorconfig",
        description: "EditorConfig baseline for consistent cross-editor whitespace",
        cleanup_paths: EDITORCONFIG_CLEANUP_PATHS,
        pre_commit_hook: None,
        format_command: None,
        check_command: None,
        required_tools: EDITORCONFIG_REQUIRED_TOOLS,
        render_files: render_editorconfig_files,
    },
    ComponentDefinition {
        id: ManagedComponent::Markdownlint,
        option: ManagedOption::Markdownlint,
        option_name: "markdownlint",
        description: "Markdown linting with markdownlint-cli2",
        cleanup_paths: MARKDOWNLINT_CLEANUP_PATHS,
        pre_commit_hook: Some(MARKDOWNLINT_PRE_COMMIT_HOOK),
        format_command: Some(MARKDOWNLINT_FORMAT_COMMAND),
        check_command: Some(MARKDOWNLINT_CHECK_COMMAND),
        required_tools: MARKDOWNLINT_REQUIRED_TOOLS,
        render_files: render_markdownlint_files,
    },
];

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ComponentSelection {
    enabled: BTreeSet<ManagedComponent>,
}

impl ComponentSelection {
    pub fn from_prettier(prettier: bool) -> Self {
        Self::from_enabled([prettier.then_some(ManagedComponent::Prettier)])
    }

    pub fn from_flags(prettier: bool, editorconfig: bool, markdownlint: bool) -> Self {
        Self::from_enabled([
            prettier.then_some(ManagedComponent::Prettier),
            editorconfig.then_some(ManagedComponent::Editorconfig),
            markdownlint.then_some(ManagedComponent::Markdownlint),
        ])
    }

    pub fn from_options(options: &ManagedOptionValues) -> Result<Self> {
        Ok(Self::from_enabled([
            managed_option_enabled(options, ManagedOption::Prettier)?
                .then_some(ManagedComponent::Prettier),
            managed_option_enabled(options, ManagedOption::Editorconfig)?
                .then_some(ManagedComponent::Editorconfig),
            managed_option_enabled(options, ManagedOption::Markdownlint)?
                .then_some(ManagedComponent::Markdownlint),
        ]))
    }

    fn from_enabled<const N: usize>(components: [Option<ManagedComponent>; N]) -> Self {
        Self {
            enabled: components.into_iter().flatten().collect(),
        }
    }

    pub fn is_enabled(&self, component: ManagedComponent) -> bool {
        self.enabled.contains(&component)
    }

    pub fn set_enabled(&mut self, component: ManagedComponent, enabled: bool) {
        if enabled {
            self.enabled.insert(component);
        } else {
            self.enabled.remove(&component);
        }
    }

    pub fn render_files(&self) -> GeneratedFiles {
        let mut files = GeneratedFiles::new();

        for component in &self.enabled {
            files.extend(component.render_files());
        }

        files
    }

    pub fn disabled_file_paths(&self) -> Vec<PathBuf> {
        ManagedComponent::ALL
            .into_iter()
            .filter(|component| !self.is_enabled(*component))
            .flat_map(ManagedComponent::cleanup_paths)
            .map(PathBuf::from)
            .collect()
    }

    pub fn pre_commit_hooks(&self) -> String {
        self.enabled
            .iter()
            .filter_map(|component| component.pre_commit_hook())
            .collect()
    }

    pub fn format_commands(&self) -> Vec<&'static str> {
        self.enabled
            .iter()
            .filter_map(|component| component.format_command())
            .collect()
    }

    pub fn check_commands(&self) -> Vec<&'static str> {
        self.enabled
            .iter()
            .filter_map(|component| component.check_command())
            .collect()
    }
}

fn render_prettier_files() -> GeneratedFiles {
    let mut files = GeneratedFiles::new();
    files.insert(
        PathBuf::from(".prettierrc.json"),
        GeneratedFile::text(
            "{\n  \"printWidth\": 100,\n  \"proseWrap\": \"always\",\n  \"singleQuote\": false\n}\n",
        ),
    );
    files.insert(
        PathBuf::from(".prettierignore"),
        GeneratedFile::text("dist/\nbuild/\nsite/\n.venv/\n.coverage\nuv.lock\n"),
    );
    files
}

fn render_editorconfig_files() -> GeneratedFiles {
    let mut files = GeneratedFiles::new();
    files.insert(
        PathBuf::from(".editorconfig"),
        GeneratedFile::text(
            "root = true\n\n[*]\ncharset = utf-8\nend_of_line = lf\ninsert_final_newline = true\ntrim_trailing_whitespace = true\nindent_style = space\nindent_size = 4\n\n[*.{md,markdown}]\ntrim_trailing_whitespace = false\n",
        ),
    );
    files
}

fn render_markdownlint_files() -> GeneratedFiles {
    let mut files = GeneratedFiles::new();
    files.insert(
        PathBuf::from(".markdownlint.jsonc"),
        GeneratedFile::text(
            "{\n  \"default\": true,\n  \"MD013\": false,\n  \"MD033\": false,\n  \"MD041\": false\n}\n",
        ),
    );
    files
}

#[cfg(test)]
mod tests {
    use crate::blueprint::ManagedOption;
    use crate::blueprint::components::{COMPONENT_REGISTRY, ComponentSelection, ManagedComponent};
    use std::path::PathBuf;

    #[test]
    fn component_registry_has_one_definition_for_each_component() {
        let registry_ids = COMPONENT_REGISTRY
            .iter()
            .map(|component| component.id)
            .collect::<Vec<_>>();

        assert_eq!(registry_ids, ManagedComponent::ALL.to_vec());
    }

    #[test]
    fn component_registry_exposes_managed_option_metadata() {
        let prettier = ManagedComponent::Prettier;
        let editorconfig = ManagedComponent::Editorconfig;

        assert_eq!(prettier.option(), ManagedOption::Prettier);
        assert_eq!(prettier.option_name(), prettier.option().as_str());
        assert_eq!(prettier.description(), prettier.option().description());
        assert_eq!(prettier.required_tools(), ["npx"]);
        assert_eq!(
            prettier.format_command(),
            Some("npx --yes prettier@3.8.3 --write --ignore-unknown .")
        );
        assert_eq!(
            prettier.check_command(),
            Some("npx --yes prettier@3.8.3 --check --ignore-unknown .")
        );

        assert_eq!(editorconfig.option(), ManagedOption::Editorconfig);
        assert_eq!(editorconfig.option_name(), editorconfig.option().as_str());
        assert_eq!(
            editorconfig.description(),
            editorconfig.option().description()
        );
        assert!(editorconfig.required_tools().is_empty());
        assert_eq!(editorconfig.format_command(), None);
        assert_eq!(editorconfig.check_command(), None);
    }

    #[test]
    fn component_selection_renders_enabled_component_files() {
        let selection = ComponentSelection::from_prettier(true);
        let files = selection.render_files();

        assert!(selection.is_enabled(ManagedComponent::Prettier));
        assert!(files.contains_key(&PathBuf::from(".prettierrc.json")));
        assert!(files.contains_key(&PathBuf::from(".prettierignore")));
        assert_eq!(
            selection.disabled_file_paths(),
            vec![
                PathBuf::from(".editorconfig"),
                PathBuf::from(".markdownlint.jsonc"),
            ]
        );
    }

    #[test]
    fn component_selection_reports_disabled_component_cleanup_paths() {
        let selection = ComponentSelection::default();

        assert!(!selection.is_enabled(ManagedComponent::Prettier));
        assert_eq!(
            selection.disabled_file_paths(),
            vec![
                PathBuf::from(".prettierrc.json"),
                PathBuf::from(".prettierignore"),
                PathBuf::from(".editorconfig"),
                PathBuf::from(".markdownlint.jsonc"),
            ]
        );
    }

    #[test]
    fn component_selection_can_enable_editorconfig_component() {
        let selection = ComponentSelection::from_flags(false, true, false);
        let files = selection.render_files();

        assert!(selection.is_enabled(ManagedComponent::Editorconfig));
        assert!(files.contains_key(&PathBuf::from(".editorconfig")));
        assert_eq!(
            selection.disabled_file_paths(),
            vec![
                PathBuf::from(".prettierrc.json"),
                PathBuf::from(".prettierignore"),
                PathBuf::from(".markdownlint.jsonc"),
            ]
        );
    }

    #[test]
    fn component_selection_set_enabled_toggles_component_state() {
        let mut selection = ComponentSelection::default();

        selection.set_enabled(ManagedComponent::Editorconfig, true);
        assert!(selection.is_enabled(ManagedComponent::Editorconfig));

        selection.set_enabled(ManagedComponent::Editorconfig, false);
        assert!(!selection.is_enabled(ManagedComponent::Editorconfig));
    }

    #[test]
    fn component_selection_exposes_non_mutating_pre_commit_hooks_for_enabled_components() {
        let selection = ComponentSelection::from_prettier(true);

        assert!(selection.pre_commit_hooks().contains("id: prettier"));
        assert!(
            selection
                .pre_commit_hooks()
                .contains("entry: npx --yes prettier@3.8.3 --check --ignore-unknown")
        );
        assert!(!selection.pre_commit_hooks().contains("--write"));
    }

    #[test]
    fn component_selection_exposes_format_commands_for_enabled_components() {
        let selection = ComponentSelection::from_prettier(true);

        assert_eq!(
            selection.format_commands(),
            vec!["npx --yes prettier@3.8.3 --write --ignore-unknown ."]
        );
    }

    #[test]
    fn component_selection_exposes_check_commands_for_enabled_components() {
        let selection = ComponentSelection::from_prettier(true);

        assert_eq!(
            selection.check_commands(),
            vec!["npx --yes prettier@3.8.3 --check --ignore-unknown ."]
        );
    }
}
