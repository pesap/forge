use std::fs;
use std::io::{IsTerminal, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use similar::TextDiff;

use crate::blueprint::files::{GeneratedFile, GeneratedFiles, ManagedFileAction};
use crate::ui;

pub fn print_diffs(
    root: &Path,
    actions: &[ManagedFileAction],
    managed_files: &GeneratedFiles,
) -> Result<()> {
    let diffs = actions
        .iter()
        .filter_map(|action| diff_for_action(root, action, managed_files).transpose())
        .collect::<Result<Vec<_>>>()?;

    if diffs.is_empty() {
        return Ok(());
    }

    ui::section("Managed diff");
    let diff_text = diffs.concat();
    if !page_diff(&diff_text)? {
        print!("{diff_text}");
    }

    Ok(())
}

fn page_diff(diff_text: &str) -> Result<bool> {
    let Some(pager) = pager_from_env(std::io::stdout().is_terminal()) else {
        return Ok(false);
    };

    let mut child = pager_command(&pager)
        .stdin(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to start pager `{pager}`"))?;

    if let Some(mut stdin) = child.stdin.take()
        && let Err(error) = stdin.write_all(diff_text.as_bytes())
        && error.kind() != std::io::ErrorKind::BrokenPipe
    {
        return Err(error).context("failed to write diff to pager");
    }

    child
        .wait()
        .with_context(|| format!("failed to wait for pager `{pager}`"))?;
    Ok(true)
}

fn pager_from_env(stdout_is_terminal: bool) -> Option<String> {
    if !stdout_is_terminal {
        return None;
    }

    std::env::var("PAGER")
        .ok()
        .map(|pager| pager.trim().to_string())
        .filter(|pager| !pager.is_empty())
}

#[cfg(unix)]
fn pager_command(pager: &str) -> Command {
    let mut command = Command::new("sh");
    command.arg("-c").arg(pager);
    command
}

#[cfg(windows)]
fn pager_command(pager: &str) -> Command {
    let mut command = Command::new("cmd");
    command.arg("/C").arg(pager);
    command
}

fn diff_for_action(
    root: &Path,
    action: &ManagedFileAction,
    managed_files: &GeneratedFiles,
) -> Result<Option<String>> {
    match action {
        ManagedFileAction::Create(path)
        | ManagedFileAction::Update(path)
        | ManagedFileAction::MetadataAppend(path) => {
            let Some(GeneratedFile::Text(new_content)) = managed_files.get(path) else {
                return Ok(None);
            };
            let diff = match action {
                ManagedFileAction::Create(path) => render_created_text_diff(path, new_content),
                ManagedFileAction::Update(path) | ManagedFileAction::MetadataAppend(path) => {
                    fs::read_to_string(root.join(path))
                        .map(|old_content| render_text_diff(path, &old_content, new_content))
                        .with_context(|| format!("failed to read {}", root.join(path).display()))?
                }
                _ => String::new(),
            };
            Ok(Some(diff))
        }
        ManagedFileAction::Conflict { path, .. } => {
            let Some(GeneratedFile::Text(new_content)) = managed_files.get(path) else {
                return Ok(None);
            };
            let full_path = root.join(path);
            match fs::read_to_string(&full_path) {
                Ok(old_content) => Ok(Some(render_text_diff(path, &old_content, new_content))),
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::IsADirectory | std::io::ErrorKind::InvalidData
                    ) =>
                {
                    Ok(None)
                }
                Err(error) => {
                    Err(error).with_context(|| format!("failed to read {}", full_path.display()))
                }
            }
        }
        ManagedFileAction::Remove(path) => {
            let full_path = root.join(path);
            if !full_path.is_file() {
                return Ok(None);
            }
            let old_content = fs::read_to_string(&full_path)
                .with_context(|| format!("failed to read {}", full_path.display()))?;
            Ok(Some(render_removed_text_diff(path, &old_content)))
        }
        _ => Ok(None),
    }
}

fn render_created_text_diff(path: &Path, new_content: &str) -> String {
    render_diff(
        "/dev/null".to_string(),
        format!("b/{}", path.display()),
        0,
        diff_line_count(new_content),
        "",
        new_content,
    )
}

fn render_removed_text_diff(path: &Path, old_content: &str) -> String {
    render_diff(
        format!("a/{}", path.display()),
        "/dev/null".to_string(),
        diff_line_count(old_content),
        0,
        old_content,
        "",
    )
}

fn render_text_diff(path: &Path, old_content: &str, new_content: &str) -> String {
    TextDiff::from_lines(old_content, new_content)
        .unified_diff()
        .header(
            &format!("a/{}", path.display()),
            &format!("b/{}", path.display()),
        )
        .to_string()
}

fn render_diff(
    old_path: String,
    new_path: String,
    old_line_count: usize,
    new_line_count: usize,
    old_content: &str,
    new_content: &str,
) -> String {
    let mut diff = format!(
        "--- {}\n+++ {}\n@@ -{},{} +{},{} @@\n",
        old_path,
        new_path,
        diff_start_line(old_line_count),
        old_line_count,
        diff_start_line(new_line_count),
        new_line_count
    );
    for line in old_content.lines() {
        diff.push('-');
        diff.push_str(line);
        diff.push('\n');
    }
    for line in new_content.lines() {
        diff.push('+');
        diff.push_str(line);
        diff.push('\n');
    }
    diff
}

fn diff_start_line(line_count: usize) -> usize {
    if line_count == 0 { 0 } else { 1 }
}

fn diff_line_count(content: &str) -> usize {
    let count = content.lines().count();
    if count == 0 { 1 } else { count }
}

#[cfg(test)]
mod tests {
    use crate::commands::diff::{
        diff_line_count, pager_from_env, render_created_text_diff, render_removed_text_diff,
        render_text_diff,
    };
    use std::path::Path;

    #[test]
    fn pager_from_env_requires_terminal_and_non_empty_pager() {
        assert_eq!(pager_from_env(false), None);
        assert_eq!(
            pager_from_env(true),
            std::env::var("PAGER")
                .ok()
                .map(|pager| pager.trim().to_string())
                .filter(|pager| !pager.is_empty())
        );
    }

    #[test]
    fn render_text_diff_aligns_repeated_blocks() {
        let diff = render_text_diff(
            Path::new("justfile"),
            "format:\n    uv run ruff format .\n    npx --yes prettier@3.8.3 --write --ignore-path .prettierignore --ignore-unknown .\nlint:\n    uv run ruff check --fix .\n    uv run ty check\n\ntest:\n    uv run pytest -q\n\nsmoke:\n    uv run python -c \"import test\"\n\nbuild:\n    uv build\n",
            "format:\n    uv run ruff format .\nlint:\n    uv run ruff check --fix .\n    uv run ty check\n\ntest:\n    uv run pytest -q\n\nsmoke:\n    uv run python -c \"import sandbox\"\n\nbuild:\n    uv build\n",
        );

        assert!(diff.contains("-    npx --yes prettier@3.8.3 --write --ignore-path .prettierignore --ignore-unknown .\n"));
        assert!(diff.contains("-    uv run python -c \"import test\"\n"));
        assert!(diff.contains("+    uv run python -c \"import sandbox\"\n"));
        assert!(!diff.contains("-lint:\n-lint:"));
        assert!(!diff.contains("+lint:\n+lint:"));
    }

    #[test]
    fn render_text_diff_shows_only_changed_hunk_with_context() {
        let diff = render_text_diff(
            Path::new("justfile"),
            "set dotenv-load := false\n\ndefault:\n    @just --list\n\nsmoke:\n    uv run python -c \"import test\"\n\nbuild:\n    uv build\n",
            "set dotenv-load := false\n\ndefault:\n    @just --list\n\nsmoke:\n    uv run python -c \"import sandbox\"\n\nbuild:\n    uv build\n",
        );

        assert!(diff.contains("--- a/justfile\n"));
        assert!(diff.contains("+++ b/justfile\n"));
        assert!(diff.contains("@@ "));
        assert!(diff.contains("     @just --list\n"));
        assert!(diff.contains("-    uv run python -c \"import test\"\n"));
        assert!(diff.contains("+    uv run python -c \"import sandbox\"\n"));
        assert!(!diff.contains("-set dotenv-load := false\n"));
        assert!(!diff.contains("+set dotenv-load := false\n"));
    }

    #[test]
    fn diff_line_count_keeps_empty_files_addressable() {
        assert_eq!(diff_line_count(""), 1);
        assert_eq!(diff_line_count("one\n"), 1);
        assert_eq!(diff_line_count("one\ntwo\n"), 2);
    }

    #[test]
    fn render_created_text_diff_uses_dev_null_old_path() {
        let diff = render_created_text_diff(Path::new(".prettierrc.json"), "{}\n");

        assert!(diff.contains("--- /dev/null\n"));
        assert!(diff.contains("+++ b/.prettierrc.json\n"));
        assert!(diff.contains("@@ -0,0 +1,1 @@\n"));
        assert!(diff.contains("+{}\n"));
    }

    #[test]
    fn render_removed_text_diff_uses_dev_null_new_path() {
        let diff = render_removed_text_diff(Path::new(".prettierignore"), "dist/\n");

        assert!(diff.contains("--- a/.prettierignore\n"));
        assert!(diff.contains("+++ /dev/null\n"));
        assert!(diff.contains("@@ -1,1 +0,0 @@\n"));
        assert!(diff.contains("-dist/\n"));
    }
}
