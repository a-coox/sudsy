//! Language packs: a tree-sitter grammar compiled as a shared library, installed per user and
//! loaded on first use (`docs/architecture/language-packs.md`).
//!
//! The binary embeds a manifest (`language file sha256 size` per line) and refuses a pack whose
//! hash differs. A build with the `static-grammars` feature of the language crates links the
//! grammars in and never loads a pack; the installed set then only records removed languages.

mod install;
mod load;
mod state;

pub use install::{download, install_from, remove};
pub use load::load;
pub use state::{cache_dir, is_removed, set_removed};
pub use tree_sitter::Language;

/// The Sudsy version packs are locked to.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const MANIFEST: &str = include_str!(concat!(env!("OUT_DIR"), "/manifest.txt"));
/// Where the release assets of the running version live; the one place that names the host.
const RELEASE_URL: &str = "https://github.com/a-coox/sudsy/releases/download/v";

#[derive(Debug, thiserror::Error)]
pub enum PackError {
    #[error("this build has no `{0}` language pack")]
    NoPack(String),
    #[error("the `{0}` language pack is not installed")]
    NotInstalled(String),
    #[error("{file}: SHA-256 {found} does not match the manifest ({expected})")]
    Hash {
        file: String,
        found: String,
        expected: String,
    },
    #[error("{0}: tree-sitter ABI {1} is outside {2}..={3} supported by this build")]
    Abi(String, usize, usize, usize),
    #[error("{0}: {1}")]
    Load(String, String),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("download failed: {0}")]
    Download(String),
}

/// One pack of the embedded manifest.
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub language: &'static str,
    pub file: &'static str,
    pub sha256: &'static str,
    pub size: u64,
}

impl Entry {
    pub fn url(&self) -> String {
        format!("{RELEASE_URL}{VERSION}/{}", self.file)
    }

    pub fn path(&self) -> std::path::PathBuf {
        cache_dir().join(self.file)
    }

    pub fn is_installed(&self) -> bool {
        self.path().is_file()
    }
}

/// The packs this build can load.
pub fn manifest() -> impl Iterator<Item = Entry> {
    MANIFEST.lines().filter_map(|line| {
        let mut f = line.split_whitespace();
        Some(Entry {
            language: f.next()?,
            file: f.next()?,
            sha256: f.next()?,
            size: f.next()?.parse().ok()?,
        })
    })
}

pub fn entry(language: &str) -> Result<Entry, PackError> {
    manifest()
        .find(|e| e.language == language)
        .ok_or_else(|| PackError::NoPack(language.into()))
}

/// The tree-sitter language of a static or pack grammar. `$suffix` names the pack's symbol
/// (`""` for the language, `"_inline"` for the second grammar of Markdown).
#[macro_export]
macro_rules! language {
    ($name:literal, $static:path) => {
        $crate::language!($name, "", $static)
    };
    ($name:literal, $suffix:literal, $static:path) => {{
        #[cfg(feature = "static-grammars")]
        {
            $crate::Language::from($static)
        }
        #[cfg(not(feature = "static-grammars"))]
        {
            $crate::load($name, $suffix).unwrap_or_else(|e| panic!("{e}"))
        }
    }};
}
