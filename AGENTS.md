# Forge Agent Guide

Operational contract for humans and agents working in this repository. This is a
practical engineering guide for daily development, review, and handoff.

## Mission and System Context

- Forge is a Rust CLI tool for scaffolding and updating repository infrastructure
  from blueprints.
- Current blueprint support: `any-project`, `python-library`, `rust-library`.
- Uses `clap` for CLI parsing, `anyhow` for errors, `dialoguer` for interactive
  prompts, `serde`/`serde_json` for machine-readable output, and `toml` for
  generated metadata.
- Optimizes for correctness, clarity, and maintainable code generation across
  interactive and automation-first workflows.

## Core Engineering Principles

- Think before changing code. Understand architecture, data flow, and CLI behavior first.
- Fix root causes, not symptoms. Avoid layering bandaids.
- Keep modules focused and remove dead code aggressively.
- Prefer explicit types and validated schemas over loose maps.
- Make performance-aware choices for data structures and allocation behavior.
- Leave the codebase simpler than you found it.

## Collaboration and Git Safety

- Assume concurrent work by users and other agents.
- Treat `git status`, `git diff`, and `git show` as read-only context.
- Never revert, overwrite, or discard changes you did not author.
- Do not run destructive git operations (`reset --hard`, checkout file rollback,
  force push) unless explicitly requested.
- If a command runs longer than 5 minutes, stop it, capture context, and report
  what blocked progress.

## Repository Layout

- `src/main.rs`: Entry point only.
- `src/lib.rs`: Module declarations and command dispatch.
- `src/cli.rs`: All clap definitions (commands, args, enums).
- `src/ui.rs`: Human-readable terminal presentation helpers.
- `src/commands/`: Command implementations.
  - `blueprints.rs`: Blueprint listing and JSON output.
  - `components.rs`: Optional component listing and JSON output.
  - `completions.rs`: Shell completion generation.
  - `diff.rs`: Shared human-readable managed file diff rendering.
  - `init.rs`: Existing-repository adoption logic.
  - `new.rs`: Project generation logic.
  - `sync.rs`: Managed infrastructure sync/check/dry-run logic.
  - `self_update.rs`: CLI self-management.
  - `doctor.rs`: Environment diagnostics.
- `src/blueprint/`: Blueprint registry, implementations, and file planning.
  - `mod.rs`: Blueprint and managed-option registry.
  - `files.rs`: Managed file abstraction, symlinks, planning, and writes.
  - `toml_value.rs`: TOML literal rendering helpers for generated metadata.
  - `components.rs`: Optional reusable components such as Prettier.
  - `any_project.rs`: The `any-project` blueprint.
  - `python_library.rs`: The `python-library` blueprint.
  - `rust_library.rs`: The `rust-library` blueprint.
- `tests/`: Integration tests (CLI → filesystem workflows).

## Required Development Workflow

Use red-green-refactor for non-trivial changes:

1. Red: add or update a failing test that proves the behavior gap.
2. Green: implement the smallest correct change to pass.
3. Refactor: improve code quality while preserving behavior.

Additional rules:

- No breadcrumbs in old locations when moving/removing code.
- Keep APIs intentional, avoid speculative abstractions.
- Validate new patterns against official docs before adoption.
- For bug fixes, add regression tests in the same change.

## Rust Standards

- Use `anyhow` for error handling — consistent `Result<T>` with `Context` and `bail!`.
- Use `?` operator liberally — embrace Rust's error ergonomics.
- Use derive macros — `#[derive(Debug, Parser, Subcommand, ValueEnum)]` for structs/enums.
- Use `std::collections::BTreeMap` for deterministic file ordering.
- Keep function signatures self-descriptive: primary subject positional,
  configuration keyword-only.
- Use `clap`'s derive macros for CLI parsing.
- Avoid `unwrap()` in production code; use `?` or `bail!`.

## Blueprint System Expectations

- Keep blueprints in `src/blueprint/{name}.rs`.
- Define a `BLUEPRINT_NAME` constant.
- Register each blueprint in `BLUEPRINT_REGISTRY` so setup fields, CLI
  discovery, metadata detection, sync dispatch, and managed cleanup share one
  integration point.
- Implement `render_project_files()` and `render_managed_files()` using
  `GeneratedFiles`.
- Use `GeneratedFile` for both text files and managed symlinks.
- Register optional cleanup paths so disabled managed options remove generated
  files without touching user-owned files.
- Register reusable optional frameworks in `components.rs`; keep their managed
  files, cleanup paths, and hook snippets together.
- Embed `[tool.forge]` metadata in generated `pyproject.toml`.
- Use shared TOML literal helpers for generated TOML values; never hand-escape
  user-provided metadata with string replacement.
- `config_from_pyproject()` must return validated blueprint configs so update
  and cleanup paths cannot use corrupt metadata.
- Store managed feature deviations under `[tool.forge.overrides]`; omitted values use blueprint defaults.
- Keep sync behavior driven by generated metadata in the target repository; do
  not introduce a separate Forge status file.
- When adding blueprints, update tests and docs in the same change.

## Testing and Quality Gates

- Prefer unit tests for validation logic, file generation, and pure functions.
- Prefer integration tests for CLI workflows using `assert_cmd` and `tempfile`,
  including create, sync, check, dry-run, and JSON output paths.
- Unit tests live in the same file under `#[cfg(test)] mod tests`.
- Integration tests live in `tests/` directory.
- Avoid mock-heavy tests when real behavior can be verified directly.
- Unless asked otherwise, run tests you touched first, then expand when risk is
  high.
- Expected local quality commands:
  1. `cargo fmt --all`
  2. `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
  3. `cargo test`

## Documentation Contract

- Documentation and code ship together. If behavior changes, docs must change.
- Update whichever docs are affected, for example:
  - `README.md` for onboarding and usage changes.
  - This `AGENTS.md` for workflow or standards updates.
- If you add a new blueprint or major subsystem, document its purpose and boundary
  in both repo-level and blueprint-level docs.

## Dependency and Tooling Policy

- Add dependencies only when necessary.
- Prefer mature, maintained libraries with broad ecosystem usage.
- Validate API stability, maintenance health, and long-term fit before adding.
- Key current dependencies: `clap`, `anyhow`, `dialoguer`, `serde`,
  `serde_json`, `toml`.

## Commit and Review Expectations

- Use Conventional Commits (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`,
  `chore:`).
- Keep commits scoped and coherent; separate refactors from behavior changes when
  practical.
- Respect commit hooks if configured.

## Definition of Done (Before Handoff)

Before marking work complete, provide:

1. What changed and why, with file references.
2. Commands/tests run and whether they passed.
3. Documentation updates made.
4. Follow-ups, risks, and open questions.

If any command could not be run, state exactly what was skipped and how to verify
it.
