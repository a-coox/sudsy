#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::OnceLock;

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Builds the C# worker once per test process and returns the assembly path.
pub fn worker_dll() -> PathBuf {
    static DLL: OnceLock<PathBuf> = OnceLock::new();
    DLL.get_or_init(|| {
        let project = repo_root().join("workers/csharp/Sudsy.Worker");
        let status = std::process::Command::new("dotnet")
            .args(["build", "-c", "Debug", "--nologo", "-v", "q"])
            .arg(&project)
            .status()
            .expect("dotnet is installed");
        assert!(status.success(), "dotnet build failed");
        project.join("bin/Debug/net10.0/Sudsy.Worker.dll")
    })
    .clone()
}

pub fn corpus_root() -> PathBuf {
    repo_root().join("tests/conformance/corpus")
}

/// Conditional compilation symbols for a corpus file. The corpora are parsed as one Windows x64
/// build would see them. Runtime's browser-only files select the browser target instead, because
/// they end in an #error directive for any other.
pub fn symbols_for(path: &std::path::Path) -> Vec<String> {
    let browser = path
        .file_name()
        .is_some_and(|n| n.to_string_lossy().contains(".Browser."));
    let symbols: &[&str] = if browser {
        &["TARGET_BROWSER", "TARGET_WASM", "TARGET_32BIT"]
    } else {
        &["TARGET_WINDOWS", "TARGET_AMD64", "TARGET_64BIT"]
    };
    symbols.iter().map(|s| s.to_string()).collect()
}

pub fn collect_cs(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            collect_cs(&path, out);
        } else if path.extension().is_some_and(|e| e == "cs") {
            out.push(path);
        }
    }
}

use core_syntax::{SyntaxKind, SyntaxNode, grammars, trivia};
use lang_csharp::generated::kind;

/// Zero-width tokens the grammar knows: every other token has text.
fn may_be_empty(k: SyntaxKind) -> bool {
    [
        kind::END_OF_FILE_TOKEN,
        kind::END_OF_DIRECTIVE_TOKEN,
        kind::END_OF_DOCUMENTATION_COMMENT_TOKEN,
        kind::OMITTED_TYPE_ARGUMENT_TOKEN,
        kind::OMITTED_ARRAY_SIZE_EXPRESSION_TOKEN,
    ]
    .contains(&k)
}

/// Every token has text of its kind, so nothing was filed under the wrong kind (skipped or
/// missing text hiding in a trivia token, for instance).
pub fn check_token_texts(tree: &SyntaxNode) -> Result<(), String> {
    let g = grammars::csharp();
    for t in tree
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
    {
        let (k, text) = (t.kind(), t.text());
        let bad = if k == kind::WHITESPACE_TRIVIA {
            !text
                .chars()
                .all(|c| c.is_whitespace() && !trivia::is_line_break(c))
        } else if k == kind::END_OF_LINE_TRIVIA {
            text.is_empty() || !text.chars().all(trivia::is_line_break)
        } else if k == kind::SINGLE_LINE_COMMENT_TRIVIA {
            !text.starts_with("//") || text.chars().any(trivia::is_line_break)
        } else if k == kind::MULTI_LINE_COMMENT_TRIVIA {
            !text.starts_with("/*")
        } else if g.is_trivia(k) {
            false
        } else {
            text.is_empty() && !may_be_empty(k)
        };
        if bad {
            return Err(format!(
                "{} {:?} has text {text:?}",
                g.name(k),
                t.text_range()
            ));
        }
    }
    Ok(())
}

use core_config::{OptionValue, ResolvedOptions};
use core_format::DirectiveTable;
use core_format::testing::Subject;
use core_syntax::Grammar;
use provider_api::{LanguageProvider, OutOfProcessProvider, SourceFile, format_file};

/// One provider (one worker process) for every test of the process.
pub fn shared_provider() -> &'static OutOfProcessProvider {
    static PROVIDER: OnceLock<OutOfProcessProvider> = OnceLock::new();
    PROVIDER.get_or_init(|| lang_csharp::provider(&worker_dll()))
}

/// C# text formatted with some options, for the shared checks.
pub struct CsSubject {
    options: Vec<(String, OptionValue)>,
    symbols: Vec<String>,
}

pub fn subject(options: &[(&str, OptionValue)]) -> CsSubject {
    CsSubject {
        options: options
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect(),
        symbols: Vec::new(),
    }
}

pub fn s(v: &str) -> OptionValue {
    OptionValue::String(v.into())
}

pub fn n(v: i64) -> OptionValue {
    OptionValue::Int(v)
}

impl CsSubject {
    pub fn with_symbols(mut self, symbols: &[&str]) -> Self {
        self.symbols = symbols.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn resolved(&self) -> ResolvedOptions {
        let schema = lang_csharp::option_schema();
        let mut options = schema.defaults();
        for (k, v) in &self.options {
            schema.apply(&mut options, k, v).unwrap();
        }
        options
    }

    fn file(&self, text: &str) -> SourceFile {
        let mut f = SourceFile::new("a.cs", 1, text);
        f.preprocessor_symbols = self.symbols.clone();
        f
    }
}

impl Subject for CsSubject {
    fn grammar(&self) -> &Grammar {
        shared_provider().grammar()
    }

    fn parse(&self, text: &str) -> Result<core_syntax::SyntaxNode, String> {
        let r = shared_provider()
            .parse(&self.file(text))
            .map_err(|e| e.to_string())?;
        match r.errors.first() {
            Some(e) => Err(format!("{} (byte {})", e.message, e.start)),
            None => Ok(r.tree),
        }
    }

    fn format(&self, text: &str) -> Result<String, String> {
        format_file(
            shared_provider(),
            &self.file(text),
            &self.resolved(),
            &DirectiveTable::default(),
        )
        .map_err(|e| e.to_string())
    }
}

/// Formats with every check of the shared helper; returns the output.
#[track_caller]
pub fn fmt(input: &str, options: &[(&str, OptionValue)]) -> String {
    core_format::testing::assert_formats(&subject(options), input)
}

pub struct Corpus {
    pub name: &'static str,
    pub dirs: &'static [&'static str],
}

pub const CORPORA: &[Corpus] = &[
    Corpus {
        name: "runtime",
        dirs: &["src/libraries/System.Private.CoreLib/src"],
    },
    Corpus {
        name: "roslyn",
        dirs: &[
            "src/Compilers/CSharp/Portable",
            "src/Compilers/Core/Portable",
            "src/Workspaces/Core/Portable",
        ],
    },
    Corpus {
        name: "aspnetcore",
        dirs: &[
            "src/Http",
            "src/Mvc/Mvc.Core",
            "src/Middleware",
            "src/Servers/Kestrel/Core/src",
            "src/Components/Components/src",
            "src/SignalR/common",
        ],
    },
];

