use std::sync::OnceLock;

use minijinja::{Environment, UndefinedBehavior};
use serde::Serialize;

static TEMPLATE_ENV: OnceLock<Environment<'static>> = OnceLock::new();

pub fn render_template(context_name: &str, context: impl Serialize) -> String {
    let mut rendered = TEMPLATE_ENV
        .get_or_init(build_environment)
        .get_template(context_name)
        .unwrap_or_else(|error| panic!("missing template {context_name}: {error}"))
        .render(context)
        .unwrap_or_else(|error| panic!("failed to render template {context_name}: {error}"));
    rendered.truncate(rendered.trim_end_matches('\n').len());
    if !rendered.is_empty() {
        rendered.push('\n');
    }
    rendered
}

macro_rules! templates {
    ($env:expr, $($name:expr => $path:expr),* $(,)?) => {
        $(
            $env.add_template($name, include_str!($path))
                .expect("template should parse");
        )*
    };
}

fn build_environment() -> Environment<'static> {
    let mut environment = Environment::new();
    environment.set_undefined_behavior(UndefinedBehavior::Strict);

    templates!(environment,
        "shared/_pre_commit_header.yaml.j2" => "templates/shared/_pre_commit_header.yaml.j2",
        "shared/agents.md.j2" => "templates/shared/agents.md.j2",
        "shared/prettierrc.json.j2" => "templates/shared/prettierrc.json.j2",
        "shared/prettierignore.j2" => "templates/shared/prettierignore.j2",
        "shared/editorconfig.j2" => "templates/shared/editorconfig.j2",
        "shared/gitattributes.j2" => "templates/shared/gitattributes.j2",
        "shared/markdownlint.jsonc.j2" => "templates/shared/markdownlint.jsonc.j2",
        "shared/forge-sync.yaml.j2" => "templates/shared/forge-sync.yaml.j2",
        "shared/py.typed.j2" => "templates/shared/py.typed.j2",
        "shared/install-forge-step.yaml.j2" => "templates/shared/install-forge-step.yaml.j2",
        "shared/setup-uv-step.yaml.j2" => "templates/shared/setup-uv-step.yaml.j2",
        "shared/read-only-checkout-step.yaml.j2" => "templates/shared/read-only-checkout-step.yaml.j2",
        "shared/read-only-permissions.yaml.j2" => "templates/shared/read-only-permissions.yaml.j2",
        "shared/cancel-redundant-ci-concurrency.yaml.j2" => "templates/shared/cancel-redundant-ci-concurrency.yaml.j2",
        "shared/serialized-sync-concurrency.yaml.j2" => "templates/shared/serialized-sync-concurrency.yaml.j2",
        "shared/serialized-ref-concurrency.yaml.j2" => "templates/shared/serialized-ref-concurrency.yaml.j2",
        "shared/serialized-release-concurrency.yaml.j2" => "templates/shared/serialized-release-concurrency.yaml.j2",
        "shared/job-timeout.yaml.j2" => "templates/shared/job-timeout.yaml.j2",
        "shared/uv-sync-locked-step.yaml.j2" => "templates/shared/uv-sync-locked-step.yaml.j2",
        "shared/uv-lock-check-step.yaml.j2" => "templates/shared/uv-lock-check-step.yaml.j2",
        "shared/uv-run-locked-step.yaml.j2" => "templates/shared/uv-run-locked-step.yaml.j2",
        "shared/prettier-pre-commit-hook.yaml.j2" => "templates/shared/prettier-pre-commit-hook.yaml.j2",
        "shared/markdownlint-pre-commit-hook.yaml.j2" => "templates/shared/markdownlint-pre-commit-hook.yaml.j2",
        "any_project/readme.md.j2" => "templates/any_project/readme.md.j2",
        "any_project/gitignore.j2" => "templates/any_project/gitignore.j2",
        "any_project/pyproject.toml.j2" => "templates/any_project/pyproject.toml.j2",
        "any_project/justfile.j2" => "templates/any_project/justfile.j2",
        "any_project/pre-commit-config.yaml.j2" => "templates/any_project/pre-commit-config.yaml.j2",
        "any_project/ci.yaml.j2" => "templates/any_project/ci.yaml.j2",
        "any_project/docs-package.json.j2" => "templates/any_project/docs-package.json.j2",
        "any_project/docs-astro.config.mjs.j2" => "templates/any_project/docs-astro.config.mjs.j2",
        "any_project/docs-tsconfig.json.j2" => "templates/any_project/docs-tsconfig.json.j2",
        "any_project/docs-index.mdx.j2" => "templates/any_project/docs-index.mdx.j2",
        "python_library/readme.md.j2" => "templates/python_library/readme.md.j2",
        "python_library/mit-license.j2" => "templates/python_library/mit-license.j2",
        "python_library/apache-license.j2" => "templates/python_library/apache-license.j2",
        "python_library/bsd-license.j2" => "templates/python_library/bsd-license.j2",
        "python_library/bsd-2-clause-license.j2" => "templates/python_library/bsd-2-clause-license.j2",
        "python_library/isc-license.j2" => "templates/python_library/isc-license.j2",
        "python_library/gitignore.j2" => "templates/python_library/gitignore.j2",
        "python_library/pyproject.toml.j2" => "templates/python_library/pyproject.toml.j2",
        "python_library/justfile.j2" => "templates/python_library/justfile.j2",
        "python_library/pre-commit-config.yaml.j2" => "templates/python_library/pre-commit-config.yaml.j2",
        "python_library/pretty-format-json-hook.yaml.j2" => "templates/python_library/pretty-format-json-hook.yaml.j2",
        "python_library/typos.toml.j2" => "templates/python_library/typos.toml.j2",
        "python_library/contributing.md.j2" => "templates/python_library/contributing.md.j2",
        "python_library/changelog.md.j2" => "templates/python_library/changelog.md.j2",
        "python_library/ci.yaml.j2" => "templates/python_library/ci.yaml.j2",
        "python_library/codecov-step.yaml.j2" => "templates/python_library/codecov-step.yaml.j2",
        "python_library/release-please.yaml.j2" => "templates/python_library/release-please.yaml.j2",
        "python_library/pypi-publish-job.yaml.j2" => "templates/python_library/pypi-publish-job.yaml.j2",
        "python_library/workflow-quality.yaml.j2" => "templates/python_library/workflow-quality.yaml.j2",
        "python_library/docs-pages.yaml.j2" => "templates/python_library/docs-pages.yaml.j2",
        "python_library/release-please-config.json.j2" => "templates/python_library/release-please-config.json.j2",
        "python_library/release-please-manifest.json.j2" => "templates/python_library/release-please-manifest.json.j2",
        "python_library/docs-package.json.j2" => "templates/python_library/docs-package.json.j2",
        "python_library/docs-astro.config.mjs.j2" => "templates/python_library/docs-astro.config.mjs.j2",
        "python_library/docs-content.config.ts.j2" => "templates/python_library/docs-content.config.ts.j2",
        "python_library/docs-tsconfig.json.j2" => "templates/python_library/docs-tsconfig.json.j2",
        "python_library/docs-index.mdx.j2" => "templates/python_library/docs-index.mdx.j2",
        "python_library/__init__.py.j2" => "templates/python_library/__init__.py.j2",
        "python_library/core.py.j2" => "templates/python_library/core.py.j2",
        "python_library/test.py.j2" => "templates/python_library/test.py.j2",
        "rust_library/readme.md.j2" => "templates/rust_library/readme.md.j2",
        "rust_library/mit-license.j2" => "templates/rust_library/mit-license.j2",
        "rust_library/apache-license.j2" => "templates/rust_library/apache-license.j2",
        "rust_library/bsd-license.j2" => "templates/rust_library/bsd-license.j2",
        "rust_library/bsd-2-clause-license.j2" => "templates/rust_library/bsd-2-clause-license.j2",
        "rust_library/isc-license.j2" => "templates/rust_library/isc-license.j2",
        "rust_library/gitignore.j2" => "templates/rust_library/gitignore.j2",
        "rust_library/cargo.toml.j2" => "templates/rust_library/cargo.toml.j2",
        "rust_library/pyproject.toml.j2" => "templates/rust_library/pyproject.toml.j2",
        "rust_library/justfile.j2" => "templates/rust_library/justfile.j2",
        "rust_library/pre-commit-config.yaml.j2" => "templates/rust_library/pre-commit-config.yaml.j2",
        "rust_library/ci.yaml.j2" => "templates/rust_library/ci.yaml.j2",
        "rust_library/lib.rs.j2" => "templates/rust_library/lib.rs.j2",
        "rust_library/docs-package.json.j2" => "templates/rust_library/docs-package.json.j2",
        "rust_library/docs-astro.config.mjs.j2" => "templates/rust_library/docs-astro.config.mjs.j2",
        "rust_library/docs-tsconfig.json.j2" => "templates/rust_library/docs-tsconfig.json.j2",
        "rust_library/docs-index.mdx.j2" => "templates/rust_library/docs-index.mdx.j2",
    );

    environment
}

#[cfg(test)]
mod tests {
    use crate::blueprint::template_engine::render_template;

    #[test]
    fn rendered_templates_keep_trailing_newline() {
        let rendered = render_template(
            "shared/forge-sync.yaml.j2",
            serde_json::json!({
                "serialized_sync_concurrency": "",
                "job_timeout": "",
                "read_only_checkout_step": "",
                "setup_uv_step": "",
                "install_forge_step": "",
            }),
        );

        assert!(rendered.ends_with('\n'));
    }

    #[test]
    fn empty_templates_remain_empty() {
        assert_eq!(render_template("shared/py.typed.j2", ()), "");
    }
}
