//! Makefile language support for tree-sitter (a fork of tree-sitter-make, see FORK.md).

use tree_sitter_language::LanguageFn;

unsafe extern "C" {
    fn tree_sitter_make() -> *const ();
}

/// The tree-sitter [`LanguageFn`] for this grammar.
pub const LANGUAGE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_make) };
