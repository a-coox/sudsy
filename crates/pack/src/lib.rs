//! A language pack: the grammar of the one enabled feature behind the symbols `core-packs`
//! loads (`sudsy_language`, and `sudsy_language_inline` for Markdown's second grammar). The
//! grammar crate's own `tree_sitter_<lang>` symbol stays internal to the library.

macro_rules! export {
    ($feature:literal, $symbol:ident, $language:expr) => {
        #[cfg(feature = $feature)]
        #[unsafe(no_mangle)]
        pub extern "C" fn $symbol() -> *const () {
            // SAFETY: the grammar crate's function returns a pointer to a static `TSLanguage`.
            unsafe { $language.into_raw()() }
        }
    };
}

macro_rules! exports {
    ($($feature:literal, $symbol:ident, $language:expr;)*) => { $(export!($feature, $symbol, $language);)* };
}

exports! {
    // generated:exports
    "dockerfile", sudsy_language, tree_sitter_containerfile::LANGUAGE;
    "editorconfig", sudsy_language, konomanoasa_tree_sitter_editorconfig::LANGUAGE;
    "gitignore", sudsy_language, konomanoasa_tree_sitter_gitignore::LANGUAGE;
    "graphql", sudsy_language, tree_sitter_graphql::LANGUAGE;
    "hcl", sudsy_language, tree_sitter_hcl::LANGUAGE;
    "ini", sudsy_language, tree_sitter_ini::LANGUAGE;
    "makefile", sudsy_language, tree_sitter_make::LANGUAGE;
    "markdown", sudsy_language, tree_sitter_md::LANGUAGE;
    "markdown", sudsy_language_inline, tree_sitter_md::INLINE_LANGUAGE;
    "powershell", sudsy_language, woodruffw_trough_tree_sitter_powershell::LANGUAGE;
    "protobuf", sudsy_language, tree_sitter_proto::LANGUAGE;
    "shell", sudsy_language, tree_sitter_bash::LANGUAGE;
    "toml", sudsy_language, tree_sitter_toml_ng::LANGUAGE;
    "xml", sudsy_language, tree_sitter_xml::LANGUAGE_XML;
    // /generated
}
