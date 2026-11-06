//! The built-in grammars, parsed once from the checked-in `.grammar` files. The loaders and
//! `by_name` are generated from `grammars/*.grammar` by the build script.

use crate::{Grammar, LanguageId};
use std::sync::OnceLock;

fn load(cell: &'static OnceLock<Grammar>, src: &str) -> &'static Grammar {
    cell.get_or_init(|| Grammar::parse(src).expect("built-in grammar is valid"))
}

/// A built-in grammar known by name and id before it is parsed (a grammar costs millions of
/// instructions; a process that never touches the language must not pay for it).
#[derive(Clone, Copy, Debug)]
pub struct LazyGrammar {
    pub language: &'static str,
    pub id: LanguageId,
    load: fn() -> &'static Grammar,
}

impl LazyGrammar {
    pub const fn new(language: &'static str, id: u8, load: fn() -> &'static Grammar) -> Self {
        LazyGrammar {
            language,
            id: LanguageId::new(id),
            load,
        }
    }

    /// Parses the grammar on the first call.
    pub fn get(&self) -> &'static Grammar {
        (self.load)()
    }
}

include!(concat!(env!("OUT_DIR"), "/registry.rs"));
