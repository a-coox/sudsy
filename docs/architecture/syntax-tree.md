# Syntax tree, grammars, wire encoding

## Tree
- `rowan` green/red tree; kinds are `u16`: 4 bits language (0 core, 1 JSON, 2 C#, 3 YAML), 1 token bit, 11 bits index.
- Lossless: printing the tree reproduces the source byte for byte; every conformance test checks it.
- Trivia (whitespace, newlines, comments) are tokens. C# doc comments and directives are structured trivia nodes (tag `#trivia`); disabled `#if` text is one verbatim token.
- `ERROR` nodes hold what could not be parsed; every token stays in the tree. Formatter and rules skip subtrees with errors; files with syntax errors are not formatted.
- `EMBEDDED` (core namespace) holds a nested language's root; supported by tree and wire, no provider emits it.
- A zero-width EOF token takes the leading trivia of the file's tail.

## Trivia attachment (every language)
Trailing trivia: after a token up to and including the end of its line. Leading: everything else before a token. A run between two tokens is split after the first trivia token ending in a line break (`\n`, `\r`, NEL, LS, PS); a run without one is all trailing. `trivia::check_attachment` verifies it (also inside structured trivia).

## Grammars (`grammars/<lang>.grammar`)
- Line-based, ungrammar-like: tokens, nodes with named child fields (`name:Type`, `?`, `*`), comment syntax, `suppress` markers.
- Field types are a kind name or a `#tag`. Tags (`#block`, `#delimited-list`, `#key-value`, `#separator`, `#string`, `#comment`, `#binary-chain`, `#member-chain`, `#restyle`, ...) drive shared shapes, rules and patterns; they double as union types in generated wrappers.
- Bodies match children in order (`core_syntax::ast::assign`); `ShapeChecker` verifies every child lands in a slot. Generated accessors use the same routine.
- `grammars/csharp.grammar` follows Roslyn's field lists (321 node kinds, 252 token kinds); hand-maintained. Types are also expressions (every `#type` kind carries `#expression`).
- `cargo xtask codegen` writes typed wrappers (`lang-*/src/generated.rs`), the worker's kind table (`workers/csharp/Sudsy.Worker/Generated/CsharpGrammar.g.cs`) and `schemas/sudsy.schema.json`. `xtask/tests/codegen_is_fresh.rs` fails on stale output.

## Wire encoding (`core-syntax/src/wire.rs`)
Per file, never per node; token text is not sent (the receiver has the source).
- `version: u32`, `kinds`: `bin` of little-endian u16, `values`: `bin` of LEB128 varints (token: UTF-8 width; node: child count), inside the MessagePack message.
- Node widths are derived from children. Whether an element is a token is a bit of its kind.
- Why flat arrays in `bin`: smallest encode+decode overhead (4.8% of Roslyn parse on CoreLib) and no second container format.
- The real cost is not the encoding but Roslyn's red-node materialization during the mapping walk (see [numbers](../performance/numbers.md)).
