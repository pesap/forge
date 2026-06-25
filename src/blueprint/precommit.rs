pub fn uv_lock_hook() -> &'static str {
    "  - repo: https://github.com/astral-sh/uv-pre-commit\n    rev: 0.9.4\n    hooks:\n      - id: uv-lock\n"
}

#[cfg(test)]
mod tests {
    use crate::blueprint::{precommit::uv_lock_hook, template_engine};

    #[test]
    fn shared_header_uses_portable_line_ending_hook() {
        let header = template_engine::render_template(
            "shared/_pre_commit_header.yaml.j2",
            serde_json::json!({"install_commit_msg_hook": false}),
        );

        assert!(header.contains("      - id: mixed-line-ending\n        args: [\"--fix=lf\"]\n"));
        assert!(!header.contains("(?i)\\.(bat|cmd)$"));
    }

    #[test]
    fn uv_lock_hook_uses_astral_uv_pre_commit() {
        let hook = uv_lock_hook();

        assert!(hook.contains("repo: https://github.com/astral-sh/uv-pre-commit"));
        assert!(hook.contains("id: uv-lock"));
    }
}
