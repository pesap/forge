use std::sync::OnceLock;

use minijinja::{Environment, UndefinedBehavior};
use serde::Serialize;

static TEMPLATE_ENV: OnceLock<Environment<'static>> = OnceLock::new();

pub fn render_template(context_name: &str, context: impl Serialize) -> String {
    TEMPLATE_ENV
        .get_or_init(build_environment)
        .get_template(context_name)
        .unwrap_or_else(|error| panic!("missing template {context_name}: {error}"))
        .render(context)
        .unwrap_or_else(|error| panic!("failed to render template {context_name}: {error}"))
}

fn build_environment() -> Environment<'static> {
    let mut environment = Environment::new();
    environment.set_undefined_behavior(UndefinedBehavior::Strict);

    environment
        .add_template(
            "any_project/readme.md.j2",
            include_str!("templates/any_project/readme.md.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "any_project/gitignore.j2",
            include_str!("templates/any_project/gitignore.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "any_project/pyproject.toml.j2",
            include_str!("templates/any_project/pyproject.toml.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "any_project/justfile.j2",
            include_str!("templates/any_project/justfile.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "any_project/pre-commit-config.yaml.j2",
            include_str!("templates/any_project/pre-commit-config.yaml.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "any_project/ci.yaml.j2",
            include_str!("templates/any_project/ci.yaml.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "any_project/docs-package.json.j2",
            include_str!("templates/any_project/docs-package.json.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "any_project/docs-astro.config.mjs.j2",
            include_str!("templates/any_project/docs-astro.config.mjs.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "any_project/docs-tsconfig.json.j2",
            include_str!("templates/any_project/docs-tsconfig.json.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "any_project/docs-index.mdx.j2",
            include_str!("templates/any_project/docs-index.mdx.j2"),
        )
        .expect("template should parse");

    environment
        .add_template(
            "python_library/readme.md.j2",
            include_str!("templates/python_library/readme.md.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "python_library/mit-license.j2",
            include_str!("templates/python_library/mit-license.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "python_library/apache-license.j2",
            include_str!("templates/python_library/apache-license.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "python_library/bsd-license.j2",
            include_str!("templates/python_library/bsd-license.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "python_library/gitignore.j2",
            include_str!("templates/python_library/gitignore.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "python_library/pyproject.toml.j2",
            include_str!("templates/python_library/pyproject.toml.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "python_library/justfile.j2",
            include_str!("templates/python_library/justfile.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "python_library/pre-commit-config.yaml.j2",
            include_str!("templates/python_library/pre-commit-config.yaml.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "python_library/changelog.md.j2",
            include_str!("templates/python_library/changelog.md.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "python_library/ci.yaml.j2",
            include_str!("templates/python_library/ci.yaml.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "python_library/release-please.yaml.j2",
            include_str!("templates/python_library/release-please.yaml.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "python_library/release-please-config.json.j2",
            include_str!("templates/python_library/release-please-config.json.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "python_library/release-please-manifest.json.j2",
            include_str!("templates/python_library/release-please-manifest.json.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "python_library/publish-pypi.yaml.j2",
            include_str!("templates/python_library/publish-pypi.yaml.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "python_library/docs-package.json.j2",
            include_str!("templates/python_library/docs-package.json.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "python_library/docs-astro.config.mjs.j2",
            include_str!("templates/python_library/docs-astro.config.mjs.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "python_library/docs-tsconfig.json.j2",
            include_str!("templates/python_library/docs-tsconfig.json.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "python_library/docs-index.mdx.j2",
            include_str!("templates/python_library/docs-index.mdx.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "python_library/__init__.py.j2",
            include_str!("templates/python_library/__init__.py.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "python_library/core.py.j2",
            include_str!("templates/python_library/core.py.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "python_library/test.py.j2",
            include_str!("templates/python_library/test.py.j2"),
        )
        .expect("template should parse");

    environment
        .add_template(
            "rust_library/readme.md.j2",
            include_str!("templates/rust_library/readme.md.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "rust_library/mit-license.j2",
            include_str!("templates/rust_library/mit-license.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "rust_library/apache-license.j2",
            include_str!("templates/rust_library/apache-license.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "rust_library/bsd-license.j2",
            include_str!("templates/rust_library/bsd-license.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "rust_library/gitignore.j2",
            include_str!("templates/rust_library/gitignore.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "rust_library/cargo.toml.j2",
            include_str!("templates/rust_library/cargo.toml.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "rust_library/pyproject.toml.j2",
            include_str!("templates/rust_library/pyproject.toml.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "rust_library/justfile.j2",
            include_str!("templates/rust_library/justfile.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "rust_library/pre-commit-config.yaml.j2",
            include_str!("templates/rust_library/pre-commit-config.yaml.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "rust_library/ci.yaml.j2",
            include_str!("templates/rust_library/ci.yaml.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "rust_library/lib.rs.j2",
            include_str!("templates/rust_library/lib.rs.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "rust_library/docs-package.json.j2",
            include_str!("templates/rust_library/docs-package.json.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "rust_library/docs-astro.config.mjs.j2",
            include_str!("templates/rust_library/docs-astro.config.mjs.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "rust_library/docs-tsconfig.json.j2",
            include_str!("templates/rust_library/docs-tsconfig.json.j2"),
        )
        .expect("template should parse");
    environment
        .add_template(
            "rust_library/docs-index.mdx.j2",
            include_str!("templates/rust_library/docs-index.mdx.j2"),
        )
        .expect("template should parse");

    environment
}
