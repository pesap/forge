set dotenv-load := false

default:
    @just --list

hooks-install:
    prek install

setup: hooks-install

ci-fmt:
    cargo fmt --all --check

ci-clippy:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

ci-prek:
    prek run --all-files

ci-test:
    cargo test

ci: ci-fmt ci-clippy ci-prek ci-test

demo-clean:
    rm -rf demo-pylib demo-rustlib demo-anyproject

_demo-refresh path create_cmd:
    bash -c 'if [ -d "$1" ]; then cargo run -- sync --path "$1" --yes || (rm -rf "$1" && eval "$2"); else eval "$2"; fi' -- {{path}} {{create_cmd}}
    cargo run -- sync --path {{path}} --check

demo-pylib:
    just _demo-refresh demo-pylib 'cargo run -- new --blueprint python-library --path demo-pylib --project-name demo-pylib --package-name demo_pylib --description "Demo Python library" --author-name "Forge Demo" --author-email "demo@example.com" --no-git-history --yes'

demo-rustlib:
    just _demo-refresh demo-rustlib 'cargo run -- new --blueprint rust-library --path demo-rustlib --project-name demo-rustlib --package-name demo_rustlib --description "Demo Rust library" --author-name "Forge Demo" --author-email "demo@example.com" --no-git-history --yes'

demo-anyproject:
    just _demo-refresh demo-anyproject 'cargo run -- new --blueprint any-project --path demo-anyproject --project-name demo-anyproject --description "Demo any-project repo" --no-git-history --yes'

demo-all: demo-pylib demo-rustlib demo-anyproject
