//! Parse, round trip and the shared format checks on the bench inputs and on probes of recent
//! syntax: every file parses without errors, prints back byte for byte, satisfies the trivia
//! attachment rule and has no `Unknown` token.

use core_syntax::{print, trivia};
use lang_js::JsProvider;
use lang_js::generated::kind;
use provider_api::format_testing::{Formatting, assert_file_names};
use provider_api::{LanguageProvider, SourceFile};
use std::path::PathBuf;

const TS: &str = r#"#!/usr/bin/env node
import type { A } from "./a";
import json from "./x.json" with { type: "json" };
export type * as ns from "./ns";
using res = getResource();
class C<in out T> {
  accessor x = 1;
  static { init(); }
  #p = 1;
  has(o: object) { return #p in o; }
  @dec() method<const U extends string>(this: C<T>, a?: U): asserts a is U {}
  declare readonly y: unique symbol;
  override z!: number;
}
type A<in out T> = T extends [infer U extends string] ? `x-${U}` : never; // trailing
enum E { A = 1, B }
declare module "m" { export const x: number }
namespace N.M { export const y = 1 }
const v = <const T,>(a: T) => a satisfies unknown as any;
let big = 1_000n, re = /[\p{L}--[a-z]]/v;
f?.<string>(x)!.y;
import foo = require("foo");
const inst = Map<string, number>;
await 1;
/* the end */
"#;

const TSX: &str = r#"const a = <T,>(x: T) => x;
const el = <ns:tag a:b="1" {...props}>{/* c */}text {x} <></></ns:tag>;
const t = <div className={`a ${b}`}>
  {list.map((x) => <li key={x}>{x}</li>)}
</div>;
"#;

const JS: &str = r#"#!/usr/bin/env node
"use strict";
const a = `t${1 + `n${2}`}`; // template
label: for (const x of y) { if (x) continue label; }
var r = a / b / c, s = /=/.test(x);
return a;
"#;

fn provider() -> JsProvider {
    JsProvider::new()
}

/// Every check on one file. `name` picks the dialect; the shared format checks use `subject`.
fn check(name: &str, text: &str, subject: &'static str) {
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
    let unknown = parsed
        .tree
        .descendants_with_tokens()
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
            assert_eq!(t.text(), text, "{name}: {:?}", t.kind());
        }
    }
    core_format::testing::assert_formats(&Formatting::new(provider(), subject), text);
}

#[test]
fn probes_of_recent_syntax() {
    check("a.ts", TS, "a.ts");
    check("a.tsx", TSX, "a.tsx");
    check("a.js", JS, "a.js");
}

#[test]
fn bench_inputs() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bench/inputs/js");
    let mut count = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        let subject = match path.extension().and_then(|e| e.to_str()) {
            Some("ts") => "a.ts",
            Some("tsx") => "a.tsx",
            Some("jsx") => "a.jsx",
            _ => "a.js",
        };
        check(&name, &std::fs::read_to_string(&path).unwrap(), subject);
        count += 1;
    }
    assert!(count >= 4, "bench inputs missing");
}

#[test]
fn syntax_errors_are_reported_and_lossless() {
    for (name, text) in [
        ("a.js", "let x = ;\n"),
        ("a.ts", "class { \u{0} }"),
        ("a.js", "@@@"),
    ] {
        let parsed = provider().parse(&SourceFile::new(name, 1, text)).unwrap();
        assert!(!parsed.errors.is_empty(), "{text:?}");
        assert_eq!(print(&parsed.tree), text);
    }
}

#[test]
fn matches_the_extensions() {
    assert_file_names(
        &provider(),
        &[
            "a.js", "a.mjs", "a.cjs", "a.jsx", "a.ts", "a.mts", "a.cts", "a.tsx", "a.d.ts",
        ],
        &["a.json", "a.jsonc", "a.css"],
    );
}
