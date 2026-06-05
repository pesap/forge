use anyhow::{Context, Result};
use toml::Value;

const PYTHON_TOOL_TABLES: [&str; 5] = [
    "tool.pytest.ini_options",
    "tool.pytest_env",
    "tool.ruff",
    "tool.ruff.lint",
    "tool.ty.rules",
];

pub(crate) fn sync_pytest_sections(pyproject: &str, generated_pyproject: &str) -> Result<String> {
    toml::from_str::<Value>(generated_pyproject)
        .context("failed to parse generated pyproject.toml for Python tool sections")?;
    let rendered = rendered_table_group(generated_pyproject, &PYTHON_TOOL_TABLES);
    if rendered.is_empty() {
        return Ok(pyproject.to_string());
    }

    let mut ranges = PYTHON_TOOL_TABLES
        .into_iter()
        .filter_map(|table_name| table_range(pyproject, table_name))
        .collect::<Vec<_>>();
    let insert_at = ranges
        .iter()
        .map(|(start, _)| *start)
        .min()
        .or_else(|| table_range(pyproject, "tool.forge").map(|(start, _)| start))
        .unwrap_or(pyproject.len());

    ranges.sort_by_key(|(start, _)| *start);
    let mut output = pyproject.to_string();
    for (start, end) in ranges.into_iter().rev() {
        output.replace_range(start..end, "");
    }
    insert_table_group(&output, insert_at.min(output.len()), &rendered)
}

pub(crate) fn sync_build_system(pyproject: &str, generated_pyproject: &str) -> Result<String> {
    let rendered = build_system_table(generated_pyproject, "generated pyproject.toml")?;
    let Some(rendered) = rendered else {
        return Ok(pyproject.to_string());
    };

    if let Some((start, end)) = table_range(pyproject, "build-system") {
        let mut output = String::with_capacity(pyproject.len() + rendered.len());
        output.push_str(&pyproject[..start]);
        output.push_str(&rendered);
        output.push_str(&pyproject[end..]);
        return Ok(output);
    }

    let mut output = pyproject.to_string();
    ensure_trailing_blank_line(&mut output);
    output.push_str(&rendered);
    Ok(output)
}

fn rendered_table_group(generated_pyproject: &str, table_names: &[&str]) -> String {
    let mut output = String::new();
    for table_name in table_names {
        let Some((start, end)) = table_range(generated_pyproject, table_name) else {
            continue;
        };
        output.push_str(&generated_pyproject[start..end]);
        if !output.ends_with("\n\n") {
            output.push('\n');
        }
    }
    output
}

fn insert_table_group(pyproject: &str, insert_at: usize, rendered: &str) -> Result<String> {
    let mut output = String::with_capacity(pyproject.len() + rendered.len() + 2);
    output.push_str(&pyproject[..insert_at]);
    ensure_trailing_blank_line(&mut output);
    output.push_str(rendered);
    if !rendered.ends_with("\n\n") {
        output.push('\n');
    }
    output.push_str(&pyproject[insert_at..]);
    toml::from_str::<Value>(&output)
        .context("failed to parse pyproject.toml after syncing table group")?;
    Ok(output)
}

fn ensure_trailing_blank_line(output: &mut String) {
    if !output.ends_with('\n') {
        output.push('\n');
    }
    if !output.ends_with("\n\n") {
        output.push('\n');
    }
}

fn build_system_table(pyproject: &str, source: &str) -> Result<Option<String>> {
    let parsed: Value =
        toml::from_str(pyproject).with_context(|| format!("failed to parse {source}"))?;
    let Some(build_system) = parsed.get("build-system").and_then(Value::as_table) else {
        return Ok(None);
    };

    let mut output = String::from("[build-system]\n");
    if let Some(requires) = build_system.get("requires").and_then(Value::as_array) {
        output.push_str("requires = [");
        for (index, requirement) in requires.iter().enumerate() {
            if index > 0 {
                output.push_str(", ");
            }
            output.push_str(&requirement.to_string());
        }
        output.push_str("]\n");
    }
    if let Some(backend) = build_system.get("build-backend").and_then(Value::as_str) {
        output.push_str("build-backend = ");
        output.push_str(&crate::blueprint::toml_value::string_literal(backend));
        output.push('\n');
    }
    output.push('\n');
    Ok(Some(output))
}

pub(crate) fn table_range(content: &str, table_name: &str) -> Option<(usize, usize)> {
    let header = format!("[{table_name}]");
    let start = content.find(&header)?;
    let after_header = start + header.len();
    let relative_end = content[after_header..]
        .find("\n[")
        .map(|index| after_header + index + 1)
        .unwrap_or(content.len());
    Some((start, relative_end))
}

#[cfg(test)]
mod tests {
    use crate::commands::pyproject_sections::sync_pytest_sections;

    const GENERATED: &str = r#"[tool.pytest.ini_options]
cache_dir = "/Users/example/Library/Caches/pytest/test"

[tool.ruff]
line-length = 110

[tool.ruff.lint]
select = ["E", "F", "I", "UP", "B", "SIM", "RUF", "ARG", "C4", "PIE", "PTH", "RET", "TID", "TC", "PERF"]
ignore = ["E501"]
fixable = ["ALL"]

[tool.ty.rules]
all = "error"

[tool.forge]
blueprint = "python-library>=0.1.0"
"#;

    #[test]
    fn sync_pytest_sections_replaces_pytest_ini_options_and_removes_pytest_env() {
        let existing = r#"[project]
name = "test"

[tool.pytest_env]
XDG_CACHE_HOME = "/tmp/cache"

[tool.pytest.ini_options]
cache_dir = "/Users/example/Library/Caches/pytest/test"

[tool.ruff]
line-length = 100

[tool.ty.rules]
all = "warn"

[tool.ruff.lint]
select = ["E", "F"]

[tool.forge]
blueprint = "python-library>=0.1.0"
"#;

        let synced = sync_pytest_sections(existing, GENERATED).expect("pytest sections sync");
        let ini_index = synced
            .find("[tool.pytest.ini_options]")
            .expect("pytest ini section should exist");
        let ruff_lint_index = synced
            .find("[tool.ruff.lint]")
            .expect("ruff lint section should exist");
        let ty_rules_index = synced
            .find("[tool.ty.rules]")
            .expect("ty rules section should exist");

        assert!(ini_index < ruff_lint_index);
        assert!(ruff_lint_index < ty_rules_index);
        assert!(synced.contains("cache_dir = \"/Users/example/Library/Caches/pytest/test\""));
        assert!(synced.contains("line-length = 110"));
        assert!(synced.contains("all = \"error\""));
        assert!(!synced.contains("[tool.pytest_env]"));
        assert!(!synced.contains("line-length = 100"));
        assert!(!synced.contains("all = \"warn\""));
    }
}
