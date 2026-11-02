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
            !(text.starts_with("/*") && text.ends_with("*/"))
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
