pub fn uv_lock_hook() -> &'static str {
    "  - repo: https://github.com/astral-sh/uv-pre-commit\n    rev: 0.9.4\n    hooks:\n      - id: uv-lock\n"
}

#[cfg(test)]
mod tests {
    use crate::blueprint::{precommit::uv_lock_hook, template_engine};

    #[test]
    fn shared_header_aligns_line_ending_hooks_with_windows_script_policy() {
        let header = template_engine::render_template("shared/_pre_commit_header.yaml.j2", ());
        let lf_hook = "      - id: mixed-line-ending\n        name: mixed line ending (LF-normalized files)\n        args: [\"--fix=lf\"]\n        exclude: '(?i)\\.(bat|cmd)$'";
        let windows_script_hook = "      - id: mixed-line-ending\n        name: mixed line ending (Windows command scripts)\n        args: [\"--fix=crlf\"]\n        files: '(?i)\\.(bat|cmd)$'";

        assert!(header.contains(lf_hook));
        assert!(header.contains(windows_script_hook));
    }

    #[test]
    fn uv_lock_hook_uses_astral_uv_pre_commit() {
        let hook = uv_lock_hook();

        assert!(hook.contains("repo: https://github.com/astral-sh/uv-pre-commit"));
        assert!(hook.contains("id: uv-lock"));
    }
}
