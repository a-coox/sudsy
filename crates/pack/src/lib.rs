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

export!("toml", sudsy_language, tree_sitter_toml_ng::LANGUAGE);
export!("xml", sudsy_language, tree_sitter_xml::LANGUAGE_XML);
export!("markdown", sudsy_language, tree_sitter_md::LANGUAGE);
export!(
    "markdown",
    sudsy_language_inline,
    tree_sitter_md::INLINE_LANGUAGE
);
export!(
    "editorconfig",
    sudsy_language,
    konomanoasa_tree_sitter_editorconfig::LANGUAGE
);
export!(
    "dockerfile",
    sudsy_language,
    tree_sitter_containerfile::LANGUAGE
);
export!("ini", sudsy_language, tree_sitter_ini::LANGUAGE);
export!("shell", sudsy_language, tree_sitter_bash::LANGUAGE);
export!(
    "powershell",
    sudsy_language,
    woodruffw_trough_tree_sitter_powershell::LANGUAGE
);
