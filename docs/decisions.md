# Decisions

Where plan.md leaves a detail open, or M0 had to choose, the choice and its reason go here.

## M0 wire-encoding benchmark

Question (plan.md "Wire encoding"): how to send a tree between processes. Candidates:
(a) flat arrays with custom framing, (b) MessagePack per element, (c) protobuf packed arrays,
(d) the flat arrays as raw `bin` fields inside the existing MessagePack message.

### Method

* Corpus: dotnet/runtime tag `v10.0.0` (`60629d14374c56f1cb51819049ad1fa529307f8d`),
  `src/libraries/System.Private.CoreLib/src`: 1679 `.cs` files, 27.8 MB, 6.2 million tree elements
  (nodes, tokens and trivia). Fetched by `tests/conformance/fetch-corpus.sh`.
* Every candidate encodes the same element sequence: per element a kind (u16) and one number
  (token width in UTF-8 bytes, or node child count). Token text is not sent.
* C# side: `workers/csharp/Sudsy.Bench` (Release, .NET 10, Roslyn 5.9.0). Parse time is
  `CSharpSyntaxTree.ParseText` + `GetRoot`; "mapping" is our Roslyn-to-grammar walk; "serialize" is
  only the candidate's encoding of the mapped tree (method used for the results below; the tools now run the production path only). One warm-up pass, then one measured pass over
  all files.
* Rust side: `tools/wire-bench` (release). "Deserialize" turns the bytes into the element arrays;
  "build tree" turns the arrays into a rowan green tree (identical for all candidates). Best of
  four passes. Every tree was checked to print back to its source.
* Machine: the development laptop (Windows 11); numbers are indicative, run-to-run noise of
  C# serialization is roughly 2x (GC), so the table uses the median of three runs.

### Results

Roslyn parse of the whole corpus: 1.74 s (median of 3 runs: 2.02, 1.74, 1.71 s).

| Option | Size | C# serialize | Rust deserialize | Overhead vs parse (serialize + deserialize) |
| --- | --- | --- | --- | --- |
| (a) flat arrays, custom framing | 18.70 MB | 88 ms | 13 ms | 5.8 % |
| (b) MessagePack per element | 31.12 MB | 100 ms | 73 ms | 10.0 % |
| (c) protobuf packed arrays | 18.69 MB | 90 ms | 45 ms | 7.8 % |
| (d) flat arrays as `bin` in MessagePack | 18.69 MB | 74 ms | 10 ms | 4.8 % |

Costs common to every option, which the encoding cannot change:

| Step | Time | vs parse |
| --- | --- | --- |
| Mapping walk (Roslyn red tree to our elements, with trivia re-attachment) | 1.67 s | 96 % |
| Rust rowan tree build | 0.26-0.31 s | 15-18 % |

### Decision

**(d) flat arrays as raw byte fields inside the MessagePack message.** It is within the 10 %
budget with margin (b is borderline and 67 % larger), it is the fastest to decode, and it needs no
framing beyond the protocol's own MessagePack message, which makes it the simplest to integrate
(a would add a second container format for the same bytes; c adds a dependency and a second
serialization stack).

Wire format v1 (`WireTree`, `crates/core-syntax/src/wire.rs`): `version: u32`, `kinds`: `bin` of
little-endian u16, `values`: `bin` of LEB128 varints (token: UTF-8 width; node: child count).
Whether an element is a token is a bit of the kind id, so there is no per-element flag. Node widths
are not sent: they are the sum of the children's. This is slightly leaner than plan.md's "kind,
width and child count per element" and carries the same information.

Only (d) is in production code. The losing candidates (a), (b) and (c), with their protobuf
packages and crate, were removed from `workers/csharp/Sudsy.Bench` and `tools/wire-bench` after the
decision; they remain in git history at commit 5ae70ca. Both tools now benchmark the production path
only (Roslyn parse, mapping walk, `WireEncoder` + MessagePack, Rust decode through
`provider_api::protocol::decode` and `WireTree::unpack`, rowan build), reusing the production
encode/decode code, as the tracked baseline for M4 mapping-cost work.

### Finding that matters more than the encoding

Encoding is cheap; **walking Roslyn's tree is not.** The mapping walk costs about as much as the
parse itself, because the public API materializes red nodes, tokens and trivia lazily and the parse
only builds the green tree. Together with the Rust tree build, a C# file costs roughly 2.1x its
Roslyn parse time to reach the Rust side, which is above plan.md's 10 % aspiration for tree
transfer (that was framed around encoding alone). Mitigations to consider before M4: a single-pass
walk (the current one pre-collects tokens), caching by content hash (already in the plan), and
a persistent worker. End to end the corpus (27.8 MB) parses, ships and rebuilds in about 9 s in
the conformance test, including process and per-file round trips. Recorded as a risk; not blocking
M0.

## Tree model

* **Kind namespace** (`crates/core-syntax/src/kind.rs`): `u16` = 4 bits language id (0 core, 1 JSON,
  2 C#, 3 YAML), 1 token bit, 11 bits index (2048 node and 2048 token kinds per language). The
  token bit makes "is this element a token" free on the wire.
* **Trivia attachment** is encoded structurally: trailing trivia are siblings right after their
  token, leading trivia siblings right before theirs. A "trivia run" between two real tokens is
  split after the first trivia token that ends with a line break (`\n`, `\r`, NEL, LS, PS); a run
  without one is entirely trailing. `trivia::check_attachment` verifies it; the corpus test runs it
  on every file. Each tree ends with a zero-width EOF token that takes the leading trivia of the
  file's tail.
* **Embedded languages** need no special node shape: an `EMBEDDED` node (core namespace) simply has
  the nested language's root as its child, and kinds are namespaced, so the wire encoding carries
  it unchanged (tested). `embedded_language()` reports the nested language.
* `ERROR` exists in the model and wire encoding. The C# worker does not emit it yet: Roslyn's
  skipped tokens arrive as one `SkippedTokensTrivia` token and the errors travel in the parse-error
  list. Proper recovery nodes are part of the M4 mapping work.

## Grammar format

* Line-based, ungrammar-like; see the header of `crates/core-syntax/src/grammar/mod.rs`. Field
  types are a kind name or a `#tag` ("any kind carrying this tag"), which replaces ungrammar's
  explicit alternation and means category tags do double duty as the "union types" of the
  generated wrappers (`AnyExpression`, `AnyValue`).
* The grammar hash is SHA-256 over the lexemes: comments and layout do not change it, declaration
  order does (it determines kind ids).
* Generated Rust wrappers live in `crates/core-syntax/src/generated/` for now. They move to the
  `lang-*` crates when those exist (M1, M4).
* `grammars/csharp.grammar` was bootstrapped from Roslyn's `SyntaxKind` enum and syntax classes
  (kind names equal Roslyn names; tags and partial fields derived heuristically) and is
  hand-maintained from here on. The bootstrap script is not kept in the repo. Fields exist only for
  kinds whose Roslyn class has the same name (e.g. `ClassDeclaration`), 574 kinds in total.
  Doc comments and directives are single trivia tokens; structured trivia is M4.
* The C# worker gets its kind table and grammar hash from `cargo xtask codegen`
  (`workers/csharp/Sudsy.Worker/Generated/CsharpGrammar.g.cs`); `xtask/tests/codegen_is_fresh.rs`
  fails if any generated file is stale. The kind table is matched to Roslyn kinds by name.

## Worker protocol

* Enum encoding is rmp-serde's default (a one-entry map `{Variant: payload}`); payloads are arrays
  in field order, so MessagePack-CSharp classes use integer `[Key]`s. A unit test pins the bytes.
* One request in flight, so there are no request ids.
* `Hello` carries protocol version, grammar hash, flow-graph format version, tree wire-format
  version and capabilities (empty for now). A mismatch is refused by both sides: the worker replies
  `version_mismatch` and exits; the proxy also checks the worker's reply, and neither case is
  retried.
* Text is sent without a leading BOM (`SourceFile::from_bytes` strips it and records `had_bom`).
  Roslyn's UTF-16 offsets are converted to UTF-8 byte offsets in the worker.
* The worker runs on a thread with a 256 MB stack, because deeply nested expressions recurse deep
  inside Roslyn.
* Crash or hang policy: the proxy restarts the worker once per run (`max_restarts = 1`) and
  retries the request; a second failure makes it unavailable for the rest of the run. A hang is a
  request without an answer within `request_timeout` (120 s default). Failing to start the process
  or being refused at the handshake is final immediately.
* Worker discovery for now: `SUDSY_CSHARP_WORKER` (path to `Sudsy.Worker.dll`), else
  `csharp-worker/Sudsy.Worker.dll` beside the executable. The worker runs through the `dotnet` host
  (framework-dependent, `RollForward=LatestMajor`).

## provider-api

* `LanguageProvider::parse` returns `Result` (plan.md sketches a plain value) because an
  out-of-process provider can be unavailable.
* Only `id`, `grammar`, `file_matchers`, `capabilities` and `parse` exist. The rest of plan.md's
  trait (`formatter`, `option_schema`, `presets`, `builtin_rules`, `semantic`, `flow`, `summaries`)
  is added with the milestone that introduces its types.
* The C# provider constructor lives in `provider-api::csharp` until `lang-csharp` exists (M4).

## core-config

* `serde-saphyr` 2 parses the file. Booleans are strict (`strict_booleans = true`): only `true` and
  `false` are booleans, so `yes`, `no`, `on`, `off` stay strings (tested), and an unquoted `off` is a
  valid rule level. Duplicate keys are errors.
* Unknown keys are errors with line and column. Option names and values stay untyped
  (`OptionValue`) until providers declare option schemas (M2). Rule levels are `off`, `warn`,
  `error`.
* An empty or comment-only file is the default config. Both `sudsy.yaml` and `sudsy.yml` in one
  directory is an error.

## Tooling

* `cargo xtask codegen [--check]` (alias in `.cargo/config.toml`).
* The conformance corpus is not fetched in CI. The corpus test is `#[ignore]`d:
  `bash tests/conformance/fetch-corpus.sh && cargo test --release -p provider-api --test corpus_roundtrip -- --ignored --nocapture`.
* Rust tests that need the worker build it with `dotnet build` on first use.

## M1a: formatting engine and JSON

### core-format
* **Printer**: Prettier's algorithm (group fits flat if the rest of the line up to the next
  possible break fits; `fill`; `conditional_group` takes the first candidate that fits, trying each
  flat but allowing forced breaks inside it; line suffixes flush at the next newline; trailing
  whitespace is trimmed). Widths use Unicode display width. A text containing a newline breaks the
  enclosing groups (`verbatim_text`) and only its first line counts when measuring.
* **Shared options** (`core_config::shared`): `line_width`, `indent_size`, `indent_style`
  (`space`|`tab`), `final_newline`. plan.md says "indent"; two options keep tabs and width apart.
  Providers pick their own defaults. `format_tree` always prints LF and then restores the file's
  line ending (first line break decides) and applies `final_newline`.
* **Comments** are never positioned by the formatter: a comment is owned by a token (trivia
  attachment rule), and printing a token through `FormatCtx::token` prints its comments. Own-line
  comments before a token stay on their own lines; a block comment on the same line stays inline;
  a line comment after a token becomes a line suffix and breaks the enclosing group. Comments
  before a closing delimiter are "dangling" and printed inside the container. Comments around a
  list separator are printed as trailing comments of the item; a second comment on a line that
  already ends in a line comment moves to its own line (found by a property test).
* **Blank lines** between items are kept (at most 1 by default, `ListStyle`/`BlockStyle`
  `max_blank_lines`). In a list that fits on one line they vanish.
* **Shapes**: delimited list, block, key-value, statement sequence, verbatim. A shape finds its
  structure from the grammar: first and last significant token are the delimiters, child nodes
  are items, tokens tagged `#separator` after an item are its separator. List separators are
  re-created from `ListStyle`; block separators are kept as written. **Binary chain and member
  chain are left for M4**: they need operator and call-chain knowledge that is not generic.
  JSON objects are tagged `#block` in the grammar but are laid out with the list shape (the
  per-node code in `lang-json` does this; the `#block` tag default stays "one item per line").
* **Layout directives** (`DIRECTIVES_VERSION = 1`): `BreakAll`, `KeepTogether` (list layout),
  `BreakBefore`/`BreakAfter`, `BlankLineBefore(n)` (read by the item gap in lists and sequences:
  a forced break expands the whole list, since a hard line breaks its group), `Align` (on the
  parent: key-value separators line up, only when the parent is expanded), `Preserve` (verbatim),
  `Style(name)` (read by provider code; shapes ignore it). Conflicts: `BreakAll` with
  `KeepTogether`, `Preserve` with any other directive, two different `BlankLineBefore` or `Style`
  values. Detected when a directive is added.
* **Ignore comments**: `sudsy-ignore` before a node keeps it verbatim (the outermost node that
  starts at that token); `sudsy-ignore-start` .. `sudsy-ignore-end` among the items of a list or
  block keeps those items verbatim, including the separators between them. A start without an end
  runs to the end of the container. The comment's first word must equal the marker.
* **Subtrees**: `FormatCtx::format(node)` depends on nothing but the node and its neighbours in
  the tree, so a subtree can be formatted in context (tested). Range formatting itself is not built.
* **Safety check**: re-parse, then compare the sequences of node starts/ends and non-trivia
  tokens. **Tokens tagged `#separator` are ignored too**, because adding or removing a trailing
  comma is a legitimate change; a wrongly removed separator still changes the structure or fails
  the re-parse. `NodeFormatter::extra_check(before, after)` is the hook for the YAML data-model
  comparison. The test helper (`core_format::testing::check`) adds comment preservation to
  round-trip, meaning and idempotency.
* `provider_api::format_file` is the pipeline (parse, refuse files with syntax errors, format,
  safety check). C#'s `OutOfProcessProvider` has a `DefaultFormatter` and an empty option schema
  until `lang-csharp` (M4); the CLI only registers providers that can format.

### lang-json
* **Parser**: hand-written lexer and recursive-descent parser. Strings, numbers and escapes are
  validated by the lexer. Errors: stray tokens become `ERROR` nodes; every token is in the tree
  (checked by property tests and JSONTestSuite). Comments and trailing commas are *errors in the
  `json` dialect* (the tree is the same). Nesting deeper than 256 levels is an error, the rest of
  the file becomes one `ERROR` node (bounds recursion in parser, formatter and tree drop).
  An empty file is an error (`n_structure_no_data`).
* **Dialect**: matcher (`*.json` json; `*.jsonc`, `tsconfig.json`, `.vscode/*.json` jsonc; most
  specific matcher wins), overridable with the JSON option `dialect` (`auto`|`json`|`jsonc`). The
  effective dialect travels in `SourceFile::language_version` and, written back by
  `resolve_dialect`, in the option `dialect` so the formatter knows whether trailing commas are
  allowed. `FilePattern::InDirectory` was added for `.vscode/*.json`.
* **Options**: `line_width` 80, `indent_size` 2, `indent_style`, `final_newline` true, `dialect`,
  `trailing_commas` (`never`|`multiline`, JSONC only, default `never`) and `collapse`:
  `preserve` (default: an object or array stays on one line only if its source had no line break
  anywhere between its delimiters, nested items included, and it fits), `always` (anything that fits goes on one line), `never` (every
  non-empty one is expanded). Objects print `{ "a": 1 }`, arrays `[1, 2]`; not configurable.
* **Config resolution** (`Config::format_options`): schema defaults, then the shared `format:`
  keys, then the language section. Unknown keys in a language section and invalid values are
  errors. A shared key the language does not declare is ignored; a key that is not a shared option
  at all is an error. M2 extends this one function.
* JSONTestSuite is pinned to nst/JSONTestSuite `1ef36fa` (`tests/conformance/fetch-json-corpus.sh`).
  The test is `#[ignore]`d like the C# corpus test. Every accepted file is also formatted with all
  checks in both dialects.

### CLI and discovery
* `core-workspace` has only `discover_files` (walk, `.gitignore` also outside a git checkout,
  hidden directories included, `.git` excluded; explicit file arguments are always kept).
* `sudsy format [--check] [paths]`: exit 0 clean, 1 if `--check` finds files that would change,
  2 if any file failed (syntax error, unreadable, bad option). Failed files are left alone and
  the rest are still processed. A UTF-8 BOM is kept; config is the nearest `sudsy.yaml` above the
  current directory (per-file resolution is M2).
