pub fn string_literal(value: &str) -> String {
    toml::Value::String(value.to_string()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_literal_escapes_toml_without_data_loss() {
        let value = "Ada \"Countess\" Lovelace with \\ paths";
        let document = format!("name = {}", string_literal(value));
        let parsed: toml::Value = toml::from_str(&document).expect("TOML should parse");

        assert_eq!(parsed["name"].as_str(), Some(value));
    }
}
