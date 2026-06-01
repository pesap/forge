<table>
  <tr>
    <td width="220" valign="top">
      <img src="assets/forge-logo.svg" alt="forge logo" width="210" />
    </td>
    <td valign="top">

# forge

**Scaffold repositories from blueprints and keep their infrastructure up to date.**

[![Forge](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/pesap/forge/main/.github/badges/forge.json)](https://github.com/pesap/forge)
[![Managed by humans](https://img.shields.io/badge/managed%20by-humans-1f6feb)](https://github.com/pesap/forge)
[![Managed with uv](https://img.shields.io/badge/managed%20with-uv-7c3aed.svg)](https://docs.astral.sh/uv/)
[![ty](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/astral-sh/ty/main/assets/badge/v0.json)](https://github.com/astral-sh/ty)
[![CI](https://github.com/pesap/forge/actions/workflows/ci.yml/badge.svg)](https://github.com/pesap/forge/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/pesap/forge)](https://github.com/pesap/forge/releases)
[![License](https://img.shields.io/badge/license-BSD--3--Clause-blue.svg)](LICENSE)

    </td>

  </tr>
</table>

`forge` generates managed project infrastructure from blueprints, then reapplies
those managed artifacts during updates. No separate status file needed: it
reads project metadata from `[tool.forge]` in `pyproject.toml` to drive
update, drift check, and cleanup.

Current blueprints: `any-project`, `python-library`, `rust-library`.

---

## Quickstart

Create a Python library project:

```bash
forge new \
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

Then inside the generated project:

```bash
uv sync --all-groups
just verify
```

That is it. Generated projects include managed CI, release-please
configuration, agent instructions, `prek` hooks, and a scheduled Forge update
workflow that opens an infrastructure update PR when drift is detected.

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

---

## Commands

| Command             | What it does                                                                      |
| ------------------- | --------------------------------------------------------------------------------- |
| `forge new`         | Create a new project from a blueprint                                             |
| `forge init`        | Adopt Forge-managed infrastructure in an existing repository                      |
| `forge update`      | Refresh managed infrastructure in a Forge-managed project                         |
| `forge blueprints`  | List available blueprints and their setup fields                                  |
| `forge components`  | List reusable optional components (Prettier, EditorConfig, PyPI publishing, etc.) |
| `forge doctor`      | Check local toolchain health                                                      |
| `forge completions` | Emit shell completion scripts (bash, zsh, fish, powershell, elvish)               |
| `forge self update` | Update the forge binary                                                           |

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
  Generated once by `forge new`.
- **Managed files** -- CI workflows, hook configs, documentation config, editor
  settings. Regenerated on every `forge update` run, so they stay current with
  the latest templates.

Managed files are tracked through `[tool.forge]` metadata embedded in
`pyproject.toml`. No separate status file.

### Optional components

Components are reusable opt-in features like Prettier, EditorConfig, PyPI
publishing, MkDocs, and Codecov. Forge uses sensible blueprint defaults and
records only explicit deviations in `[tool.forge.overrides]`.

```bash
# List components and their supported blueprints
forge components --json

# Add Prettier to an existing project
forge update --path . --set prettier=true

# Remove it
forge update --path . --set prettier=false
```

---

## Workflows

### Starting a new project

Interactive mode prompts for the blueprint and its fields, shows a review
summary, then asks for confirmation before writing files.

```bash
forge new
```

Non-interactive mode requires `--blueprint` and `--yes`:

```bash
forge new --blueprint python-library --path ./my-pkg --project-name my-pkg --yes
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

`any-project` accepts `--author-name`, `--author-email`, `--license`, and
component flags.

`--package-name` defaults from `--project-name` when omitted.

`--python-min` accepts values from `3.8` through `3.14`. Generated CI tests the
configured minimum through `3.14`.

Component flags: `--prettier`, `--editorconfig`, `--docs`, `--codecov`,
`--pypi-publish`. Use `--docs=false` to disable MkDocs (enabled by default for
`any-project` and `rust-library`).

If you enable `--pypi-publish`, Forge writes a commented trusted-publishing
workflow at `.github/workflows/publish-pypi.yaml`; register that workflow as a
trusted publisher in PyPI before uncommenting the publish step.

Boolean flags accept script-friendly forms: `--pypi-publish` or
`--pypi-publish=true`.

GitHub flags: `--github-owner` and `--github-visibility` require `--github`.
Visibility defaults to public.

In interactive mode, prompts include editable defaults and the review summary
includes a copyable `forge new ... --yes` command so the same setup can be
reused in automation. When stdin is not a terminal, prompt-driven setup fails
fast with a `--yes` hint. Use `--yes`, `--json`, or `--dry-run` to bypass the
confirmation step.

When `--github` is enabled, Forge runs `uv lock` before the initial commit so
the pushed repository includes the lockfile. If GitHub creation fails, the
local project is left in place.

</details>

### Adopting Forge in an existing repo

```bash
forge init --path . --blueprint python-library --yes
```

`forge init` writes only managed infrastructure and metadata, not starter source
files. Conflicting existing files are reported instead of overwritten.

| Flag        | Effect                                   |
| ----------- | ---------------------------------------- |
| `--dry-run` | Preview what would be written            |
| `--json`    | Machine-readable init report             |
| `--diff`    | Include text diffs for conflicting files |
| `--force`   | Overwrite conflicting managed paths      |

After init succeeds, future changes flow through `forge update --path .`.

Repositories that already have `[tool.forge]` metadata are rejected by `init`;
use `forge update` instead.

### Keeping infrastructure current

```bash
# Check for drift (exit code reflects result)
forge update --path . --check

# Preview changes
forge update --path . --dry-run --diff

# Apply changes
forge update --path . --yes
```

Forge reads `[tool.forge]` from `pyproject.toml`, compares the registered
managed files against the templates, and writes creates, updates, relinks, or
removals as needed. Symlink relinks and text file updates go through temporary
paths before replacing targets.

| Flag              | Effect                             |
| ----------------- | ---------------------------------- |
| `--json`          | Machine-readable update report     |
| `--dry-run`       | Preview without writing            |
| `--diff`          | Include text diffs                 |
| `--check`         | Exit nonzero on drift (for CI)     |
| `--set key=value` | Enable or disable a managed option |
| `--yes`           | Skip confirmation prompt           |

<details>
<summary>Update behavior details</summary>

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

- **Agent instructions** -- shared `AGENTS.md` with a managed `CLAUDE.md` symlink
- **CI** -- GitHub Actions with read-only token permissions, `uv` caching,
  lockfile verification (`uv lock --check`), bounded timeouts, and cancellation
  of stale runs on the same ref
- **Hooks** -- `prek` hooks that check Forge infrastructure drift
- **Release** -- `release-please` configuration
- **Scheduled update workflow** -- opens an infrastructure update PR when drift
  is found (write permissions only, serialized runs)

### Blueprint-specific output

| Blueprint        | Generates                                                                                                                                      |
| ---------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| `python-library` | `pyproject.toml`, `uv.lock`, `.python-version`, `src/` layout, pytest config, `just` tasks, optional MkDocs, optional PyPI publishing via OIDC |
| `rust-library`   | `Cargo.toml`, `src/` layout, Rust-focused `just` tasks, optional MkDocs                                                                        |
| `any-project`    | Language-agnostic: the common files above plus MkDocs by default, no package source files                                                      |

### Optional components

| Component       | What it adds                                                                                 |
| --------------- | -------------------------------------------------------------------------------------------- |
| Prettier        | `.prettierrc`, `prek` hook for JSON/YAML/Markdown formatting                                 |
| EditorConfig    | `.editorconfig` baseline for cross-editor whitespace consistency                             |
| MkDocs          | Documentation scaffold, `just docs` recipe                                                   |
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
