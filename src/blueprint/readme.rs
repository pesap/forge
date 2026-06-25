pub fn automated_update_section() -> &'static str {
    include_str!("templates/shared/automated-update-section.md.j2")
}

#[cfg(test)]
mod tests {
    use crate::blueprint::readme::automated_update_section;

    #[test]
    fn automated_update_section_uses_forge_managed_language() {
        let section = automated_update_section();

        assert!(section.contains("Infrastructure Sync"));
        assert!(section.contains("Forge-managed infrastructure changes"));
        assert!(section.contains("forge sync --path . --yes"));
        assert!(section.contains("forge sync --path . --dry-run"));
        assert!(section.contains("forge sync --path . --check"));
        assert!(section.contains("uv lock"));
        assert!(section.contains("runs `uv lock` only when needed"));
        assert!(section.contains("[tool.forge]"));
        assert!(section.contains("does not write a separate status file"));
        assert!(!section.contains("template-managed"));
        assert!(!section.contains("infra-only"));
    }
}
