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

pub fn yaml() -> &'static Grammar {
    static G: OnceLock<Grammar> = OnceLock::new();
    load(&G, include_str!("../../../grammars/yaml.grammar"))
}

/// The built-in grammar of `language` (its `language` line), if there is one.
pub fn by_name(language: &str) -> Option<&'static Grammar> {
    [json, csharp, yaml]
        .into_iter()
        .map(|g| g())
        .find(|g| g.language == language)
}
