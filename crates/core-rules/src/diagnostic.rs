//! The diagnostics model every rule source reports into (plan.md "Diagnostics and fixes").

use core_config::Level;
use core_syntax::{TextRange, TextSize};

/// Rule categories (plan.md "Rule trait"), in fix priority order: when two fixes overlap, the
/// one of the earlier category wins.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Category {
    Correctness,
    Suspicious,
    Style,
    Cleanup,
    Performance,
}

impl Category {
    pub fn as_str(self) -> &'static str {
        match self {
            Category::Correctness => "correctness",
            Category::Suspicious => "suspicious",
            Category::Style => "style",
            Category::Cleanup => "cleanup",
            Category::Performance => "performance",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Warning,
    Error,
}

impl Severity {
    /// `None` for `Level::Off`: a rule that is off reports nothing.
    pub fn of(level: Level) -> Option<Severity> {
        match level {
            Level::Off => None,
            Level::Warn => Some(Severity::Warning),
            Level::Error => Some(Severity::Error),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Warning => "warning",
            Severity::Error => "error",
        }
    }
}

/// How far a fix may be trusted (plan.md "Diagnostics and fixes"). Ordered from least to most
/// trusted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FixKind {
    /// Editor quick fix only; `fix` never applies it.
    Suggestion,
    /// Applied by `fix --unsafe` and by an editor quick fix.
    Unsafe,
    /// Applied by `fix`.
    Safe,
}

impl FixKind {
    pub fn as_str(self) -> &'static str {
        match self {
            FixKind::Safe => "safe",
            FixKind::Unsafe => "unsafe",
            FixKind::Suggestion => "suggestion",
        }
    }

    /// Does `fix` (with or without `--unsafe`) apply fixes of this kind?
    pub fn applies(self, allow_unsafe: bool) -> bool {
        match self {
            FixKind::Safe => true,
            FixKind::Unsafe => allow_unsafe,
            FixKind::Suggestion => false,
        }
    }
}

/// Replaces `range` of the source with `text`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edit {
    pub range: TextRange,
    pub text: String,
}

impl Edit {
    pub fn replace(range: TextRange, text: impl Into<String>) -> Edit {
        Edit {
            range,
            text: text.into(),
        }
    }

    pub fn delete(range: TextRange) -> Edit {
        Edit::replace(range, "")
    }

    pub fn insert(at: TextSize, text: impl Into<String>) -> Edit {
        Edit::replace(TextRange::empty(at), text)
    }
}

/// A titled set of edits that are applied together or not at all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fix {
    pub title: String,
    pub kind: FixKind,
    pub edits: Vec<Edit>,
}

impl Fix {
    pub fn new(title: impl Into<String>, kind: FixKind, edits: Vec<Edit>) -> Fix {
        Fix {
            title: title.into(),
            kind,
            edits,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Related {
    pub range: TextRange,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    /// `no-empty-catch`, or `plugin/rule`; [`SYNTAX_ERROR`] for a file that does not parse.
    pub rule: String,
    pub category: Category,
    pub severity: Severity,
    pub message: String,
    pub range: TextRange,
    pub related: Vec<Related>,
    pub fixes: Vec<Fix>,
}

/// The id under which a parse error is reported; no rule has it.
pub const SYNTAX_ERROR: &str = "syntax-error";

/// The id under which check reports a file that ormat would change; no rule has it.
pub const FORMAT: &str = "format";

impl Diagnostic {
    /// Adds a related span (for example the first of two duplicate keys).
    pub fn related(&mut self, range: TextRange, message: impl Into<String>) -> &mut Self {
        self.related.push(Related {
            range,
            message: message.into(),
        });
        self
    }

    pub fn fix(&mut self, fix: Fix) -> &mut Self {
        self.fixes.push(fix);
        self
    }
}
