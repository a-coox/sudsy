use core_config::OptionSchema;
use core_format::NodeFormatter;
use core_syntax::{Grammar, LanguageId, SyntaxNode};
use std::path::Path;

/// What a language provider can do. So far: parsing, formatting and format options. Later
/// milestones add `presets`, `builtin_rules`, `semantic`, `flow` and `summaries` (plan.md
/// "Provider interface") together with the types they need.
pub trait LanguageProvider: Send + Sync {
    fn id(&self) -> LanguageId;

    /// Kinds, fields and category tags.
    fn grammar(&self) -> &Grammar;

    /// Extensions, known file names and dialects.
    fn file_matchers(&self) -> &[FileMatcher];

    /// Capability layers the provider answers (empty for syntax-only providers).
    fn capabilities(&self) -> Capabilities;

    /// Shared tree plus parse errors. Unlike plan.md's sketch this can fail: an out-of-process
    /// provider may be unavailable.
    fn parse(&self, file: &SourceFile) -> Result<ParseResult, ProviderError>;

    /// Per-node formatting code and hooks; tags cover the rest.
    fn formatter(&self) -> &dyn NodeFormatter;

    /// The format options this provider understands.
    fn option_schema(&self) -> &OptionSchema;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FilePattern {
    /// Extension without the dot, e.g. `cs`.
    Extension(String),
    /// Exact file name, e.g. `tsconfig.json`.
    FileName(String),
    /// Files with this extension directly inside a directory of this name, e.g. `.vscode/*.json`.
    InDirectory { dir: String, extension: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileMatcher {
    pub pattern: FilePattern,
    /// Dialect selected by this matcher (e.g. `jsonc`), if the language has several.
    pub dialect: Option<String>,
}

impl FileMatcher {
    pub fn extension(ext: &str) -> Self {
        FileMatcher {
            pattern: FilePattern::Extension(ext.into()),
            dialect: None,
        }
    }

    pub fn matches(&self, path: &Path) -> bool {
        let has_extension = |e: &str| path.extension().is_some_and(|x| x.eq_ignore_ascii_case(e));
        match &self.pattern {
            FilePattern::Extension(e) => has_extension(e),
            FilePattern::FileName(n) => path.file_name().is_some_and(|x| x == n.as_str()),
            FilePattern::InDirectory { dir, extension } => {
                has_extension(extension)
                    && path
                        .parent()
                        .and_then(Path::file_name)
                        .is_some_and(|d| d == dir.as_str())
            }
        }
    }

    /// When several matchers match a file, the most specific one wins.
    pub fn specificity(&self) -> u8 {
        match self.pattern {
            FilePattern::Extension(_) => 1,
            FilePattern::InDirectory { .. } => 2,
            FilePattern::FileName(_) => 3,
        }
    }
}

/// The analysis layers of plan.md "Capability layers".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Capabilities {
    pub binding: bool,
    pub types: bool,
    pub values: bool,
    pub flow: bool,
    pub project: bool,
}

impl Capabilities {
    pub const NONE: Capabilities = Capabilities {
        binding: false,
        types: false,
        values: false,
        flow: false,
        project: false,
    };

    /// Parses the names used in the worker's `Hello`; unknown names are ignored.
    pub fn from_names<S: AsRef<str>>(names: &[S]) -> Self {
        let has = |n: &str| names.iter().any(|x| x.as_ref() == n);
        Capabilities {
            binding: has("binding"),
            types: has("types"),
            values: has("values"),
            flow: has("flow"),
            project: has("project"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceFile {
    pub id: String,
    pub version: u64,
    /// Text without a leading byte-order mark.
    pub text: String,
    pub had_bom: bool,
    /// Provider-specific, e.g. a C# language version.
    pub language_version: Option<String>,
}

#[derive(Debug, thiserror::Error)]
#[error("{id} is not valid UTF-8")]
pub struct SourceError {
    pub id: String,
}

impl SourceFile {
    pub fn new(id: impl Into<String>, version: u64, text: impl Into<String>) -> Self {
        SourceFile {
            id: id.into(),
            version,
            text: text.into(),
            had_bom: false,
            language_version: None,
        }
    }

    /// Decodes UTF-8 and strips a leading BOM (remembered in `had_bom` so writers can restore it).
    pub fn from_bytes(
        id: impl Into<String>,
        version: u64,
        bytes: Vec<u8>,
    ) -> Result<Self, SourceError> {
        let id = id.into();
        let mut text = String::from_utf8(bytes).map_err(|_| SourceError { id: id.clone() })?;
        let had_bom = text.starts_with('\u{feff}');
        if had_bom {
            text.remove(0);
        }
        Ok(SourceFile {
            id,
            version,
            text,
            had_bom,
            language_version: None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    /// Byte range into the source text.
    pub start: u32,
    pub len: u32,
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug)]
pub struct ParseResult {
    pub tree: SyntaxNode,
    pub errors: Vec<ParseError>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    /// The worker cannot be used for the rest of this run (not installed, crashed twice, refused).
    #[error("{0}")]
    Unavailable(String),
    #[error("worker error {code}: {message}")]
    Worker { code: String, message: String },
    #[error("invalid tree from worker: {0}")]
    InvalidTree(#[from] core_syntax::wire::WireError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bom_is_stripped_and_remembered() {
        let f = SourceFile::from_bytes("a", 1, b"\xef\xbb\xbfclass A {}".to_vec()).unwrap();
        assert!(f.had_bom);
        assert_eq!(f.text, "class A {}");
        assert!(SourceFile::from_bytes("a", 1, vec![0xff]).is_err());
    }

    #[test]
    fn directory_matcher() {
        let m = FileMatcher {
            pattern: FilePattern::InDirectory {
                dir: ".vscode".into(),
                extension: "json".into(),
            },
            dialect: None,
        };
        assert!(m.matches(Path::new("repo/.vscode/settings.json")));
        assert!(!m.matches(Path::new("repo/settings.json")));
        assert!(!m.matches(Path::new("repo/.vscode/a/settings.json")));
    }

    #[test]
    fn matchers() {
        let m = FileMatcher::extension("cs");
        assert!(m.matches(Path::new("src/A.CS")));
        assert!(!m.matches(Path::new("src/A.csx")));
    }
}
