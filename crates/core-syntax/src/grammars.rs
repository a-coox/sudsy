//! The built-in grammars, parsed once from the checked-in `.grammar` files. The loaders and
//! `by_name` are generated from `grammars/*.grammar` by the build script.

use crate::Grammar;
use std::sync::OnceLock;

fn load(cell: &'static OnceLock<Grammar>, src: &str) -> &'static Grammar {
    cell.get_or_init(|| Grammar::parse(src).expect("built-in grammar is valid"))
}

include!(concat!(env!("OUT_DIR"), "/registry.rs"));
