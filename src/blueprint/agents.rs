use std::path::PathBuf;

use crate::blueprint::files::{GeneratedFile, GeneratedFiles};

pub fn render_agent_instructions(extra_guidance: &[&str]) -> String {
    let mut instructions = vec![
        "MUST FOLLOW TDD FOR FEATURES AND BUG FIXES.",
        "MUST KEEP INFRASTRUCTURE SCRIPTS AND CI DETERMINISTIC.",
        "MUST PRESERVE USER-AUTHORED PROJECT CODE DURING MANAGED UPDATES.",
    ];
    instructions.extend(extra_guidance);

    let bullets = instructions
        .into_iter()
        .map(|instruction| format!("- {instruction}\n"))
        .collect::<String>();

    format!("# AGENTS\n\nGuidance for coding agents in this repository.\n\n{bullets}")
}

pub fn render_agent_files(extra_guidance: &[&str]) -> GeneratedFiles {
    let mut files = GeneratedFiles::new();
    files.insert(
        PathBuf::from("AGENTS.md"),
        GeneratedFile::text(render_agent_instructions(extra_guidance)),
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
        let instructions = render_agent_instructions(&[]);

        assert!(instructions.contains("# AGENTS"));
        assert!(instructions.contains("MUST FOLLOW TDD"));
        assert!(instructions.contains("MUST KEEP INFRASTRUCTURE SCRIPTS AND CI DETERMINISTIC"));
        assert!(instructions.contains("MUST PRESERVE USER-AUTHORED PROJECT CODE"));
    }

    #[test]
    fn agent_instructions_include_blueprint_specific_guidance() {
        let instructions = render_agent_instructions(&["Run cargo test before handoff."]);

        assert!(instructions.contains("Run cargo test before handoff."));
    }

    #[test]
    fn agent_files_include_shared_instructions_and_claude_symlink() {
        let files = render_agent_files(&[]);

        assert!(
            files
                .get(&PathBuf::from("AGENTS.md"))
                .and_then(|file| file.as_text())
                .is_some_and(|content| content.contains("Guidance for coding agents"))
        );
        assert_eq!(
            files
                .get(&PathBuf::from("CLAUDE.md"))
                .and_then(|file| file.symlink_target()),
            Some(Path::new("AGENTS.md"))
        );
    }
}
