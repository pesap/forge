pub fn automated_update_section() -> &'static str {
    "## Automated Forge Syncs\n\nThis repository includes `.github/workflows/forge-sync.yaml`, which runs weekly and opens a PR with Forge-managed infrastructure syncs from `forge sync --path .`. Forge reads the project configuration from `[tool.forge]` in `pyproject.toml` and does not use a separate status file.\n\nUse these commands for local maintenance:\n\n```bash\nforge sync --path . --dry-run\nforge sync --path . --check\nforge sync --path .\nuv lock\n```\n\nRun `uv lock` when Forge reports it as a next step after changing managed options or other `pyproject.toml` metadata.\n\n"
}

#[cfg(test)]
mod tests {
    use crate::blueprint::readme::automated_update_section;

    #[test]
    fn automated_update_section_uses_forge_managed_language() {
        let section = automated_update_section();

        assert!(section.contains("Automated Forge Syncs"));
        assert!(section.contains("Forge-managed infrastructure syncs"));
        assert!(section.contains("forge sync --path ."));
        assert!(section.contains("forge sync --path . --dry-run"));
        assert!(section.contains("forge sync --path . --check"));
        assert!(section.contains("uv lock"));
        assert!(section.contains("[tool.forge]"));
        assert!(section.contains("does not use a separate status file"));
        assert!(!section.contains("template-managed"));
        assert!(!section.contains("infra-only"));
    }
}
