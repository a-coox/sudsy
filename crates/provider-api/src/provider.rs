use core_config::OptionSchema;
use core_format::NodeFormatter;
use core_rules::{DEFAULT_HOOKS, Rule, RuleHooks};
use core_semantics::{BehaviorSummaries, CompilerDiagnostic, FileSemantics, Layers};
use core_syntax::{Embedded, Grammar, LanguageId, SyntaxKind, SyntaxNode};
use std::path::{Path, PathBuf};

/// What a language provider can do. So far: parsing, formatting, format options and rules. Later
/// milestones add `semantic`, `flow` and `summaries` together with the types they need.
pub trait LanguageProvider: Send + Sync {
    fn id(&self) -> LanguageId;

    /// The language's name (`csharp`), known without loading the grammar.
    fn language(&self) -> &str;

    /// The crate that implements the language (`env!("CARGO_PKG_NAME")` there): `cargo xtask
    /// affected` maps a changed crate to the languages it benchmarks. Empty: unknown, always benched.
    fn crate_name(&self) -> &'static str {
        ""
    }

    /// Kinds, fields and category tags; parsed on the first call, so ask only when needed.
    fn grammar(&self) -> &Grammar;

    /// Extensions, known file names and dialects.
    fn file_matchers(&self) -> &[FileMatcher];

    /// Capability layers the provider answers (empty for syntax-only providers).
    fn capabilities(&self) -> Capabilities;

    /// Shared tree plus parse errors. Unlike a plain value this can fail: an out-of-process
    /// provider may be unavailable.
    fn parse(&self, file: &SourceFile) -> Result<ParseResult, ProviderError>;

    /// Per-node formatting code and hooks; tags cover the rest.
    fn formatter(&self) -> &dyn NodeFormatter;

    /// The format options this provider understands.
    fn option_schema(&self) -> &OptionSchema;

    /// The language's own rules; the rules every language shares are `core_rules::shared::RULES`.
    /// A provider that reads its built-in pattern files on the first call lets a run that never
    /// lints the language skip them.
    fn rules(&self) -> &'static [&'static dyn Rule] {
        &[]
    }

    /// [`rules`](Self::rules) without the built-in pattern files, which cannot lay out: what
    /// `format` consults, so that formatting reads no rule file.
    fn rust_rules(&self) -> &'static [&'static dyn Rule] {
        self.rules()
    }

    /// What the shared rules ask this language about its keys and scalars (see
    /// [`core_rules::RuleHooks`]); the defaults suit a language with nothing special.
    fn rule_hooks(&self) -> &dyn RuleHooks {
        &DEFAULT_HOOKS
    }

    /// The code of other languages in the file (a Dockerfile's `RUN`, a Markdown fence).
    fn embedded(&self, _root: &SyntaxNode, _text: &str) -> Vec<Embedded> {
        Vec::new()
    }

    /// Whether another language's file may embed this one: not a language with a worker process,
    /// which would start for a snippet.
    fn embeddable(&self) -> bool {
        true
    }

    /// The node kinds that are bodies (methods, lambdas, local functions): body-scope rules are
    /// called once for each.
    fn body_kinds(&self) -> &'static [SyntaxKind] {
        &[]
    }

    /// What the language's own compiler reports for the file under the given diagnostic ids. Rules
    /// do not use it; the gate that compares the flow analyses with the compiler does.
    fn compiler_diagnostics(
        &self,
        _file: &SourceFile,
        _ids: &[&str],
    ) -> Result<Vec<CompilerDiagnostic>, ProviderError> {
        Ok(Vec::new())
    }

    /// The provider's built-in behavior summaries (its standard library); the user's `semantics:`
    /// config is added to them.
    fn summaries(&self) -> BehaviorSummaries {
        BehaviorSummaries::default()
    }

    /// Starts the provider's background processes ahead of the first request (long-running
    /// servers only; a one-shot CLI run starts them on demand). Providers without any ignore it.
    fn prestart(&self) {}

    /// Tells a provider with a semantic model which files make up the project (directories,
    /// project or solution files). Providers without one ignore it.
    fn open_workspace(&self, _paths: &[PathBuf]) {}

    /// The answers of the semantic model for the file's current text, for the layers asked
    /// (`None` for providers without one, or without the layer). `project` adds the references
    /// that project-scope rules need.
    fn semantics(
        &self,
        _file: &SourceFile,
        _layers: Layers,
    ) -> Result<Option<FileSemantics>, ProviderError> {
        Ok(None)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FilePattern {
    /// Extension without the dot, e.g. `cs`.
    Extension(String),
    /// Exact file name, e.g. `tsconfig.json`; a trailing `*` makes it a prefix (`Dockerfile.*`).
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
            FilePattern::FileName(n) => {
                path.file_name().is_some_and(|x| match n.strip_suffix('*') {
                    Some(prefix) => x.to_string_lossy().starts_with(prefix),
                    None => x == n.as_str(),
                })
            }
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

/// The capability layers a provider answers.
pub type Capabilities = Layers;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceFile {
    pub id: String,
    pub version: u64,
    /// Text without a leading byte-order mark.
    pub text: String,
    pub had_bom: bool,
    /// Provider-specific, e.g. a C# language version.
    pub language_version: Option<String>,
    /// Conditional compilation symbols (C#); empty for none.
    pub preprocessor_symbols: Vec<String>,
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
            preprocessor_symbols: Vec::new(),
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
            preprocessor_symbols: Vec::new(),
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

/// What the user is told when the .NET runtime is missing (editor popup text; the CLI appends
/// where to get it).
pub const RUNTIME_MISSING: &str =
    "Sudsy needs the .NET 10 runtime to format and lint C#. JSON and YAML keep working.";
/// Where to download the runtime.
pub const RUNTIME_URL: &str = "https://dotnet.microsoft.com/download/dotnet/10.0";

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    /// The worker cannot be used any more in this process (not installed, crashed twice in a row, refused).
    #[error("{0}")]
    Unavailable(String),
    /// The runtime the worker needs is not installed; the string says what was found missing.
    #[error("the worker's runtime is missing: {0}")]
    RuntimeMissing(String),
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
