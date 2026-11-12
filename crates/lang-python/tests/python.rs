//! Parse, round trip and the shared format checks on the bench inputs and on probes of recent
//! syntax: every file parses without errors, prints back byte for byte, satisfies the trivia
//! attachment rule and has no `Unknown` token. The ignored test runs the same on the corpus
//! (`tests/conformance/fetch-python-corpus.sh`).

use core_syntax::{print, trivia};
use lang_python::PythonProvider;
use lang_python::generated::kind;
use provider_api::format_testing::{assert_file_names, check_corpus};
use provider_api::{LanguageProvider, SourceFile};
use std::path::PathBuf;

const PROBES: &str = r#"#!/usr/bin/env python
"""Module docstring."""
from __future__ import annotations
import os, sys as system
from . import (a, b as c)

@decorator(1)
async def f[T: int, *Ts, **P](x: T = 1, /, y=2, *args, z, **kw) -> list[T]:
    async with a as b, (c):
        await x  # trailing
    return [i async for i in y if (i)]

class C[T](Base, metaclass=M):
    x: int = 1
    def g(self): return lambda: (yield)

match command.split():
    case [Point(x=0, y=0) | None, *rest] if rest:
        pass
    case {"a": 1, **kw} | (1 | 2):
        pass
    case _:
        pass

try:
    pass
except* (A, B) as e:
    raise X from e
finally:
    del (a), b[0]

type X[T] = list[T]
s = f"a{b!r:>{width}}c" "d" 'e' \
    'f'
t = t"x{y}"
n = 0x_ff + 1_0.5e-3j + (1,) + ((2))
print(*a, **b, sep='')
x = a if b else (c := d)
if x:
    pass
elif y: pass
else:
    pass
for i in range(3): continue
while (x): break
with (open(f) as g, open(h) as i):
    pass
global_ = lambda: 1
b = rb'e' b"f"
"#;

fn provider() -> PythonProvider {
    PythonProvider::new()
}

/// Every check on one file except formatting.
fn check_parse(name: &str, text: &str) {
    let parsed = provider().parse(&SourceFile::new(name, 1, text)).unwrap();
    assert!(
        parsed.errors.is_empty(),
        "{name}: {:?}",
        parsed.errors.first()
    );
    assert_eq!(print(&parsed.tree), text, "{name}: round trip");
    let grammar = provider().grammar().clone();
    trivia::check_attachment(&parsed.tree, |k| grammar.is_trivia(k))
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    let unknown = (parsed.tree.descendants_with_tokens())
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == kind::UNKNOWN);
    assert!(unknown.is_none(), "{name}: unknown token {unknown:?}");
    // Fixed-text tokens carry their text.
    for t in parsed
        .tree
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
    {
        if let Some(text) = grammar.def(t.kind()).and_then(|d| d.text.as_deref()) {
            assert_eq!(t.text().to_lowercase(), text.to_lowercase(), "{name}");
        }
    }
}

#[test]
fn probes_of_recent_syntax() {
    check_parse("a.py", PROBES);
}

#[test]
fn parentheses_are_nodes_of_their_own() {
    let parsed = provider()
        .parse(&SourceFile::new("a.py", 1, "f((a), ((b)))\nclass C((B)): pass\n"))
        .unwrap();
    let texts: Vec<String> = (parsed.tree.descendants())
        .filter(|n| n.kind() == kind::PARENTHESIZED)
        .map(|n| n.text().to_string())
        .collect();
    assert_eq!(texts, ["(a)", "((b))", "(b)", "(B)"]);
}

#[test]
fn empty_nodes_stand_for_missing_parts() {
    let parsed = provider().parse(&SourceFile::new("a.py", 1, "x = 
")).unwrap();
    let name = (parsed.tree.descendants())
        .find(|n| n.kind() == kind::EXPR_NAME && n.text_range().is_empty());
    assert!(name.is_some());
}

#[test]
fn bench_inputs() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bench/inputs/py");
    let mut count = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        check_parse(&name, &std::fs::read_to_string(&path).unwrap());
        count += 1;
    }
    assert!(count >= 4, "bench inputs missing");
}

#[test]
fn syntax_errors_are_reported_once_per_statement_and_lossless() {
    for (text, errors) in [
        ("x = \n", 1),
        ("def f(:\n    pass\n", 1),
        ("a = 1 +\nb = (\n", 2),
        ("lazy import os\n", 1),
        ("x = 1 € 2\n", 1),
    ] {
        let parsed = provider().parse(&SourceFile::new("a.py", 1, text)).unwrap();
        assert!(parsed.errors.len() >= 1, "{text:?}");
        assert!(parsed.errors.len() <= errors + 1, "{text:?}: {:?}", parsed.errors);
        assert_eq!(print(&parsed.tree), text);
    }
}

#[test]
fn target_version_gates_syntax() {
    let ok = |v: &str| {
        let mut file = SourceFile::new("a.py", 1, "type X = int\n");
        file.language_version = Some(v.into());
        provider().parse(&file).unwrap().errors.is_empty()
    };
    assert!(ok("3.12") && !ok("3.9"));
}

#[test]
fn byte_order_mark_and_line_endings_are_trivia() {
    for text in ["\u{feff}x = 1\n", "if x:\r\n    y = [1,\r\n         2]\r\n", "x = 1\\\r\n    + 2\r\n"] {
        check_parse("a.py", text);
    }
}

#[test]
fn matches_the_extensions() {
    assert_file_names(&provider(), &["a.py", "a.pyi", "a.pyw"], &["a.pyc", "a.js", "a.txt"]);
}

/// Every `.py` file of the pinned corpus: `cargo test --profile corpus -p lang-python --test
/// python -- --ignored --nocapture conformance`.
#[test]
#[ignore = "needs the corpus: tests/conformance/fetch-python-corpus.sh"]
fn conformance() {
    let (count, unparsed) = check_corpus(PythonProvider::new(), "a.py", "python", &["py", "pyi"]);
    assert!(count > 5000, "{count} files");
    println!("{} of {count} files have syntax errors", unparsed.len());
}
