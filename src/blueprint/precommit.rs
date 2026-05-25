pub fn uv_lock_hook() -> &'static str {
    "  - repo: https://github.com/astral-sh/uv-pre-commit\n    rev: 0.9.4\n    hooks:\n      - id: uv-lock\n"
}

#[cfg(test)]
mod tests {
    use crate::blueprint::precommit::uv_lock_hook;

    #[test]
    fn uv_lock_hook_uses_astral_uv_pre_commit() {
        let hook = uv_lock_hook();

        assert!(hook.contains("repo: https://github.com/astral-sh/uv-pre-commit"));
        assert!(hook.contains("id: uv-lock"));
    }
}
