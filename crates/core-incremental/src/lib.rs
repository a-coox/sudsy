//! The incremental query engine (plan.md "Incremental engine"), built on `salsa`.
//!
//! Core computations are queries memoized by what they read, so an edit recomputes only what
//! depends on it. Today's inputs are file text ([`File`]) and the config files
//! ([`ConfigFile`]); today's queries are in [`queries`]: `config`, `selection`, `options`,
//! `parse_settings`, `parse` and `format`. A query that fails returns the failure as a value
//! ([`Failure`]).
//!
//! Backdating: when a query re-executes to an equal value, its dependents are not re-run.
//! Equality of every value in the graph is structural: `GreenNode` compares its tree, config and
//! options compare their contents, and failures compare their messages.

mod db;
mod failure;
mod inputs;
mod queries;

pub use db::{Database, Db, EventHook, Providers};
pub use failure::{Failure, Stage};
pub use inputs::{ConfigFile, File};
pub use queries::{
    FileOptions, Parsed, Selection, config, format, options, parse, parse_settings, selection,
};
