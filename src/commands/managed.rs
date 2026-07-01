use std::path::Path;

use serde::ser::{Serialize, SerializeStruct};

use crate::blueprint::components::ManagedComponent;
use crate::blueprint::files::GeneratedFiles;
use crate::blueprint::{BlueprintName, ManagedOption};

pub(crate) fn managed_infrastructure_summary(files: &GeneratedFiles) -> String {
    let mut parts = Vec::new();

    if files.contains_key(Path::new("pyproject.toml")) {
        parts.push("pyproject.toml".to_string());
    }
    if files.contains_key(Path::new("justfile")) {
        parts.push("justfile".to_string());
    }
    if files.contains_key(Path::new(".pre-commit-config.yaml")) {
        parts.push("prek hooks".to_string());
    }
    if files.contains_key(Path::new("AGENTS.md")) {
        parts.push("AGENTS.md".to_string());
    }
    if files.contains_key(Path::new("CLAUDE.md")) {
        parts.push("CLAUDE.md link".to_string());
    }
    if files.contains_key(Path::new("docs/package.json")) {
        parts.push("docs".to_string());
    }

    let workflow_count = files
        .keys()
        .filter(|path| path.starts_with(Path::new(".github/workflows")))
        .count();
    if workflow_count > 0 {
        parts.push(format!("github actions ({workflow_count})"));
    }

    if parts.is_empty() {
        "managed files".to_string()
    } else {
        parts.join(", ")
    }
}

pub(crate) fn required_tools_summary_for_options(
    blueprint: BlueprintName,
    options: &[SelectedOption],
) -> String {
    let mut tools = blueprint.definition().required_tools.to_vec();

    for component in ManagedComponent::ALL {
        if !blueprint.supports_option(component.option()) {
            continue;
        }
        if !selected_option_enabled(options, component.option_name()) {
            continue;
        }

        for required_tool in component.required_tools() {
            if !tools.contains(required_tool) {
                tools.push(required_tool);
            }
        }
    }

    tools.join(", ")
}

fn selected_option_enabled(options: &[SelectedOption], option_name: &str) -> bool {
    options
        .iter()
        .find(|option| option.name() == option_name)
        .is_some_and(|option| option.enabled)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SelectedOption {
    pub(crate) option: ManagedOption,
    pub(crate) enabled: bool,
}

impl SelectedOption {
    pub(crate) fn new(option: ManagedOption, enabled: bool) -> Self {
        Self { option, enabled }
    }

    pub(crate) fn name(&self) -> &'static str {
        self.option.as_str()
    }
}

impl Serialize for SelectedOption {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("SelectedOption", 2)?;
        state.serialize_field("name", self.name())?;
        state.serialize_field("enabled", &self.enabled)?;
        state.end()
    }
}

pub(crate) fn format_selected_options(options: &[SelectedOption]) -> String {
    let enabled = options
        .iter()
        .filter(|option| option.enabled)
        .map(SelectedOption::name)
        .collect::<Vec<_>>();
    let disabled = options
        .iter()
        .filter(|option| !option.enabled)
        .map(SelectedOption::name)
        .collect::<Vec<_>>();

    let enabled_summary = if enabled.is_empty() {
        "none".to_string()
    } else {
        enabled.join(", ")
    };
    let disabled_summary = if disabled.is_empty() {
        "none".to_string()
    } else {
        disabled.join(", ")
    };

    format!("enabled: {enabled_summary}; disabled: {disabled_summary}")
}
