# forge

`forge` is a project generator and managed infrastructure updater. It renders
managed project infrastructure from blueprints, then reapplies those managed
artifacts during updates without keeping a separate status file in the target
repository.

Current blueprint support:

- `any-project`
- `python-library`
- `rust-library`

## Commands

- `forge new` creates a new project from a blueprint
- `forge init` adds Forge-managed infrastructure to an existing repository
- `forge blueprints` lists available blueprints and their intended use
- `forge components` lists reusable optional components such as Prettier
- `forge completions` emits shell completion scripts
- `forge update` updates managed infra files in an existing project
- `forge self update` updates the `forge` CLI (or prints install-method guidance)
- `forge doctor` reports local tool status

Running `forge` with no subcommand prints top-level help and quickstart examples
and exits successfully for first-run discovery.

`forge doctor` exits nonzero when required local tools are missing, while still
printing every checked tool and detected version so setup problems can be fixed
in one pass. When required tools are missing, human and JSON output include
`next_steps` for recovery.
Use `forge doctor --blueprint python-library`, `rust-library`, or `any-project`
to check only the tools required for that project type.
Inside an existing Forge-managed repository, use `forge doctor --path .` to
detect the blueprint and enabled components from `pyproject.toml`, then check
only the matching toolchain. Path-scoped doctor checks validate the current
strict `[tool.forge]` metadata before reporting tool status, so corrupt managed
metadata fails before environment diagnostics.
If the path is not yet Forge-managed, doctor points at `forge init --path ...`
for repository adoption.
When a scoped doctor check fails, `next_steps` keeps the same `--path` or
`--blueprint` scope for the rerun command.
Use `forge doctor --json` to emit the same diagnostics as structured JSON for
CI or setup scripts.
Doctor JSON includes top-level `status_code` values `ok` and
`missing_required`.
Doctor JSON includes `scope_code` values `global`, `blueprint`, or `path` so
automation can branch on the diagnostic scope without parsing human output.
When scoped to a managed project path, doctor JSON also includes
`blueprint_version`.
Each tool entry includes stable `status_code` values: `installed`,
`missing_required`, or `missing_optional`.

Use `forge completions bash`, `zsh`, `fish`, `powershell`, or `elvish` to print
completion scripts for your shell.

When run interactively, `forge new` prompts for the blueprint and only then asks
for the fields that blueprint needs, including editable defaults such as license
and minimum Python version. In `--yes` mode, `--blueprint` is required so
automation always records an explicit project type, defaulted fields keep their
documented defaults, and missing required setup flags are reported together
before any files are written.
After interactive prompts, Forge shows a setup review summary and asks for
confirmation before writing files; `--yes`, `--json`, and `--dry-run` bypass
that confirmation step. The review includes a copyable `forge new ... --yes`
command so prompt-driven setup and automation use the same flags.
When stdin is not an interactive terminal, prompt-driven setup fails fast with a
`--yes` hint so CI and scripts do not hang waiting for input.
Use `forge new --json --yes ...` to emit a machine-readable creation report with
a clean JSON stdout stream for scripts. Human and JSON creation reports include
the selected managed options so setup choices are visible before future
`forge update` runs.
Creation JSON includes both `blueprint` and `blueprint_version`.
`forge new --json` includes `status_code` values `created` or `dry_run`.
Use `forge new --dry-run ...` to preview generated files without creating the
destination directory, initializing git, or touching GitHub. If `--github` is
included, the preview reports the requested repository visibility without
requiring `gh`.
Dry-run reports include a copyable `forge new ...` command with the same setup
flags so the reviewed project can be created directly.
Add `--diff` to a human dry run to inspect generated text files before creating
the project.
For `python-library` and `rust-library`, `--package-name` defaults from
`--project-name` when omitted.

Use `forge init --path . --blueprint ... --yes` when a repository already has
source code and should start using Forge-managed infrastructure. `init` writes
only managed infrastructure and metadata, not starter source files. During
adoption, differing existing managed paths are reported as conflicts instead of
being overwritten; use `forge init --dry-run` or `--json` to inspect the plan.
Interactive `forge init` runs also show a setup review summary and ask for
confirmation before applying managed infrastructure; `--yes`, `--json`, and
`--dry-run` bypass that confirmation step. The review includes a copyable
`forge init ... --yes` command so prompt-driven setup and automation stay aligned.
The JSON report includes the selected options, planned actions, and
`next_steps` for conflict recovery, dry-run, and applied initialization flows.
Initialization JSON includes both `blueprint` and `blueprint_version`.
`forge init --json` includes `status_code` values `initialized`, `dry_run`, or
`conflicts`.
Dry-run reports include a copyable `forge init ...` command with the same setup
flags so the reviewed plan can be applied directly.
Successful init reports include `cd ...`, `uv sync --all-groups`, and
`just verify` next steps so commands run from the adopted repository.
Add `--diff` to a dry run to inspect text changes for conflicting managed files
before accepting ownership.
After reviewing the planned managed files, rerun with `--force` to let Forge
overwrite those selected infrastructure paths. Conflict reports include a
copyable `forge init ... --force` command with the original setup flags.
Repositories that already contain `[tool.forge]` metadata are rejected by
`forge init`; use `forge update --path .` for managed repositories instead.
After init succeeds, future changes flow through `forge update --path .`.

## Quickstart

```bash
cargo run -- new \
  --blueprint python-library \
  --path ./demo-lib \
  --project-name demo-lib \
  --package-name demo_lib \
  --description "Demo package" \
  --author-name "Jane Doe" \
  --author-email jane@example.com \
  --license BSD-3-Clause \
  --python-min 3.11 \
  --yes
```

Then inside generated repo:

```bash
uv sync --all-groups
just verify
```

Generated Python projects include shared agent instructions in `AGENTS.md`, a
managed `CLAUDE.md` symlink to those instructions, `uv`/`prek` tooling,
GitHub Actions, release-please configuration, and a scheduled forge update
workflow that opens an infrastructure update PR.
Generated `prek` hooks check Forge-managed infrastructure drift locally with
`forge update --path . --check`.
Generated CI workflows use read-only repository token permissions, disable
persisted checkout credentials, cancel stale runs for the same ref, cache `uv`
dependencies from `pyproject.toml` and `uv.lock`, enforce that CI does not
rewrite the lockfile with an explicit `uv lock --check`, use current maintained
action majors, and use bounded job timeouts. The scheduled Forge update
workflow serializes update runs and requests write permissions only so it can
open update PRs. Update PRs refresh `uv.lock` after `forge update --path .` so
dependency metadata and generated locked CI stay in sync.
Generated `just verify` recipes and hooks use locked, non-mutating `uv` commands
for verification steps, while `sync`, `format`, and fix-oriented tasks remain
allowed to refresh the environment intentionally.
Optional trusted PyPI publishing uses a dedicated `pypi` GitHub environment and
job-scoped OIDC permissions. Generated release and publish workflows serialize
duplicate runs so release PR creation and PyPI publishing do not overlap for the
same ref or release.

Language-agnostic infrastructure projects use `--blueprint any-project`. They
generate the same managed agent instructions, `uv`/`prek` workflow, CI, and
scheduled Forge update workflow without creating language-specific package
source files. They also include MkDocs documentation by default.

Rust library projects use `--blueprint rust-library`. They generate Cargo
package files, Rust-focused `just` tasks, cargo fmt/clippy hooks, CI, shared
agent instructions, MkDocs project docs, and the same scheduled Forge update
workflow.

Optional components are registered in Forge and stored in `[tool.forge.options]`
in `pyproject.toml`.
Use `forge components` or `forge components --json` to inspect the reusable
component registry, managed files, hook support, mutating format commands,
non-mutating check commands, and supported blueprints.
Use `forge components --blueprint python-library` (or another blueprint) to
focus on components supported by a specific project type.
The JSON form is an object with top-level `status_code` (`ok`) and a
`components` array.
For example, `forge new --prettier ... --yes` adds managed Prettier config and a
local `prek` hook for JSON, YAML, and Markdown using a pinned Prettier version.
`forge new --editorconfig ... --yes` adds a managed `.editorconfig` baseline for
cross-editor whitespace consistency.
`just format` applies Prettier when the component is enabled, while generated
hooks and CI use Prettier check mode so verification does not rewrite files.
Later `forge update --path .` uses that existing metadata to add, refresh, or
remove managed component files without a separate status file.
Boolean creation flags accept both script-friendly values and shorthand forms:
use `--pypi-publish` or `--pypi-publish=true` to enable trusted PyPI publishing,
use `--prettier=false` to leave Prettier disabled explicitly in generated
metadata, and use `--docs=false` or `--codecov=false` to disable components
that are enabled by default.
For optional setup booleans, use explicit boolean values (for example
`--prettier=false` and `--github=false`).
Creation and init flags are kebab-case.
When docs are disabled, Forge omits MkDocs files, the MkDocs dependency group,
and the generated `just docs` recipe.
Blueprint-specific creation flags are validated before files are written, so
language-specific options such as `--python-min`, `--package-name`, or
`--author-name` are rejected when the selected blueprint does not use them.
For `python-library`, `--python-min` must be a Python 3 `major.minor` value from
`3.8` through `3.14`; Forge uses it in `.python-version`, packaging metadata,
and CI. Generated CI never tests Python versions below that configured minimum,
and includes supported newer Python releases through `3.14`.
GitHub repository flags are also strict: `--github-owner` and
`--github-visibility` require `--github`, and visibility defaults to public only
when repository creation is enabled.
When `--github` is enabled, Forge runs `uv lock` before the initial commit so
the pushed repository includes the lockfile required by generated locked CI.
If dependency locking or GitHub repository creation fails after local
generation, Forge leaves the local project in place and prints a retry command
from that project directory.

Use `forge update --path . --dry-run` to preview managed creates, updates,
relinks, and removals before changing files.
Use `forge update --path . --yes` to apply managed changes without an
interactive confirmation prompt.
In interactive terminals, `forge update` asks for confirmation before applying
managed changes; use `--yes` to skip the prompt.
When stdin is not an interactive terminal, apply-mode updates fail fast unless
`--yes` is passed.
When apply-mode update finds no managed drift, Forge reports `Project checked`
and `managed infrastructure is already current`.
Add `--diff` to human dry-runs, or to `--check`, when you want a text diff of
managed file creates, updates, and removals before applying an update.
Managed text updates and symlink relinks are staged through temporary paths in
the same directory before replacing the target, so a failed write does not leave
a truncated file or missing link behind.
Add `--json` to emit the same update report as structured JSON for automation.
The JSON report includes the detected blueprint, the project-stored blueprint
`version`, project-stored blueprint `options`, top-level `status_code`,
`changes` and `conflicts` counts,
`next_steps`, and
per-file actions with stable `reason_code` values for conflicts. Filesystem
conflicts such as a directory at a managed file path, a non-directory managed
parent path, or an unreadable managed text file are reported before Forge writes
any changes, with `next_steps` pointing at conflict recovery.
`status_code` values are `updated`, `current`, `dry_run`, `out_of_date`, and
`conflicts`.
Blueprint and component file paths, including managed symlink targets, must be
repository-relative and cannot use absolute paths or `..` traversal, so
generated infrastructure cannot escape the target repository.
Forge reads only the current strict `[tool.forge]` schema. Unknown metadata is
rejected.
If `tool.forge.blueprint_version` is newer than the running Forge binary,
managed commands fail fast and ask you to upgrade Forge before applying changes.
Use `forge update --path . --check` in CI to fail when managed infrastructure
has drifted. Drift reports include `forge update --path ...` in `next_steps` so
CI logs and JSON automation point directly at the repair command.
Dry-run and check reports preserve any `--set` overrides and include `--yes` in
their apply command, so previews can be rerun directly after review.
Use `forge update --path . --set prettier=true` to enable an optional managed
component after project creation; use `prettier=false` to remove its managed
files.
When a `--set` change only affects `[tool.forge.options]`, Forge preserves
unrelated `pyproject.toml` comments and formatting while changing the selected
option value.
Human and JSON update output include `uv lock` as a next step when the planned
managed changes touch `pyproject.toml`.
Option names in `--set` use the same kebab-case names shown by
`forge blueprints`; for example, use `pypi-publish=true`.
If `[tool.forge.options]` is missing or missing newer supported keys, Forge
defaults those option values to blueprint defaults during metadata validation
and writes the canonical full option set on the next successful `forge update`.
Unknown option keys still fail so typos do not silently change behavior.
`forge blueprints --json` emits the blueprint registry, setup fields, required
local tools, supported managed options, each default state, descriptions, and
canonical create/init/check commands as structured JSON. The JSON form is an
object with top-level `status_code` (`ok`) and a `blueprints` array.

Blueprints are selected explicitly with `--blueprint`.

Command output is intentionally linear and automation-friendly: completion
summaries use section headings, `[ok]` status lines, key/value context, and
copyable next-step commands instead of spinners or terminal-only effects.
When stdout is an interactive terminal, Forge adds restrained color for scanning;
redirected output, CI, `TERM=dumb`, and `NO_COLOR` stay plain (`--color never`
also forces plain output).
Use `--color auto|always|never` to control color policy explicitly.
