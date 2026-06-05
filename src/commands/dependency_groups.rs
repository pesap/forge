use anyhow::{Context, Result};
use std::collections::{BTreeMap, BTreeSet};
use toml::Value;

use crate::blueprint::toml_value;
use crate::commands::pyproject_sections::table_range;

pub(crate) fn sync_dependency_groups(pyproject: &str, generated_pyproject: &str) -> Result<String> {
    let generated = dependency_groups(generated_pyproject, "generated pyproject.toml")?;
    if generated.is_empty() {
        return Ok(pyproject.to_string());
    }

    let mut groups = dependency_groups(pyproject, "pyproject.toml")?;
    migrate_linting_to_code_quality(&mut groups, &generated);
    let generated_specs = dependency_specs_by_name(&generated);
    for dependencies in groups.values_mut() {
        for dependency in dependencies.iter_mut() {
            let Some(name) = dependency_name(dependency) else {
                continue;
            };
            if let Some(generated_spec) = generated_specs.get(name) {
                *dependency = Value::String(generated_spec.clone());
            }
        }
    }

    for (name, generated_dependencies) in generated {
        if name == "dev" {
            merge_dev_dependency_group(&mut groups, generated_dependencies);
            continue;
        }

        let dependencies = groups.entry(name).or_default();
        for generated_dependency in generated_dependencies {
            merge_dependency(dependencies, generated_dependency);
        }
    }

    let rendered = render_dependency_groups(&groups);
    if let Some((start, end)) = table_range(pyproject, "dependency-groups") {
        let mut output = String::with_capacity(pyproject.len() + rendered.len());
        output.push_str(&pyproject[..start]);
        output.push_str(&rendered);
        output.push_str(&pyproject[end..]);
        return Ok(output);
    }

    let mut output = pyproject.to_string();
    if !output.ends_with('\n') {
        output.push('\n');
    }
    if !output.ends_with("\n\n") {
        output.push('\n');
    }
    output.push_str(&rendered);
    Ok(output)
}

fn dependency_groups(pyproject: &str, source: &str) -> Result<BTreeMap<String, Vec<Value>>> {
    let parsed: Value =
        toml::from_str(pyproject).with_context(|| format!("failed to parse {source}"))?;
    let Some(groups) = parsed.get("dependency-groups").and_then(Value::as_table) else {
        return Ok(BTreeMap::new());
    };

    let mut dependency_groups = BTreeMap::new();
    for (name, dependencies) in groups {
        let Some(dependencies) = dependencies.as_array() else {
            continue;
        };
        dependency_groups.insert(name.clone(), dependencies.clone());
    }
    Ok(dependency_groups)
}

fn migrate_linting_to_code_quality(
    groups: &mut BTreeMap<String, Vec<Value>>,
    generated: &BTreeMap<String, Vec<Value>>,
) {
    let Some(generated_code_quality) = generated.get("code-quality") else {
        return;
    };

    if let Some(dev_dependencies) = groups.get_mut("dev") {
        dev_dependencies.retain(|dependency| include_group(dependency) != Some("linting"));
    }

    let generated_names = generated_code_quality
        .iter()
        .filter_map(dependency_name)
        .collect::<BTreeSet<_>>();
    let linting_is_forge_owned = groups.get("linting").is_some_and(|dependencies| {
        dependencies
            .iter()
            .filter_map(dependency_name)
            .all(|name| generated_names.contains(name))
    });
    if linting_is_forge_owned {
        groups.remove("linting");
    }
}

fn dependency_specs_by_name(groups: &BTreeMap<String, Vec<Value>>) -> BTreeMap<String, String> {
    let mut specs = BTreeMap::new();
    for dependencies in groups.values() {
        for dependency in dependencies {
            let Some(spec) = dependency.as_str() else {
                continue;
            };
            specs.insert(package_name(spec).to_string(), spec.to_string());
        }
    }
    specs
}

fn merge_dev_dependency_group(
    groups: &mut BTreeMap<String, Vec<Value>>,
    generated_dependencies: Vec<Value>,
) {
    let existing_dependencies = groups.remove("dev").unwrap_or_default();
    let mut dependencies = generated_dependencies;
    for existing_dependency in existing_dependencies {
        merge_dependency(&mut dependencies, existing_dependency);
    }
    groups.insert("dev".to_string(), dependencies);
}

fn merge_dependency(dependencies: &mut Vec<Value>, generated_dependency: Value) {
    if let Some(generated_name) = dependency_name(&generated_dependency) {
        if let Some(existing) = dependencies
            .iter_mut()
            .find(|dependency| dependency_name(dependency) == Some(generated_name))
        {
            *existing = generated_dependency;
            return;
        }
    } else if include_group(&generated_dependency).is_some()
        && dependencies
            .iter()
            .any(|dependency| dependency == &generated_dependency)
    {
        return;
    }

    dependencies.push(generated_dependency);
}

fn dependency_name(dependency: &Value) -> Option<&str> {
    dependency.as_str().map(package_name)
}

fn package_name(dependency: &str) -> &str {
    dependency
        .split(|character: char| {
            !character.is_ascii_alphanumeric()
                && character != '-'
                && character != '_'
                && character != '.'
        })
        .next()
        .unwrap_or(dependency)
}

fn include_group(dependency: &Value) -> Option<&str> {
    dependency
        .as_table()
        .and_then(|table| table.get("include-group"))
        .and_then(Value::as_str)
}

fn render_dependency_groups(groups: &BTreeMap<String, Vec<Value>>) -> String {
    let mut output = String::from("[dependency-groups]\n");
    let mut rendered = BTreeSet::new();
    for name in ["dev", "code-quality", "build", "linting", "test"] {
        if let Some(dependencies) = groups.get(name) {
            render_dependency_group(&mut output, name, dependencies);
            rendered.insert(name);
        }
    }
    for (name, dependencies) in groups {
        if rendered.contains(name.as_str()) {
            continue;
        }
        render_dependency_group(&mut output, name, dependencies);
    }
    output.push('\n');
    output
}

fn render_dependency_group(output: &mut String, name: &str, dependencies: &[Value]) {
    output.push_str(name);
    if should_render_inline(dependencies) {
        output.push_str(" = [");
        for (index, dependency) in dependencies.iter().enumerate() {
            if index > 0 {
                output.push_str(", ");
            }
            output.push_str(&render_dependency(dependency));
        }
        output.push_str("]\n");
        return;
    }

    output.push_str(" = [\n");
    for dependency in dependencies {
        output.push_str("  ");
        output.push_str(&render_dependency(dependency));
        output.push_str(",\n");
    }
    output.push_str("]\n");
}

fn should_render_inline(dependencies: &[Value]) -> bool {
    dependencies
        .iter()
        .all(|dependency| include_group(dependency).is_some())
        || (dependencies.len() <= 3 && dependencies.iter().all(Value::is_str))
}

fn render_dependency(dependency: &Value) -> String {
    if let Some(dependency) = dependency.as_str() {
        return toml_value::string_literal(dependency);
    }
    if let Some(include_group) = include_group(dependency) {
        return format!(
            "{{ include-group = {} }}",
            toml_value::string_literal(include_group)
        );
    }
    dependency.to_string()
}

#[cfg(test)]
mod tests {
    use crate::commands::dependency_groups::sync_dependency_groups;

    #[test]
    fn sync_dependency_groups_migrates_forge_linting_group_to_code_quality() {
        let existing = r#"[project]
name = "demo"

[dependency-groups]
dev = [{ include-group = "linting" }, { include-group = "test" }]
linting = ["prek~=0.3.5", "ruff~=0.1.0", "ty~=0.0.1"]
test = ["pytest~=9.0.0"]
"#;
        let generated = r#"[dependency-groups]
dev = [{ include-group = "code-quality" }, { include-group = "test" }]
code-quality = ["prek~=0.4.1", "ruff~=0.14.0", "ty~=0.0.1"]
test = ["pytest~=9.0.0"]
"#;

        let synced = sync_dependency_groups(existing, generated).expect("dependency group sync");

        assert!(synced.contains(
            "dev = [{ include-group = \"code-quality\" }, { include-group = \"test\" }]"
        ));
        assert!(
            synced.contains("code-quality = [\"prek~=0.4.1\", \"ruff~=0.14.0\", \"ty~=0.0.1\"]")
        );
        assert!(!synced.contains("linting = ["));
        assert!(!synced.contains("include-group = \"linting\""));
    }

    #[test]
    fn sync_dependency_groups_preserves_user_linting_group_with_extra_dependencies() {
        let existing = r#"[project]
name = "demo"

[dependency-groups]
dev = [{ include-group = "linting" }]
linting = ["ruff~=0.1.0", "custom-linter~=1.0"]
"#;
        let generated = r#"[dependency-groups]
dev = [{ include-group = "code-quality" }, { include-group = "test" }]
code-quality = ["ruff~=0.14.0"]
test = ["pytest~=9.0.0"]
"#;

        let synced = sync_dependency_groups(existing, generated).expect("dependency group sync");

        assert!(synced.contains("code-quality = [\"ruff~=0.14.0\"]"));
        assert!(synced.contains("linting = [\"ruff~=0.14.0\", \"custom-linter~=1.0\"]"));
        assert!(!synced.contains("include-group = \"linting\""));
    }
}
