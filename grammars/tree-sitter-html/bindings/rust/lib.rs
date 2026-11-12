//! HTML language support for tree-sitter (a fork of tree-sitter-html, see FORK.md).

use tree_sitter_language::LanguageFn;

unsafe extern "C" {
    fn tree_sitter_html() -> *const ();
}

/// The tree-sitter [`LanguageFn`] for this grammar.
pub const LANGUAGE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_html) };
