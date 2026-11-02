//! The YAML provider: a hand-written lossless YAML 1.2 parser, the data model used by the
//! safety check, and the formatter.

#[rustfmt::skip]
pub mod generated;
pub mod model;
mod parser;
pub mod scalar;

pub use parser::{Parsed, parse};
