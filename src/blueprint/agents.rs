use std::path::PathBuf;

use crate::blueprint::files::{GeneratedFile, GeneratedFiles};
use crate::blueprint::template_engine;

pub fn render_agent_instructions() -> String {
    template_engine::render_template("shared/agents.md.j2", ())
}

pub fn render_agent_files() -> GeneratedFiles {
    let mut files = GeneratedFiles::new();
    files.insert(
        PathBuf::from("AGENTS.md"),
        GeneratedFile::text(render_agent_instructions()),
    );
    files.insert(
        PathBuf::from("CLAUDE.md"),
        GeneratedFile::symlink(PathBuf::from("AGENTS.md")),
    );
    files
}

#[cfg(test)]
mod tests {
    use crate::blueprint::agents::{render_agent_files, render_agent_instructions};
    use std::path::{Path, PathBuf};

    #[test]
    fn agent_instructions_include_shared_safety_guidance() {
        let instructions = render_agent_instructions();

        assert!(instructions.contains("# AGENTS.md"));
        assert!(instructions.contains("Use red-green-refactor for features and bug fixes"));
        assert!(instructions.contains("Keep infrastructure scripts and CI deterministic"));
        assert!(instructions.contains("Preserve user-authored source and configuration"));
        assert!(instructions.contains("selected blueprint/options"));
        assert!(instructions.contains("infrastructure:"));
        assert!(!instructions.contains("files recorded in `[tool.forge]` metadata"));
    }

    #[test]
    fn agent_instructions_include_requested_rules() {
        let instructions = render_agent_instructions();

        assert!(instructions.contains("Verify user claims against files, tests, or docs"));
        assert!(instructions.contains("Make surgical edits"));
    }

    #[test]
    fn agent_files_include_shared_instructions_and_claude_symlink() {
        let files = render_agent_files();

        assert!(
            files
                .get(&PathBuf::from("AGENTS.md"))
                .and_then(|file| file.as_text())
                .is_some_and(|content| content.contains("# AGENTS"))
        );
        assert_eq!(
            files
                .get(&PathBuf::from("CLAUDE.md"))
                .and_then(|file| file.symlink_target()),
            Some(Path::new("AGENTS.md"))
        );
    }
}
