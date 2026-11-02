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
* Generated Rust wrappers live in the language crates (`lang-json/src/generated.rs`,
  `lang-yaml/src/generated.rs`); `csharp.rs` stays in `core-syntax/src/generated/` until `lang-csharp`
  exists (M4). `codegen::rust_module` takes the path by which the module reaches `core_syntax`.
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

## M1b: YAML

### Parser evaluation (plan.md: "evaluate existing crates first")
Checked on crates.io and docs.rs, 2026-11; the crates were not run against the yaml-test-suite.

| Crate | What it is | Why not |
| --- | --- | --- |
| `saphyr-parser` 0.3 (successor of `yaml-rust2`), `libyml`/`unsafe-libyaml`, `serde-saphyr` | YAML 1.2 event or value parsers; events carry source marks | Not lossless: comments, blank lines and the spelling of scalars are not in the output. |
| `yaml_parser` 0.3 (g-plane, MIT; the parser of `pretty_yaml`) | "Semi-tolerant YAML concrete syntax tree parser" on `rowan` | Closest to what we need, but its kinds and tree shape are its own (principle 1: only our grammar defines the tree), its trivia placement is not our rule, and "semi-tolerant" means it deviates from the spec; we need the suite as a gate and control over error recovery. |
| `yaml-edit` 0.3 (Apache-2.0, first released 2025-08) | "Lossless parser and editor", `rowan` based, editing API | Same tree-shape problem, young, and aimed at editing, not at feeding a shared formatter. |
| `tree-sitter-yaml` | Incremental CST grammar with an external C scanner | Adds a C toolchain and a second tree model; its recovery and trivia do not follow our rule. |

Verdict: write our own (as plan.md expected). It is about 1600 lines for the parser and 800 for the
data model loader with scalar decoding, and it passes the whole suite (below).

### Tree shape (`grammars/yaml.grammar`)
* One node per collection, entry and scalar: `BlockMapping`/`BlockMappingEntry`,
  `BlockSequence`/`BlockSequenceEntry`, `FlowMapping`/`FlowSequence`, `FlowPair` (a `key: value`
  in `{}` or the single pair in `[]`), `Scalar`, `BlockScalar` (header and content tokens),
  `AliasNode`, and `PropertyNode` (tag and anchor tokens, then the node they apply to). An empty node
  (`key:`, `-`) is simply absent. Tags: `#value` on every node that can be a value, `#block`,
  `#delimited-list`, `#key-value`, `#string`, `#comment`.
* Indentation is trivia (whitespace tokens). A multi-line plain or quoted scalar is one token that
  contains its line breaks; so is a block scalar's content (indentation included, never ending in
  a line break: the blank lines after it are trivia, and the loader counts them for `keep`
  chomping). This keeps the trivia attachment rule valid for every tree.
* Properties on several lines (`&a`, newline, `!!str`, newline, `value`) are one `PropertyNode`.
* New tags in the shared engine: `#restyle` on scalar tokens and block headers (the safety check
  compares only their position; the data-model check verifies the value) and `#separator` on `---`
  (the formatter may add or drop it; documents are still compared as nodes).

### Parser
* Recursive descent after the spec's productions, with `n` the enclosing block indentation. The
  parser reports events (`Start`, tokens, `Finish`); `parser/builder.rs` lexes the trivia between
  tokens, splits it by the attachment rule and builds the tree, so no byte can be lost.
* An error aborts the rest of its document: everything up to the next document marker becomes one
  `ERROR` node of `Unknown` tokens (every byte stays in the tree), and the next document is
  parsed normally. Errors carry the first problem only.
* Nesting deeper than 100 (`MAX_DEPTH`; JSON allows 256) is an error. YAML recurses more per
  level (collection, entry, value), and a test pins that 100 levels format on a 2 MiB test-thread
  stack in debug builds.
* Choices where the spec and common parsers differ: the closing bracket of a flow collection
  must be indented like its content (spec; libyaml accepts less), so the formatter moves a broken
  flow collection to its own line; `%TAG` handles are checked per document; a document marker
  inside a flow collection, quoted scalar or at the start of a plain scalar line is an error; a
  tab can separate (`key:<tab>value`) but never indent a block collection; the 1024 character
  limit of implicit keys is not enforced.
* Characters outside YAML's printable set are errors; a BOM at the start of a later document
  line is trivia.

### Data model (`lang-yaml::model`)
Loaded from our tree (no second parser): core-schema resolution (`yes`, `on`, `0b1`, `1_000` are
strings; `0o17`, `0xFF`, `.inf`, `.nan` are not), tags (`%TAG` handles, verbatim and custom tags
kept, `!!str`/`!!int`/`!!float`/`!!bool`/`!!null` force the type), anchors and aliases expanded,
plain, quoted and block scalar folding with chomping and indentation indicators (the parent
indentation comes from the tree). A lone `...` is not a document. The end of the input counts as
a line break for block scalars, as the suite's expectations show.

### Formatter
* Options (`option_schema()`): `line_width` 80, `indent_size` 2, `final_newline` true (shared),
  `sequence_indent` (`preserve`, `indented`, `flush`), `quote_style` (`preserve`, `single`,
  `double`), `document_start` (`preserve`, `always`, `never`) and `max_blank_lines` 1.
  No `indent_style`: tabs cannot indent YAML. Deviation from plan.md: `sequence_indent` has a third
  value, `preserve`, and it is the default, because both styles are common (Kubernetes versus
  GitHub Actions) and the zero-config goal is the smallest diff.
* Zero-config behavior: quotes, key order, flow versus block, comments and blank lines (at most
  one) are kept; spacing and indentation are normalized (`key: value`, `{ a: 1 }`, `[1, 2]`).
* `quote_style`: a scalar changes only if the other quote style needs no more escapes than the
  source has (a double-quoted `it's` stays double; `"say \"hi\""` becomes `'say "hi"'`), never when
  it contains control characters, and never when it spans lines. Plain scalars are never quoted.
* `document_start`: `always` adds `---` to documents that have content; `never` leaves it out of
  the first document unless it has directives or no content; comments on a removed marker stay.
* Block scalars: contents are re-indented, never re-wrapped; trailing blanks of lines are kept
  (new `Doc::Verbatim` and a protected output length in the printer; `format_tree` no longer trims
  protected trailing blanks and newlines). A block indentation indicator is rewritten to match the
  new indentation. Blank lines after a `keep` scalar are exact, not limited by `max_blank_lines`.
* Multi-line plain and quoted scalars keep their line structure; continuation lines are re-indented
  and lose surrounding blanks (line folding ignores them), except an escaped blank before a break.
* Where a value goes after `key:`, `-`, `?` or `---` is `block::node_after`: scalars stay on the
  line they were on, a collection after `-` starts on the dash line (`- a: 1`), a comment after the
  indicator keeps the value on the next line. A flow collection that does not fit moves under its
  key (`key:`, newline, `[`...`]`), because the closing bracket may not sit at the key's column.
  A flow collection used as a key is printed as written so that it never breaks.
* Hooks (all in `lang-yaml/src/format`): the flow-versus-block placement above, and **comment
  placement at the end of block collections**: the comments between the end of a nested
  collection and the next token belong to that token (trivia rule) and would move to its
  indentation. A block collection that ends right before the token takes the leading comments
  indented at least as far as itself (innermost first) and prints them after its last entry.
* `sudsy-ignore` before an entry and `sudsy-ignore-start`/`-end` among the entries of a block
  collection keep those entries as written; flow collections use the shared list shape's handling.
* Safety: the generic tree comparison, then `extra_check` loads the data model before and after
  and aborts on any difference (value, type, tag, structure, document count).
* The CLI formats `.yaml`/`.yml` files, `sudsy.yaml` among them, with the options of the nearest
  config. The config loader itself still uses `serde-saphyr` (M0); switching it to our parser is
  not part of M1b.

### Known gaps
* Trailing blanks at the end of a block scalar's last line, and a `keep` scalar's blank lines at
  the very end of the file, survive only through the printer's protected output length.
* Properties split over several lines with a comment between them are printed as written.
* The formatter does not wrap long plain or quoted scalars or re-flow long comments.
* Existing files with a flow collection whose closing bracket sits at its key's column are
  rejected by the parser (as the spec requires).
* Speed is about 1 MB/s end to end (parse, format, re-parse, tree comparison, two model loads),
  the same order as JSON; the shared engine dominates.

### Conformance (yaml-test-suite `data` branch at `6ad3d2c`, `tests/conformance/fetch-yaml-corpus.sh`)
`cargo test --release -p lang-yaml --test conformance -- --ignored --nocapture`:
308 valid cases parse without errors, round-trip byte for byte and satisfy the attachment rule;
94 error cases report an error (402 in all); 279 of the 279 valid cases with an `in.json` have an
equal data model; all 308 valid cases format with every shared check (re-parse, tree comparison,
data model, comments kept, idempotency), also under each of the 22 single option values and two
combined option sets.

## Incremental engine (`core-incremental`)

Built on `salsa` 0.28 (current stable). Nothing in it blocked use: tracked functions, singleton
inputs, event hooks, thread handles and structural-equality backdating all fit what exists.

### Graph
* **Inputs:** `File { path, version, text, had_bom }` (one per file; an edit sets `text` and
  `version`) and the singleton `ConfigFile { path, text }`. Which config applies (the walk up from
  the working directory) stays with the caller; the engine sees only its contents.
* **Queries:** `config` (parse + validate) -> `selection` (provider and matcher dialect, from the
  path only) -> `options` (resolved format options, dialect fixed) -> `language_version` (the
  only part of the options parsing reads, a separate query so changing `indent_size` leaves the
  parse valid) -> `parse` (green tree + parse errors) -> `format` (output; the safety check runs
  inside it, so "formatted output plus safety result" is one value, `Ok(text)` or the failure).
* The provider list is part of the database, not an input: fixed for its lifetime. The Wasm host
  (M6) will make it dynamic, and it becomes an input then. Rule results, semantic requests and
  flow graphs plug in as further queries over `parse` (comment at the top of `queries.rs`); none
  exist yet.

### Failures are values
`Failure { stage, message }` is `Clone + Eq`, so a failing query is memoized and backdated like
any other. The layers below have non-`Eq` errors; the message is the error and its causes joined
with `": "`, exactly what `anyhow`'s `{:#}` printed, so CLI output is unchanged. A C# worker that
is missing or crashed is `Err(Failure { stage: Parse })`, never a panic. `parse` is not pure for
C#, but is determined by what it reads (path, version, text, dialect) given a deterministic
worker. A failed parse stays memoized for the run (the proxy already declares the worker unusable
for the rest of the run); an editor retries by setting the file again, which always bumps the
revision even for the same text.

### Equality and rowan
Query values need `Eq`/`PartialEq` for backdating. `SyntaxNode` (red) is `!Send` and has identity
equality, so `parse` stores the `GreenNode`: `Send + Sync`, a refcounted pointer (cloning from a
memo is free), and structurally comparable. The red root is rebuilt in O(1) with
`SyntaxNode::new_root` where a consumer needs one. Structural `==` costs O(tree) but only when a
query re-runs and a previous value exists; the first run never compares. The alternative
(pointer equality) would make every re-parse "changed". Because trees are lossless, an equal tree
means equal text, so tree backdating only fires when the text is unchanged (e.g. new `version`);
the backdating that pays off is further down: `config` (comment-only edits), `options` (a change
to the `yaml:` section leaves JSON options equal, so JSON files are not reformatted), and
`format` (a whitespace edit whose output is unchanged stops at `format`: dependents do not
re-run). `format` reads `version` (the safety re-parse needs it), so a version bump alone
re-runs it. `salsa::Update` is satisfied by salsa's `PartialEq` fallback; no `unsafe` needed.

### Parallelism
A salsa database handle is `Clone` and shares the memos; reads on separate handles run
concurrently and identical in-flight queries block rather than duplicate. Setting an input needs
`&mut` access and waits for all other handles to be dropped (readers on other handles are
cancelled), so the rule is: set inputs on one handle, then fan out. The CLI sets the config, then
`std::thread::scope` workers (no new dependency) each take a clone, claim paths from an atomic
counter, read the file, create its `File` input (creating is allowed concurrently) and run
`format`. Results are reported in path order afterwards, so output and exit codes are identical
and deterministic. The C# worker is one process behind a `Sync` proxy, so C# parses serialize on
it. Memos for the whole run stay in memory until exit (previously each file was dropped after
formatting); `lru` on `parse` is the lever if that matters for very large trees.

### Tests and results
`crates/core-incremental/tests/engine.rs` (11 tests) asserts on salsa `WillExecute` events:
one edited file re-parses only that file; a config edit re-resolves options without re-parsing;
a comment-only config edit stops at `config`; an unrelated-language config edit stops at
`options`; a whitespace edit with equal output stops at `format`; worker failure (a real
`OutOfProcessProvider` with a missing command) is an error value; threads share memos.
`sudsy format --check` on 2381 mixed JSON/YAML files (2.3 MB, 10 syntax errors, release,
Windows): 1.82 s before, 0.75 s after; stdout, stderr and exit code identical.

### Small changes elsewhere
`provider_api::format_parsed` (format an already-parsed tree; `format_file` calls it) and
`select_provider_index` (`select_provider` calls it).

## Performance

Release build, Windows, one thread, system allocator, `format_file` as the CLI calls it
(parse, layout, print, re-parse, tree comparison, YAML model comparison). Timings vary by about
10% between runs.

### Method
* `cargo run --release -p perf-bench -- [json|yaml|json-flat|all] [--phases-only]`
  (`tools/perf-bench`) generates the inputs deterministically (40k pretty-printed JSON objects,
  11.1 MB; 60k YAML sequence items, 5.0 MB; a one-line JSON object, 1.1 MB), times every phase
  separately, and times whole `format_file` calls at 1x, 4x (the sizes above) and 16x.
  `--features count` adds allocations per phase and `--sites` samples which code allocates;
  `--features mimalloc` swaps the global allocator.
* No sampling profiler runs here (`samply` needs Administrator), so the work was found from
  phase timings, allocation counts and sampled allocation backtraces.
* Guards, counting work instead of time so they cannot flake: `core-format/tests/scaling.rs`
  (steps `fits` takes grow at most 4x for 4x the document, for long lists, deep nesting and
  fills) and `tools/perf-bench/tests/scaling.rs` (allocations of a whole `format_file` call grow
  at most 4.6x for 4x the input, for JSON, one-line JSON and YAML, and for nesting depth).

### Result
| phase (ms) | JSON before | JSON after | YAML before | YAML after |
|---|---|---|---|---|
| parse | 539 | 306 | 345 | 303 |
| layout document | 6255 | 869 | 3970 | 1155 |
| print | 405 | 233 | 376 | 179 |
| drop of the layout document | (not timed) | 150 | (not timed) | 104 |
| safety: re-parse | 336 | 313 | 346 | 305 |
| safety: tree comparison | 940 | 135 | 606 | 103 |
| safety: YAML data model (2 loads, were 2 loads and 2 parses) | - | - | 1344 | 625 |
| total | 8475 (1.3 MB/s) | 2007 (5.6 MB/s) | 6987 (0.7 MB/s) | 2773 (1.8 MB/s) |

Whole `format_file`, best of 3: JSON 5.3 / 5.3 / 5.0 MB/s and YAML 1.8 / 1.7 / 1.6 MB/s at
1x / 4x / 16x (before: 1.5 / 1.4 / 1.3 and 0.8 / 0.75 / 0.76 on slightly larger inputs). Time is
linear in the size of the file. With mimalloc as the global allocator: JSON 6.7, YAML 2.4 MB/s.

### Root causes and fixes
* **Tree comparison** built a red node, a `String` and an event per token of both trees; it now
  walks the green trees (`safety.rs`), 7x faster.
* **Item documents were cloned** at every level (`item.doc.clone()` in lists and blocks), copying
  each subtree once per ancestor; they are moved.
* **`last_token` / `first_token` per item** scanned the whole subtree (size x depth); item
  tokens are no longer needed (blank lines and ignore marks come from `comments::node_leading`),
  and `last_token` walks back from the end of the node.
* **Collapse-preserve check** (`spans_lines`) scanned the text of every list for a line break; for
  nested lists on one line that is size x depth (4x the depth cost 14x). `FormatCtx` now finds
  the tree's line breaks once (`lines.rs`) and answers each list by binary search.
* **Per-token costs**: tag queries compared strings (now a bit set for the hot tags); comment
  lookups collected trivia into a `Vec` of red tokens (now a green-tree walk that allocates
  nothing when there is no comment, and a token without comments is plain text, not a concat of
  three documents); punctuation and literals share their text instead of allocating.
* **Printer**: every `indent` document pushed a new interned level and cloned its string (now
  built once per level); `fits` allocated its work list per call (now reused); ASCII width fast
  path. The measurement was not quadratic: `fits` stops at the first possible break and the rest
  of the queue is not expanded (checked by the scaling test, also for one-line input).
* **Safety check**: the YAML model comparison parsed both texts again (`extra_check` now gets the
  two trees the generic check already has); the re-parse copied the source text only to replace
  it; `format_tree` copied the output twice and the input once.
* **YAML**: comment takers (`last_token`, `column`, `prev_significant` of every ancestor) ran for
  every token although there was no comment to place; tokens were copied twice per scalar.

### Not reached: 25 MB/s JSON, 10 MB/s YAML
The remaining time is spread evenly over the phases, so no further single fix helps:
* Parse alone costs 306 ms for the JSON file and the safety check parses the output again; with
  no other work that caps `format --check` at about 18 MB/s. Two thirds of the parse is the
  parser feeding rowan's node builder (cache lookups and green allocations for one token every
  3 bytes of pretty-printed input, most of them whitespace and newlines).
* Layout is about 7 allocations per node, mostly rowan red nodes: every formatter step that
  looks at a child or neighbour allocates a red node or token (100 ns each with the system
  allocator). A faster allocator gives 1.3-1.4x (mimalloc, a one-line `#[global_allocator]` in the
  CLI binary, left to the CLI owner).
* The data-model check of YAML loads two models (half a second for 5 MB).

Proposals, none done here:
1. Set mimalloc (or another fast allocator) as the global allocator of the CLI: +25-35% on
   everything (measured: JSON 5.3 to 6.7 MB/s, YAML 1.7 to 2.4 MB/s).
2. Compare the output without building a second tree: a provider-level `tokens(text)` that
   returns the significant tokens, compared with the input's, would remove the re-parse and
   the tree comparison (about a fifth of the time).
3. Give formatting code a cursor over the green tree (kinds, text, children, trivia ranges by
   index) in place of rowan red nodes, and make trivia of one line a single token. That is a
   change to `NodeFormatter`, and to the provider API for C#, and is what stands between this
   engine and 25 MB/s; it also fits the incremental work in `core-incremental`.
4. YAML: share anchored nodes with `Rc`; `anchors.get(name).cloned()` copies the whole
   anchored node at every alias (not hit by the benchmark, quadratic for alias-heavy files).
