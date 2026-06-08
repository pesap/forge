use crate::blueprint::template_engine;

pub fn render_line_ending_policy() -> String {
    format!(
        "{}\n",
        template_engine::render_template("shared/gitattributes.j2", ())
    )
}

#[cfg(test)]
mod tests {
    use crate::blueprint::gitattributes::render_line_ending_policy;

    #[test]
    fn line_ending_policy_preserves_windows_command_scripts_as_crlf() {
        let policy = render_line_ending_policy();

        assert!(policy.contains("* text=auto eol=lf"));
        assert!(policy.contains("*.bat text eol=crlf"));
        assert!(policy.contains("*.cmd text eol=crlf"));
    }
}
