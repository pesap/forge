set dotenv-load := false

default:
    @just --list

demo-clean:
    rm -rf demo-pylib demo-rustlib demo-anyproject

demo-pylib:
    if [ -d demo-pylib ]; then cargo run -- update --path demo-pylib --yes; else cargo run -- new --blueprint python-library --path demo-pylib --project-name demo-pylib --package-name demo_pylib --description "Demo Python library" --author-name "Forge Demo" --author-email "demo@example.com" --yes; fi
    cargo run -- update --path demo-pylib --check

demo-rustlib:
    if [ -d demo-rustlib ]; then cargo run -- update --path demo-rustlib --yes; else cargo run -- new --blueprint rust-library --path demo-rustlib --project-name demo-rustlib --package-name demo_rustlib --description "Demo Rust library" --author-name "Forge Demo" --author-email "demo@example.com" --yes; fi
    cargo run -- update --path demo-rustlib --check

demo-anyproject:
    if [ -d demo-anyproject ]; then cargo run -- update --path demo-anyproject --yes; else cargo run -- new --blueprint any-project --path demo-anyproject --project-name demo-anyproject --description "Demo any-project repo" --yes; fi
    cargo run -- update --path demo-anyproject --check

demo-all: demo-pylib demo-rustlib demo-anyproject
