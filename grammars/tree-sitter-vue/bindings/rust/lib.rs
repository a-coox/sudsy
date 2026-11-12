//! Vue language support for tree-sitter.

use tree_sitter_language::LanguageFn;

unsafe extern "C" {
    fn tree_sitter_vue() -> *const ();
}

/// The tree-sitter [`LanguageFn`] for this grammar.
pub const LANGUAGE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_vue) };
