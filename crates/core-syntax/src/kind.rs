//! `SyntaxKind` namespacing.
//!
//! A kind is a `u16` laid out as
//!
//! ```text
//!  15..12  11     10..0
//! +-------+-----+----------+
//! | lang  | tok |  index   |
//! +-------+-----+----------+
//! ```
//!
//! * `lang`: [`LanguageId`]. `0` is the core namespace shared by all languages
//!   ([`SyntaxKind::ERROR`], [`SyntaxKind::EMBEDDED`]); `1` JSON, `2` C#, `3` YAML.
//! * `tok`: set for tokens (including trivia tokens), clear for nodes. Because
//!   it is part of the kind, the wire encoding needs no extra per-element flag.
//! * `index`: position among the language's nodes or tokens, in grammar
//!   declaration order (so reordering a grammar changes ids and its hash).
//!
//! Each language therefore has up to 2048 node kinds and 2048 token kinds.

/// Identifies a language namespace (4 bits).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LanguageId(u8);

impl LanguageId {
    pub const CORE: LanguageId = LanguageId(0);
    pub const JSON: LanguageId = LanguageId(1);
    pub const CSHARP: LanguageId = LanguageId(2);
    pub const YAML: LanguageId = LanguageId(3);
    /// Largest id that fits the 4-bit namespace.
    pub const MAX: u8 = 15;

    pub const fn new(id: u8) -> Self {
        assert!(id <= Self::MAX);
        LanguageId(id)
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

/// A node or token kind. See the module docs for the bit layout.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SyntaxKind(pub u16);

impl SyntaxKind {
    const LANG_SHIFT: u16 = 12;
    const TOKEN_BIT: u16 = 1 << 11;
    /// Largest `index` (exclusive) per language and per node/token class.
    pub const MAX_INDEX: u16 = 1 << 11;

    /// A subtree the parser could not make sense of. Every token stays in the tree.
    pub const ERROR: SyntaxKind = SyntaxKind::new(LanguageId::CORE, false, 0);
    /// A node whose children are the root of a tree in another language.
    pub const EMBEDDED: SyntaxKind = SyntaxKind::new(LanguageId::CORE, false, 1);

    pub const fn new(lang: LanguageId, token: bool, index: u16) -> Self {
        assert!(index < Self::MAX_INDEX);
        let token_bit = if token { Self::TOKEN_BIT } else { 0 };
        SyntaxKind(((lang.0 as u16) << Self::LANG_SHIFT) | token_bit | index)
    }

    pub const fn lang(self) -> LanguageId {
        LanguageId((self.0 >> Self::LANG_SHIFT) as u8)
    }

    pub const fn is_token(self) -> bool {
        self.0 & Self::TOKEN_BIT != 0
    }

    pub const fn index(self) -> u16 {
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
        let k = SyntaxKind::new(LanguageId::CSHARP, true, 1234);
        assert_eq!(k.lang(), LanguageId::CSHARP);
        assert!(k.is_token());
        assert_eq!(k.index(), 1234);
        assert_eq!(k.0, 0x2000 | 0x800 | 1234);
    }

    #[test]
    fn core_kinds_are_nodes_in_the_core_namespace() {
        for k in [SyntaxKind::ERROR, SyntaxKind::EMBEDDED] {
            assert_eq!(k.lang(), LanguageId::CORE);
            assert!(!k.is_token());
        }
        assert_ne!(SyntaxKind::ERROR, SyntaxKind::EMBEDDED);
    }
}
