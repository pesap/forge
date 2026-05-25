pub fn uv_lock_hook() -> &'static str {
    "  - repo: https://github.com/astral-sh/uv-pre-commit\n    rev: 0.9.4\n    hooks:\n      - id: uv-lock\n"
}

pub fn forge_update_check_hook() -> &'static str {
    "      - id: forge-update-check\n        name: forge managed infrastructure check\n        entry: forge update --path . --check\n        language: system\n        pass_filenames: false\n"
}

#[cfg(test)]
mod tests {
    use crate::blueprint::precommit::{forge_update_check_hook, uv_lock_hook};

    #[test]
    fn uv_lock_hook_uses_astral_uv_pre_commit() {
        let hook = uv_lock_hook();

        assert!(hook.contains("repo: https://github.com/astral-sh/uv-pre-commit"));
        assert!(hook.contains("id: uv-lock"));
    }

    #[test]
    fn forge_update_check_hook_checks_managed_infrastructure_drift() {
        let hook = forge_update_check_hook();

        assert!(hook.contains("id: forge-update-check"));
        assert!(hook.contains("forge update --path . --check"));
        assert!(hook.contains("pass_filenames: false"));
    }
}
