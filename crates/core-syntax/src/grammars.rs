//! The built-in grammars, parsed once from the checked-in `.grammar` files.

use crate::Grammar;
use std::sync::OnceLock;

fn load(cell: &'static OnceLock<Grammar>, src: &str) -> &'static Grammar {
    cell.get_or_init(|| Grammar::parse(src).expect("built-in grammar is valid"))
}

pub fn json() -> &'static Grammar {
    static G: OnceLock<Grammar> = OnceLock::new();
    load(&G, include_str!("../../../grammars/json.grammar"))
}

pub fn csharp() -> &'static Grammar {
    static G: OnceLock<Grammar> = OnceLock::new();
    load(&G, include_str!("../../../grammars/csharp.grammar"))
}
