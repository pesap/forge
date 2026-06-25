# Forge

[![CI](https://github.com/pesap/forge/actions/workflows/ci.yaml/badge.svg)](https://github.com/pesap/forge/actions/workflows/ci.yaml)
[![Release](https://img.shields.io/github/v/release/pesap/forge)](https://github.com/pesap/forge/releases)
[![License](https://img.shields.io/badge/license-BSD--3--Clause-blue.svg)](LICENSE)

Forge is a Rust CLI for creating repositories from blueprints and keeping their
infrastructure current. It manages files such as CI workflows, hook
configuration, docs scaffolding, release automation, and task recipes while
preserving project-owned code and configuration.

Current blueprints: `any-project`, `python-library`, `rust-library`.

<p align="center">
  <a href="#quickstart">Quickstart</a> ·
  <a href="#install">Install</a> ·
  <a href="#commands">Commands</a> ·
  <a href="#concepts">Concepts</a> ·
  <a href="#workflows">Workflows</a> ·
  <a href="#development">Development</a>
</p>

---

## Quickstart

Create a Python library project:

```bash
forge init \
  --blueprint python-library \
  --path ./demo-lib \
  --project-name demo-lib \
  --package-name demo_lib \
  --description "Demo package" \
  --license BSD-3-Clause \
  --python-min 3.11 \
  --no-git-history \
  --yes
```

Then verify the generated project:

```bash
uv sync --all-groups
just verify
```

Generated projects include CI, release-please configuration, agent
instructions, `prek` hooks, docs scaffolding, and a scheduled Forge sync
workflow that opens an infrastructure pull request when managed files drift.

---

## Install

```bash
cargo install forge-cli
```

Or build from source:

```bash
git clone https://github.com/pesap/forge
cd forge
cargo build --release
# target/release/forge is now ready
```

If you installed Forge with the standalone shell or PowerShell installer from a
GitHub release, update it with:

```bash
forge self update
```

`forge self upgrade` is an alias. Package-manager installs should be updated
with that package manager instead.

---

## Commands

| Command             | What it does                                                        |
| ------------------- | ------------------------------------------------------------------- |
| `forge init`        | Create a new project or adopt infrastructure in an existing repo    |
| `forge sync`        | Refresh managed files in a Forge-managed project                    |
| `forge blueprints`  | List available blueprints and setup fields                          |
| `forge components`  | List optional managed components                                    |
| `forge doctor`      | Check local toolchain health                                        |
| `forge completions` | Emit shell completion scripts                                       |
| `forge self update` | Update a standalone-installer Forge binary                          |

Running `forge` with no subcommand prints top-level help and quickstart examples
for first-run discovery.

---

## Concepts

### Blueprints

A blueprint defines what files a project generates. Each blueprint registers its
setup fields, managed files, required tools, and optional components in a single
integration point.

```bash
# List all blueprints with their fields and defaults
forge blueprints --json
```

### Managed infrastructure

Forge writes two kinds of files:

- **Project files** -- starter source code, package metadata, agent instructions.
  Generated once by `forge init`.
- **Managed files** -- CI workflows, hook configs, documentation config, editor
  settings, line-ending policy, and task-runner config. Regenerated on every
  `forge sync` run, so they stay current with the latest templates.

Managed files are tracked through `[tool.forge]` metadata embedded in
`pyproject.toml`. No separate status file.

### Optional components

Components are reusable managed features like Prettier, EditorConfig, PyPI
publishing, Astro Starlight docs, and Codecov. Forge uses sensible blueprint defaults and
records only explicit deviations in `[tool.forge.overrides]`.

```bash
# List components and their supported blueprints
forge components --json

# Add Prettier to an existing project
forge sync --path . --set prettier=true

# Remove it
forge sync --path . --set prettier=false
```

---

## Workflows

### Starting a new project

Interactive mode prompts for the blueprint and its fields, shows a review
summary, then asks for confirmation before writing files.

```bash
forge init
```

Non-interactive mode requires `--blueprint` and `--yes`:

```bash
forge init --blueprint python-library --path ./my-pkg --project-name my-pkg --yes
```

Other useful flags:

| Flag        | Effect                                            |
| ----------- | ------------------------------------------------- |
| `--json`    | Machine-readable creation report on stdout        |
| `--dry-run` | Preview generated files without creating anything |
| `--diff`    | Include text diffs in human dry-run output        |
| `--github`  | Also create a GitHub repository and push          |

<details>
<summary>Blueprint-specific flags and interactive mode</summary>

Python library blueprints accept `--package-name`, optional `--author-name`,
optional `--author-email`, `--license`, `--python-min`, and component flags.

Rust library blueprints accept `--package-name`, optional `--author-name`,
optional `--author-email`, `--license`, and component flags.

`any-project` accepts component flags.

`--package-name` defaults from `--project-name` when omitted.

For library blueprints, `--license` defaults to `BSD-3-Clause`. Supported
OSI-approved license IDs are `BSD-3-Clause`, `MIT`, `Apache-2.0`,
`BSD-2-Clause`, and `ISC`.

`--python-min` accepts values from `3.8` through `3.14`. Generated CI tests the
configured minimum through `3.14` on Ubuntu and runs a lightweight Windows smoke
job on `windows-latest`.

Managed option flags: `--ci`, `--forge-sync`, `--docs-pages`,
`--workflow-quality`, `--docs`, `--prettier`, `--editorconfig`, `--codecov`,
and `--pypi-publish`. Use `--ci=false`, `--forge-sync=false`,
`--docs-pages=false`, or `--workflow-quality=false` when a repository needs
custom GitHub Actions workflows, for example enterprise runners or private
dependency checkout. Use `--editorconfig=false` to disable
EditorConfig (enabled by default for all blueprints) and `--docs=false` to
disable Astro Starlight docs (enabled by default).

If you enable `--pypi-publish`, Forge writes a commented trusted-publishing
workflow at `.github/workflows/publish-pypi.yaml`; register that workflow as a
trusted publisher in PyPI before uncommenting the publish step.

Boolean flags accept script-friendly forms: `--pypi-publish` or
`--pypi-publish=true`.

GitHub flags: `--github-owner` and `--github-visibility` require `--github`.
Visibility defaults to public.

In interactive mode, prompts include editable defaults and the review summary
includes a copyable `forge init ... --yes` command so the same setup can be
reused in automation. When stdin is not a terminal, prompt-driven setup fails
fast with a `--yes` hint. Use `--yes`, `--json`, or `--dry-run` to bypass the
confirmation step.

When `--github` is enabled, Forge runs `uv lock` before the initial commit so
the pushed repository includes the lockfile. If GitHub creation fails, the
local project is left in place.

</details>

### Adopting Forge in an existing repo

For an existing repository, start with interactive setup so Forge can collect the
metadata it needs before it writes managed infrastructure:

```bash
forge init --path . --blueprint python-library
```

For non-interactive use, Forge can infer the blueprint for high-confidence
Python and Rust package repositories. Python inference uses PEP 621
`pyproject.toml` plus source-layout or `uv` build-backend signals, then fills
`--project-name`, `--description`, and `--python-min` from project metadata:

```bash
forge init --path . --dry-run --json --yes
forge init --path . --blueprint python-library --yes
```

If the project metadata does not exist yet, pass the required setup fields
explicitly:

```bash
forge init \
  --path . \
  --blueprint python-library \
  --project-name my-library \
  --description "My library" \
  --yes
```

`forge init` writes Forge-managed infrastructure and `[tool.forge]` metadata, not
starter source files. Existing user-owned files at managed paths are preserved by
default and recorded in Forge metadata as ignored managed paths. JSON and human
output include reason codes such as `new_managed_file`, `metadata_append`, and
`existing_user_file_preserved` so adoption plans are reviewable.

When adopting an existing `pyproject.toml`, Forge marks it as external with
`pyproject = "external"`, preserves existing build-system, dependency groups,
pytest, Ruff, coverage, mypy, ty, and other tool tables, and appends only Forge
metadata. Later `forge sync` refreshes Forge metadata without taking over those
external sections.

Forge detects existing Sphinx, MkDocs, Starlight/Astro docs systems, and
existing Markdown content under `docs/`, and does not create a parallel Starlight
`docs/` site unless you explicitly request docs takeover. With `--takeover-docs`,
Forge can relocate a simple existing docs page into the canonical
`docs/src/content/docs/index.mdx` location; otherwise it reports that manual
migration is required. With `--takeover-ci`, Forge can relocate a compatible
existing release workflow into the canonical release-please path instead of
leaving duplicate workflow infrastructure. Existing GitHub workflow infrastructure
is preserved by default instead of adding a parallel Forge-managed
release/workflow scaffold, and hook config remains preserved by default.

Use a dry run first when adopting a repository that already has infrastructure
such as `pyproject.toml`, `README.md`, CI workflows, hooks, or a `justfile`:

```bash
forge init --path . --blueprint python-library --project-name my-library --description "My library" --dry-run --diff
```

| Flag               | Effect                                                  |
| ------------------ | ------------------------------------------------------- |
| `--dry-run`        | Preview what would be written                           |
| `--json`           | Machine-readable init report                            |
| `--diff`           | Include text diffs for managed file changes             |
| `--ignore PATH`    | Exclude a Forge-managed path or directory prefix (`docs/` for prefixes) |
| `--takeover PATH`  | Convert one existing user-owned path/prefix to managed (`docs/` for prefixes) |
| `--takeover-docs`  | Convert existing docs files to Forge-managed Starlight   |
| `--takeover-ci`    | Convert existing GitHub workflow files to Forge-managed  |
| `--takeover-hooks` | Convert existing hook config to Forge-managed            |
| `--takeover-all`   | Convert all generated-path conflicts to Forge management |
| `--yes`            | Confirm non-interactive apply after reviewing the plan   |

After init succeeds, future changes flow through `forge sync --path .`.

Repositories that already have `[tool.forge]` metadata are rejected by `init`;
use `forge sync` instead.

### Keeping infrastructure current

```bash
# Check for drift (exit code reflects result)
forge sync --path . --check

# Preview changes
forge sync --path . --dry-run --diff

# Apply changes
forge sync --path . --yes
```

Forge reads `[tool.forge]` from `pyproject.toml`, compares the registered
managed files against the templates, and writes creates, updates, relinks, or
removals as needed. Symlink relinks and text file updates go through temporary
paths before replacing targets.

| Flag              | Effect                             |
| ----------------- | ---------------------------------- |
| `--json`          | Machine-readable sync report       |
| `--dry-run`       | Preview without writing            |
| `--diff`          | Include text diffs                 |
| `--check`         | Exit nonzero on drift (for CI)     |
| `--set key=value` | Enable or disable a managed option |
| `--yes`           | Skip confirmation prompt           |

Use `--set` to opt out of Forge-managed workflows that a repository owns
directly. For example, enterprise repositories with private dependency checkout
or custom runners can preserve their workflow stack with:

```bash
forge sync --path . --yes \
  --set ci=false \
  --set forge-sync=false \
  --set docs-pages=false \
  --set workflow-quality=false
```

The generated forge-sync workflow only runs `uv lock` when Forge changes
lockfile-relevant `pyproject.toml` metadata. Pure `[tool.forge]` metadata
adoption does not refresh the lockfile, so infrastructure syncs do not compete
with Dependabot or other dependency-update automation for unrelated lockfile
churn.

For Python repositories with existing project dependencies, entry points,
`tool.uv.sources`, or `tool.uv.workspace`, Forge treats `pyproject.toml` as
externally owned during sync and preserves that project metadata. Forge records
`pyproject = "external"` under `[tool.forge]` and continues managing the
surrounding infrastructure.

When sync sees existing workflow files with strong enterprise ownership signals,
such as private checkout actions, private runners, or private dependency install
flags, it records the matching workflow options as disabled, or records a path
ignore for Forge-managed workflows without a dedicated option, instead of
replacing those files. Pass `--set ci=true`, `--set forge-sync=true`,
`--set docs-pages=true`, or `--set workflow-quality=true` to explicitly hand a
workflow option back to Forge. To hand back a path-ignored workflow, first
replace the custom file or remove its enterprise-only requirements, then remove
the path from `ignore`.

When a managed workflow option is turned off, Forge removes the existing
workflow only if it still exactly matches the previously generated Forge
workflow. Custom workflow files are left in place.

<details>
<summary>Sync behavior details</summary>

Managed text updates and symlink relinks are staged through temporary paths in
the same directory before replacing the target, so a failed write does not leave
a truncated file or missing link behind.

If `tool.forge.blueprint_version` is newer than the running Forge binary,
managed commands fail fast and ask for an upgrade first.

`--set` changes that only touch `[tool.forge.overrides]` preserve unrelated
`pyproject.toml` comments and formatting.

When `--set` adds a managed component, its files are generated. When `--set`
removes a component, its files are deleted. User-owned files are never touched
by component cleanup.

</details>

### Diagnostics

```bash
# Check all required tools
forge doctor

# Check tools for a specific project type
forge doctor --blueprint python-library

# Check tools for the current directory's blueprint
forge doctor --path .
```

| Flag                 | Effect                                                          |
| -------------------- | --------------------------------------------------------------- |
| `--json`             | Structured diagnostics for CI or setup scripts                  |
| `--blueprint <name>` | Scope to a blueprint's required toolchain                       |
| `--path .`           | Detect blueprint from `pyproject.toml` and check matching tools |

<details>
<summary>Doctor output details</summary>

Doctor exits nonzero when required tools are missing, while printing every
checked tool and detected version. Human and JSON output include `next_steps`
for recovery.

JSON output includes `status_code` (`ok`, `missing_required`), `scope_code`
(`global`, `blueprint`, `path`), `blueprint_version` (when path-scoped), and
per-tool `status_code` (`installed`, `missing_required`, `missing_optional`).

Path-scoped doctor validates strict `[tool.forge]` metadata before checking
tools, so corrupt metadata fails before environment diagnostics.

If the path is not yet Forge-managed, doctor points at `forge init --path ...`.

</details>

### Shell completions

```bash
forge completions bash
forge completions zsh
forge completions fish
forge completions powershell
forge completions elvish
```

---

## Generated output

All blueprints generate:

- **Agent instructions** -- shared `AGENTS.md` with a managed `CLAUDE.md` link
  (symlink where supported; Windows falls back to a hardlink or copy when symlink
  privileges are unavailable)
- **CI** -- GitHub Actions with read-only token permissions, `uv` caching,
  lockfile verification (`uv lock --check`), lightweight Windows smoke jobs,
  bounded timeouts, and cancellation of stale runs on the same ref
- **Hooks** -- `prek` hooks for formatting, linting, metadata hygiene, and
  lockfile verification. Generated spell-checking skips `data/**` by default so
  domain datasets and exported CSVs do not break infrastructure-only PRs.
- **Release** -- `release-please` configuration
- **Scheduled sync workflow** -- opens an infrastructure sync PR when drift
  is found (write permissions only, serialized runs)

### Blueprint-specific output

| Blueprint        | Generates                                                                                                                                      |
| ---------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| `python-library` | `pyproject.toml`, `uv.lock`, `.python-version`, `src/` layout, pytest config, `just` tasks, optional Astro Starlight docs, optional PyPI publishing via OIDC |
| `rust-library`   | `Cargo.toml`, `src/` layout, Rust-focused `just` tasks, optional Astro Starlight docs                                                                        |
| `any-project`    | Language-agnostic: the common files above plus Astro Starlight docs by default, no package source files                                                      |

### Optional components

| Component       | What it adds                                                                                 |
| --------------- | -------------------------------------------------------------------------------------------- |
| Prettier        | `.prettierrc.json`, `.prettierignore`, and pre-commit hook for JSON/YAML/Markdown formatting |
| EditorConfig    | `.editorconfig` baseline for cross-editor whitespace consistency                             |
| Docs            | Astro Starlight documentation scaffold and `just docs` recipe                                |
| Codecov         | CI integration for coverage reporting (where supported)                                      |
| PyPI publishing | Trusted publishing via OIDC, `pypi` GitHub environment, serialized release/publish workflows |

Only component choices that differ from blueprint defaults are recorded in
`[tool.forge.overrides]`. Toggling them later updates managed files without
touching anything user owns.

---

## Output conventions

Forge output is linear and automation-friendly: completion summaries use section
headings, `[ok]` status lines, key/value context, and copyable next-step
commands instead of spinners or terminal-only effects.

- Interactive terminals get restrained color for scanning.
- Redirected output, CI, `TERM=dumb`, and `NO_COLOR` stay plain.
- `--color auto|always|never` controls color policy explicitly.

---

## Development

```bash
git clone https://github.com/pesap/forge
cd forge
cargo build
cargo test
```

Run all quality gates:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test
```

---

## License

BSD 3-Clause. See [LICENSE](LICENSE).
