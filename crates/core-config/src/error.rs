use std::fmt;
use std::path::PathBuf;

/// A problem in a config file, with its position when known (1-based).
#[derive(Debug, thiserror::Error)]
pub struct ConfigError {
    pub path: Option<PathBuf>,
    pub line: Option<u64>,
    pub column: Option<u64>,
    pub message: String,
}

impl ConfigError {
    pub(crate) fn at(line: u64, column: u64, message: impl Into<String>) -> Self {
        ConfigError {
            path: None,
            line: Some(line),
            column: Some(column),
            message: message.into(),
        }
    }

    pub(crate) fn with_path(mut self, path: PathBuf) -> Self {
        self.path = Some(path);
        self
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(p) = &self.path {
            write!(f, "{}", p.display())?;
            if let (Some(l), Some(c)) = (self.line, self.column) {
                write!(f, ":{l}:{c}")?;
            }
            write!(f, ": ")?;
        } else if let (Some(l), Some(c)) = (self.line, self.column) {
            write!(f, "{l}:{c}: ")?;
        }
        write!(f, "{}", self.message)
    }
}
