//! `SyntaxKind` namespacing.
//!
//! A kind is a `u32` laid out as
//!
//! ```text
//!  31..24  23     22..0
//! +-------+-----+----------+
//! | lang  | tok |  index   |
//! +-------+-----+----------+
//! ```
//!
//! * `lang`: [`LanguageId`]. `0` is the core namespace shared by all languages
//!   ([`SyntaxKind::ERROR`]); `1` JSON, `2` C#, `3` YAML.
//! * `tok`: set for tokens (including trivia tokens), clear for nodes. Because
//!   it is part of the kind, the wire encoding needs no extra per-element flag.
//! * `index`: position among the language's nodes or tokens, in grammar
//!   declaration order (so reordering a grammar changes ids and its hash).
//!
//! Each language therefore has up to 8,388,608 node kinds and as many token kinds, and there are 255 languages.

/// Identifies a language namespace (8 bits).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LanguageId(u8);

impl LanguageId {
    pub const CORE: LanguageId = LanguageId(0);
    pub const JSON: LanguageId = LanguageId(1);
    pub const CSHARP: LanguageId = LanguageId(2);
    pub const YAML: LanguageId = LanguageId(3);
    /// Largest id that fits the 8-bit namespace.
    pub const MAX: u8 = u8::MAX;

    pub const fn new(id: u8) -> Self {
        LanguageId(id)
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

/// A node or token kind. See the module docs for the bit layout.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SyntaxKind(pub u32);

impl SyntaxKind {
    const LANG_SHIFT: u32 = 24;
    const TOKEN_BIT: u32 = 1 << 23;
    /// Largest `index` (exclusive) per language and per node/token class.
    pub const MAX_INDEX: u32 = 1 << 23;

    /// A subtree the parser could not make sense of. Every token stays in the tree.
    pub const ERROR: SyntaxKind = SyntaxKind::new(LanguageId::CORE, false, 0);

    pub const fn new(lang: LanguageId, token: bool, index: u32) -> Self {
        assert!(index < Self::MAX_INDEX);
        let token_bit = if token { Self::TOKEN_BIT } else { 0 };
        SyntaxKind(((lang.0 as u32) << Self::LANG_SHIFT) | token_bit | index)
    }

    pub const fn lang(self) -> LanguageId {
        LanguageId((self.0 >> Self::LANG_SHIFT) as u8)
    }

    pub const fn is_token(self) -> bool {
        self.0 & Self::TOKEN_BIT != 0
    }

    pub const fn index(self) -> u32 {
        self.0 & (Self::MAX_INDEX - 1)
    }
}

impl std::fmt::Debug for SyntaxKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}{}",
            self.lang().get(),
            if self.is_token() { "t" } else { "n" },
            self.index()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_round_trips() {
        let k = SyntaxKind::new(LanguageId::CSHARP, true, 1000);
        assert_eq!(k.lang(), LanguageId::CSHARP);
        assert!(k.is_token());
        assert_eq!(k.index(), 1000);
        assert_eq!(k.0, (2 << 24) | (1 << 23) | 1000);
    }

    #[test]
    fn core_kinds_are_nodes_in_the_core_namespace() {
        assert_eq!(SyntaxKind::ERROR.lang(), LanguageId::CORE);
        assert!(!SyntaxKind::ERROR.is_token());
    }
}
