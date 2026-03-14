# forge

`forge` is a project generator and infra upgrader.

Current blueprint support:

- `python-library`

## Commands

- `forge new` creates a new project from a blueprint
- `forge upgrade` upgrades managed infra files in an existing project
- `forge self update` updates the `forge` CLI (or prints install-method guidance)
- `forge doctor` reports local tool status

## Quickstart

```bash
cargo run -- new \
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
