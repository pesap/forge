use crate::blueprint::template_engine;

pub fn render_line_ending_policy() -> String {
    format!(
        "{}\n",
        template_engine::render_template("shared/gitattributes.j2", ())
    )
}
