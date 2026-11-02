//! Errors as query results.
//!
//! A memoized value must be comparable and cheap to share, and the error types of the layers
//! below (`ConfigError`, `ProviderError`, `FileFormatError`) are neither `Clone` nor `Eq`. A
//! query that can fail therefore returns `Result<T, Failure>`: the failure is a value that is
//! memoized, compared and backdated like any other, never a panic.

use std::error::Error;
use std::fmt;

/// Which query failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// The config file does not parse or validate.
    Config,
    /// The config's format options are invalid for this file's language.
    Options,
    /// No provider handles the file. Callers ask [`crate::Database::supports`] first.
    Unsupported,
    /// The provider could not parse the file: for C# the worker is missing, crashed or refused.
    Parse,
    /// The file has syntax errors, formatting failed, or the output failed the safety check.
    Format,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    pub stage: Stage,
    /// The error and its causes joined with `": "`, as `anyhow`'s alternate display prints them.
    pub message: String,
}

impl Failure {
    pub(crate) fn new(stage: Stage, error: &dyn Error) -> Self {
        let mut message = error.to_string();
        let mut source = error.source();
        while let Some(cause) = source {
            message.push_str(": ");
            message.push_str(&cause.to_string());
            source = cause.source();
        }
        Failure { stage, message }
    }

    pub(crate) fn message(stage: Stage, message: impl Into<String>) -> Self {
        Failure {
            stage,
            message: message.into(),
        }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for Failure {}
