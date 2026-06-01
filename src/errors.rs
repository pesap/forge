use std::error::Error as StdError;
use std::fmt::{Display, Formatter};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    CliUsage,
    Input,
    Env,
    Conflict,
    Internal,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CliUsage => "FORGE_E_CLI_USAGE",
            Self::Input => "FORGE_E_INPUT",
            Self::Env => "FORGE_E_ENV",
            Self::Conflict => "FORGE_E_CONFLICT",
            Self::Internal => "FORGE_E_INTERNAL",
        }
    }
}

#[derive(Debug)]
pub struct CodedError {
    code: ErrorCode,
    message: String,
}

impl CodedError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> ErrorCode {
        self.code
    }
}

impl Display for CodedError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl StdError for CodedError {}

pub fn coded_error(code: ErrorCode, message: impl Into<String>) -> anyhow::Error {
    anyhow::Error::new(CodedError::new(code, message))
}

pub fn runtime_error_code(error: &anyhow::Error) -> ErrorCode {
    // Preserve explicit typed classification even when callers attach context layers.
    for cause in error.chain() {
        if let Some(coded) = cause.downcast_ref::<CodedError>() {
            return coded.code();
        }
    }

    let message = error.to_string().to_ascii_lowercase();
    if message.contains("conflict") || message.contains("out of date") {
        return ErrorCode::Conflict;
    }
    if message.contains("unsupported")
        || message.contains("invalid")
        || message.contains("missing required")
        || message.contains("requires --")
        || message.contains("is not supported")
    {
        return ErrorCode::Input;
    }
    if message.contains("missing forge metadata")
        || message.contains("missing [tool.forge] metadata")
        || message.contains("missing tool.forge.")
    {
        return ErrorCode::Env;
    }
    if message.contains("failed to")
        || message.contains("not installed")
        || message.contains("not authenticated")
        || message.contains("does not exist")
        || message.contains("is not a directory")
    {
        return ErrorCode::Env;
    }

    ErrorCode::Internal
}
