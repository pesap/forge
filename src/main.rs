use clap::Parser;
use clap::error::ErrorKind;
use forge::cli::Cli;
use forge::errors::{ErrorCode, runtime_error_code};

fn main() {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => match error.kind() {
            ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => error.exit(),
            _ => {
                eprintln!("{error}");
                eprintln!("error_code: {}", parse_error_code(error.kind()).as_str());
                std::process::exit(2);
            }
        },
    };

    if let Err(error) = forge::run(cli) {
        eprintln!("error: {error}");
        for cause in error.chain().skip(1) {
            eprintln!("  caused by: {cause}");
        }
        eprintln!("error_code: {}", runtime_error_code(&error).as_str());
        std::process::exit(1);
    }
}

fn parse_error_code(kind: ErrorKind) -> ErrorCode {
    match kind {
        ErrorKind::InvalidValue
        | ErrorKind::UnknownArgument
        | ErrorKind::InvalidSubcommand
        | ErrorKind::NoEquals
        | ErrorKind::ValueValidation
        | ErrorKind::TooManyValues
        | ErrorKind::TooFewValues
        | ErrorKind::WrongNumberOfValues
        | ErrorKind::ArgumentConflict
        | ErrorKind::MissingRequiredArgument
        | ErrorKind::MissingSubcommand
        | ErrorKind::InvalidUtf8
        | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => ErrorCode::CliUsage,
        ErrorKind::Io | ErrorKind::Format => ErrorCode::Env,
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => ErrorCode::Internal,
        _ => ErrorCode::Internal,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_error_code;
    use clap::error::ErrorKind;
    use forge::errors::{ErrorCode, coded_error, runtime_error_code};

    #[test]
    fn parse_error_codes_cover_common_classes() {
        assert_eq!(
            parse_error_code(ErrorKind::UnknownArgument),
            ErrorCode::CliUsage
        );
        assert_eq!(parse_error_code(ErrorKind::Io), ErrorCode::Env);
    }

    #[test]
    fn runtime_error_code_classifies_messages() {
        assert_eq!(
            runtime_error_code(&anyhow::anyhow!("managed infrastructure has conflicts")),
            ErrorCode::Conflict
        );
        assert_eq!(
            runtime_error_code(&anyhow::anyhow!("option 'python-min' is not supported")),
            ErrorCode::Input
        );
        assert_eq!(
            runtime_error_code(&anyhow::anyhow!("failed to read repository path /tmp/repo")),
            ErrorCode::Env
        );
        assert_eq!(
            runtime_error_code(&anyhow::anyhow!(
                "missing Forge metadata at /tmp/repo/pyproject.toml"
            )),
            ErrorCode::Env
        );
        assert_eq!(
            runtime_error_code(&coded_error(ErrorCode::Input, "bad input")),
            ErrorCode::Input
        );
        assert_eq!(
            runtime_error_code(&anyhow::anyhow!("unexpected panic path")),
            ErrorCode::Internal
        );
    }
}
