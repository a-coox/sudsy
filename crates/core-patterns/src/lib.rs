//! Declarative pattern rules: ast-grep-style YAML rule
//! files whose patterns are snippets of the target language with metavariables.
//!
//! * [`metavar`]: `$X` and `$$$ARGS`, and how they are written into a snippet the language's own
//!   parser accepts.
//! * [`matcher`]: matching a compiled pattern against the shared tree.
//! * [`expr`]: the rule expression (`pattern`, `kind`, `regex`, `inside`, `has`, `any`, ...).
//! * [`file`]: the rule file format and the [`PatternRule`] it becomes. A pattern rule is a
//!   plain `core_rules::Rule`, so it runs through the same engine as every other rule.
//!
//! The matcher does not parse anything itself: whoever compiles a pattern supplies the parse
//! function of the pattern's language ([`SnippetParser`]).

pub mod expr;
pub mod file;
pub mod matcher;
pub mod metavar;

pub use file::{PatternError, PatternRule, load, load_lazy};
pub use matcher::{Captures, Pattern, SnippetParser};
