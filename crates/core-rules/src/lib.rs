//! The rule engine: the rule trait with its four scopes,
//! the diagnostics and fix model, suppression comments, the fix loop, and the rules every
//! language shares. Languages contribute their own rules through the provider.
//!
//! * [`rule`]: the `Rule` trait, `RuleMeta` and the contexts.
//! * [`engine`]: the single traversal per file and the project phase.
//! * [`configure`]: which rules run on a file, with which options.
//! * [`fixes`]: fix selection and application, and the loop around them.
//! * [`layout`]: layout directives that rules attach for the formatter.
//! * `suppress`: `sudsy-disable-next-line`.

pub mod comments;
pub mod configure;
pub mod diagnostic;
pub mod engine;
pub mod extension;
pub mod fixes;
pub mod layout;
pub mod lines;
pub mod options;
pub mod project;
pub mod rule;
pub mod shared;
pub mod sorting;
pub mod suppress;
pub mod tree;

pub use configure::activate;
pub use diagnostic::{
    Category, Diagnostic, Edit, FORMAT, Fix, FixKind, Related, SYNTAX_ERROR, Severity,
};
pub use engine::{ActiveRule, FileReport, layers_needed, run_file, run_project, run_rules};
pub use extension::{ALL_LANGUAGES, ExtensionRule, Extensions};
pub use fixes::{Analysis, FixOutcome, fix_loop};
pub use layout::{Attach, LayoutRole, attached, run_layout};
pub use lines::LineIndex;
pub use options::{Check, Opt, OptionDecl, RuleOptions};
pub use project::{Fact, ProjectContext, ProjectFacts};
pub use rule::{BodyContext, FileContext, Layer, Rule, RuleContext, RuleMeta};
