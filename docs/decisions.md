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
encode/decode code, as the tracked baseline for mapping-cost work (M2a measured it again).

### Finding that matters more than the encoding

Encoding is cheap; **walking Roslyn's tree is not.** The mapping walk costs about as much as the
parse itself, because the public API materializes red nodes, tokens and trivia lazily and the parse
only builds the green tree. Together with the Rust tree build, a C# file costs roughly 2.1x its
Roslyn parse time to reach the Rust side, which is above plan.md's 10 % aspiration for tree
transfer (that was framed around encoding alone). Mitigations: a single-pass walk (done in M2a), caching by content hash (already in the plan), and
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
* `ERROR` exists in the model and wire encoding. The C# worker emits it for missing and skipped
  tokens since M2a (see "M2a: C# mapping").

## Grammar format

* Line-based, ungrammar-like; see the header of `crates/core-syntax/src/grammar/mod.rs`. Field
  types are a kind name or a `#tag` ("any kind carrying this tag"), which replaces ungrammar's
  explicit alternation and means category tags do double duty as the "union types" of the
  generated wrappers (`AnyExpression`, `AnyValue`).
* The grammar hash is SHA-256 over the lexemes: comments and layout do not change it, declaration
  order does (it determines kind ids).
* Generated Rust wrappers live in the language crates (`lang-json/src/generated.rs`,
  `lang-yaml/src/generated.rs`, and `lang-csharp/src/generated.rs` since M2a).
  `codegen::rust_module` takes the path by which the module reaches `core_syntax`.
* `grammars/csharp.grammar` was bootstrapped from Roslyn's `SyntaxKind` enum and syntax classes
  (kind names equal Roslyn names) and is hand-maintained from here on. The bootstrap scripts are not
  kept in the repo. M2a replaced the partial bodies of the first bootstrap with complete ones, see
  "M2a: C# mapping".
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
* Crash or hang policy: the proxy restarts the worker once (`max_restarts = 1`) and retries the
  request; a second failure in a row makes it unavailable for the rest of the process (a reply
  gives the allowance back, see "M4: Editor"). A hang is a
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
* The C# provider constructor lives in `lang_csharp` (moved out of `provider-api` in M2a).

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
  re-created from `ListStyle`; block separators are kept as written. Binary chain and member
  chain arrived with the C# formatter (M2b, below).
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
  safety check). An `OutOfProcessProvider` has a `DefaultFormatter` and an empty option schema
  unless `with_format` gives it more (C# does, since M2b).

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

### Follow-up decisions (lead)

* mimalloc is the CLI's global allocator (proposal 1). CLI `format --check` on the 10.6 MB JSON /
  5.1 MB YAML inputs: 6.0 s / 9.1 s before the performance pass, 1.7 s / 3.6 s after it, 1.2 s /
  2.4 s with mimalloc.
* Proposals 2 (compare output tokens without building a second tree) and 3 (green-tree cursor for
  the formatter) wait for M2's gate, which measures C# speed against CSharpier. C# throughput is
  bounded first by the Roslyn parse plus mapping walk (about 2.1x parse time, see "Wire encoding"),
  so the formatter's share is decided with real C# numbers rather than JSON/YAML ones. Proposal 4
  (share YAML anchored nodes) is only relevant to alias-heavy files and stays open.

## M2a: C# mapping

The tree the C# formatter (M2b) builds on. Corpus result: 7659 files, 89.8 MB, 26.0 million tree
elements (runtime CoreLib, roslyn, aspnetcore, see below) parse with no errors, round-trip byte for
byte, satisfy the trivia attachment rule and fit every grammar body.

### Crate and protocol
* `crates/lang-csharp` holds the C# provider (`provider`, `locate_worker`) and the generated
  wrappers (`lang_csharp::generated`). `provider-api::csharp` and `core-syntax/src/generated` are
  gone.
* Protocol **v2**: `Parse` carries `preprocessor_symbols` (`SourceFile::preprocessor_symbols`,
  empty by default). Symbols belong to the file's build, so `OpenWorkspace` will supply them
  (M5); until then callers set them, and nothing in the config reads them yet.

### Grammar bodies
* Every node body in `grammars/csharp.grammar` is Roslyn's field list for that kind, in order
  (generated once from dotnet/roslyn's `Syntax.xml`, whose kinds match Microsoft.CodeAnalysis.CSharp
  5.9.0; the generator is not kept). Optional fields are `?`, lists `*`, and a `Choice` in the XML
  (the `for` statement's declaration versus initializers) is optional items in order. 321 node
  kinds, 252 token kinds.
* **Bodies are matched against the children in order** (`core_syntax::ast::assign`): a child goes to
  the first slot at or after the current one that accepts its kind; a `*` slot stays current.
  This replaced "n-th child of that type", which could not tell `condition` from `initializers` in
  `for`, or a cast's type from its operand (a type name is also an expression). The grammar parser
  therefore no longer rejects a type that appears in several items. The generated accessors and
  `core_syntax::shape::ShapeChecker` share the routine; the checker is what the conformance suites
  use: every child must land in a slot (separators aside) and every required item must be present.
  JSON and YAML use the same generated code and pass unchanged.
* Field types follow Roslyn: an abstract class used as a field type becomes a tag on all its kinds
  (`#expression`, `#statement`, `#type`, `#pattern`, `#member`, `#name`, `#simple-name`,
  `#base-type`, `#xml-node`, ...). **Types are also expressions** (as in Roslyn's hierarchy: `int.Parse`
  has a `PredefinedType` on the left of the dot), so every `#type` kind also carries `#expression`.
  Token fields with several kinds get a tag named after the field (`#binary-operator`,
  `#assignment-operator`, `#modifier`, `#attribute-target`, ...).
* Tags for the formatter beyond the plan's list: `#binary-chain` (the 20 chainable binary
  expression kinds plus `and`/`or` patterns; not `is`/`as`), `#member-chain` (member access,
  invocation, element access, conditional access, binding and `!`), `#assignment`,
  `#type-declaration`, `#namespace`, `#interpolation`, `#declaration` (with `#member`). `#block`:
  `Block`, `NamespaceDeclaration`, `AccessorList`. `#delimited-list`: argument, parameter,
  type-argument, type-parameter and attribute lists, all initializers and the collection
  expression, tuples, property and positional pattern clauses, list patterns, switch expressions.
  `#key-value`: `NameColon`, `NameEquals`, `ExpressionColon`, `SwitchExpressionArm`. `#separator`:
  the comma. `#string`: string literals, interpolated strings (and their text).
* `record class` is `RecordDeclaration` and `record struct` is `RecordStructDeclaration` (two kinds,
  one body); the keyword after `record` is in `class_or_struct_keyword`.

### Structured trivia and errors
* Doc comments (`SingleLineDocumentationCommentTrivia`, `MultiLine...`) and directives (`#if`,
  `#region`, `#pragma`, ... all `*DirectiveTrivia`) are **nodes** tagged `#trivia`, holding the XML
  or the directive's tokens. Their kinds moved from the token to the node namespace. Whitespace,
  line ends, `//` and `/* */` comments, `DisabledTextTrivia` and `ConflictMarkerTrivia` stay trivia
  tokens, so a disabled `#if` branch is one verbatim token. `Grammar::is_trivia` is true for the
  structured kinds.
* `trivia::check_attachment` treats a structured trivia as one element of its run (its last
  character decides whether it ends a line) and checks the rule again inside it, as a tree of its
  own. The worker maps a structured trivia as a scope of its own for the same reason.
* Zero-width tokens other than missing ones are kept as tokens: `EndOfDirectiveToken` (which owns
  the directive's line end), `EndOfDocumentationCommentToken`, the `Omitted*` tokens and EOF.
* **Missing tokens** are zero-width `ERROR` nodes where the token would be. **Skipped tokens**
  (`SkippedTokensTrivia`, which is no longer a kind) are an `ERROR` node per skipped run holding the
  real tokens and their trivia, as a sibling of the token they were attached to. Every token is in
  the tree and every byte of the source is covered. `ERROR` nodes are ignored by accessors, and the
  shape check skips nodes that contain one.

### The four CoreLib files with parse errors
All four end a `#if`/`#elif` chain in `#else` + `#error` for a target the conformance run never
defined, so the missing preprocessor symbols were the cause, not the mapping:
`System/OperatingSystem.cs` (no `TARGET_*` operating system; also CS1525 where the `#else` branch
leaves an expression empty), `System/Threading/PortableThreadPool.cs` (`TARGET_64BIT`/`TARGET_32BIT`),
`RuntimeInformation.cs` (`TARGET_AMD64`...) and `RuntimeInformation.Browser.cs` (a browser-only
file: `TARGET_BROWSER`). The conformance run now parses every file with `TARGET_WINDOWS`,
`TARGET_AMD64` and `TARGET_64BIT`, and files named `*.Browser.*` with `TARGET_BROWSER`,
`TARGET_WASM`, `TARGET_32BIT`. The language version was never the issue (latest is used). With that,
no corpus file reports an error, and the test asserts it.

### Corpus
`tests/conformance/fetch-corpus.sh` fetches three pinned, shallow, sparse checkouts (only `.cs`
files): dotnet/runtime v10.0.0 `System.Private.CoreLib/src` (1679 files, 27.8 MB), dotnet/roslyn
tag Visual-Studio-2026-Version-18.7 `src/Compilers/CSharp/Portable`, `src/Compilers/Core/Portable`
and `src/Workspaces/Core/Portable` (2858 files, 39.6 MB), dotnet/aspnetcore v10.0.0 `src/Http`,
`Mvc/Mvc.Core`, `Middleware`, `Servers/Kestrel/Core/src`, `Components/Components/src` and
`SignalR/common` (3122 files, 22.4 MB). Total 7659 files, 89.8 MB; the ignored test
`cargo test --release -p lang-csharp --test corpus -- --ignored` takes about 27 s. It checks
lossless round trip, the attachment rule, token texts by kind (no skipped text hiding in trivia),
no parse errors, and the shape check. Edge-case fixtures live in `tests/fixtures/csharp/`
(18 files; `error_*.cs` must report errors) with insta snapshots of their trees beside them; the
same checks run on every fixture, also with CRLF line ends, in the normal test run.

### Mapping cost
Method: `workers/csharp/Sudsy.Bench` on CoreLib (Release, workstation GC without the background
thread, best of three runs; numbers wobble by about 10 %), plus a scratch profile project that
timed bare Roslyn traversals.

| | Parse | Mapping | Mapping / parse | Elements |
| --- | --- | --- | --- | --- |
| M0 mapper (default GC) | 1424 ms | 1482 ms | 104 % | 6.2 M |
| M0 mapper (no concurrent GC) | 927 ms | 1057 ms | 114 % | 6.2 M |
| M2a mapper | 940 ms | 830 ms | 88 % | 8.4 M |

Per element 170 ns became 100 ns while the tree grew by 35 % (structured trivia). **The 30 % target
is out of reach, and the reason is the Roslyn API, not the mapper.** Visiting every node, token and
trivia through the public API, with no mapping at all, costs 270-330 ms (34-40 % of parse) for the
M0 tree shape and 540 ms (65 %) for the M2a shape, because every node is a red node that the
public API creates and links on demand while the parse only builds the green tree. Roslyn's green
tree is internal and reflection on it is ruled out. What was removed:
* the second pass (`DescendantTokens` collected every token first, the walk then repeated the
  work), per-token lists, and the cut table: one explicit-stack walk, trivia attached as it goes;
* the extra lookup for the rare case that needs the next token (a token whose trailing trivia end
  without a line break). It is skipped unless the character after the token can start trivia
  (`#`, `///`; inside doc comments also line breaks, spaces, `*`), and when the next token is a
  sibling nothing needs to move. This was about 15 % of parse before the cheap checks;
* `ConcurrentGarbageCollection=false` in `workers/csharp/Directory.Build.props`: the worker is a
  batch process and the background GC thread slowed parse and mapping alike by about 20 %.

Not measurable: kind lookups, the output arrays and width computation (stubbing each out changed
nothing beyond the noise). Two routines exist: the fast one for tokens with plain trivia and the
general one (used for missing tokens, and for tokens next to skipped tokens, in files that have
parse errors). `Sudsy.Bench <dir> <out> --verify` and a test over the fixtures check that the fast
path produces exactly what the general routine does; the corpus agrees on all 7659 files.

### Known gaps
* A missing token that carries trivia of its own (only possible next to skipped tokens) is
  attached with the same rule but its position among the neighbours' trivia is approximate; no
  corpus or fixture file has one.
* `#!` (shebang) directives only parse in script mode, so no fixture covers `ShebangDirectiveTrivia`.
* Preprocessor symbols and the language version cannot be set from the config yet.
* The CI does not fetch the corpus; the corpus test is run by hand (as before).

## M2b: C# formatter

Corpus result (`cargo test --release -p lang-csharp --test format_corpus -- --ignored`): all 7659
files (89.9 MB: CoreLib, roslyn, aspnetcore) format, pass the safety check and are idempotent
(`format(format(x)) == format(x)`); 0 failures. The first run had 142 (137 idempotency, 5 output
that did not parse); the causes are in "Comments" below and in the conditional-access fix. Fixtures
and option values are snapshot-tested in `crates/lang-csharp/tests/format.rs` with the shared
round-trip, meaning, idempotency and comments-kept checks.

### Structure
* `lang-csharp/src/format`: `mod.rs` dispatches on kind and grammar tag; nodes with no code of their
  own are `seq` (their children one after the other, separated by `Sep`: none, space, hard line,
  the brace separator, or "embedded statement": indented on its own line). `spacing.rs` decides the
  space between two tokens (a short table of exceptions; the default is a space), `braces.rs` types,
  namespaces, blocks, switch, enum and accessors, `lists.rs` the delimited lists and the lambda
  hook, `chains.rs` binary, member, conditional and assignment layout, `query.rs` LINQ, `tags.rs` a
  table of the grammar tags asked for every node.
* The tag defaults (`#block`, `#delimited-list`, `#key-value`) are not used for C#: every kind goes
  through the dispatch. The grammar tags are used to find kinds (`#binary-chain`, `#member-chain`,
  `#assignment`, `#initializer-expression`, `#type-declaration`, `#statement`, `#switch-label`).
* Files with syntax errors are not formatted: reported with the first error, file left alone, exit
  code 2, same as JSON and YAML. The CLI registers the C# provider always; without a worker
  (`SUDSY_CSHARP_WORKER` or `csharp-worker/` beside the executable) C# files fail with that reason
  and JSON and YAML are still formatted. Without the `preprocessor_symbols` option (M3a) the 4 CoreLib
  files whose `#else` branch is an `#error` fail to parse.
* Safety check: `provider_api::format_parsed`, unchanged (re-parse in the worker, tree comparison
  ignoring trivia; structured trivia count as trivia as a whole).

### core-format additions (all generic)
* `shapes::binary_chain` and `shapes::member_chain` (the two shapes of plan.md that were missing):
  the first lays out operands and operators (operator first on continuation lines), the second
  takes the already formatted head and call groups and chooses between one line and one group per
  line (a `conditional_group`). Reading the tree (precedence levels, which links form a group, when
  the head merges with the first call) stays in `lang-csharp`.
* `shapes::delimited_list_between` and `block_between` for nodes whose delimiters are not their
  first and last tokens (`new { ... }`, `x switch { ... }`, enums, list patterns); `ListStyle`
  got `leading_line` (a `line` before the opening delimiter, inside the group: braces on their own
  line only when the list breaks) and `empty`, `BlockStyle` got `empty`; a child tagged
  `#list-prefix` before the first item is printed first (the attribute target in `[assembly: A]`).
* **Structured trivia are comments** (`comments::Comment::element` is a token or a trivia node).
  Kinds: `Line`, `Block`, `Lines` (a doc comment: printed line by line at the current
  indentation) and `Directive` (tag `#verbatim-trivia`: preprocessor directives, disabled text and
  conflict markers, kept as written starting at column 0; `#region` and `#endregion` also carry
  `#indented` and follow the indentation). New docs: `LineStart` (text at column 0, on a line of its
  own) and `NewlineIfNeeded` (a hard line break unless the line is empty). The safety check skips
  structured trivia whole; the test helper compares them line by line without indentation.
* Own-line comments start their own line (`NewlineIfNeeded`) wherever the formatter put the code
  before them; this changed no JSON or YAML output.

### Comments
* A trailing line comment on an operator (`&&`, `=`, `?`, `:`, `=>`) makes the next operand start a
  new line, otherwise the comment moves to the end of the later line, changes owner and the next
  run formats differently (this was most of the 142 first failures). A line comment between the
  links of a member chain breaks the chain at every dot (two comments would end on one line).
* A comment on its own line between `=` and the value keeps the value on the next line, indented.
* Hooks that print tokens themselves (lambda hug) apply only when no delimiter or comma of the
  argument list and no argument start has a comment; else the shared list shape is used.

### Layout decisions (where plan.md and the style guide are silent)
* Defaults are `csharp/microsoft`: width 120, indent 4, Allman braces, final newline. Options
  (plan.md config section only): `brace_newline` (`all`, `none`), `trailing_commas` (`preserve`
  (default), `never`, `multiline`; applies to initializers, collection expressions, enums and switch
  expressions, the only places C# allows a trailing comma), `member_chain_min_calls` (3), and the
  shared four. `none` moves opening braces to the line of the header; `else`, `catch` and
  `finally` still start a line. Presets and `csharpier-compat` are in "M2c".
* Blank lines between members and statements are kept (at most one), never added. An empty body is
  `{ }` on the header's line. Attribute lists on declarations and statements are one per line,
  inline on parameters, type parameters and lambdas. Expression-bodied members stay on the header's
  line. `where` clauses go on their own indented lines. Embedded statements (`if (x) y;`) go on
  their own indented line. `else if` stays on one line.
* Binary chains: one group per precedence level, operator first; a chain after `=`, `=>` or inside
  `if ( )` or `( )` adds no extra indent because the parent indents. The conditional operator breaks
  before `?` and `:`. Query expressions break before each clause. Interpolated strings are kept as
  written (their expressions are not formatted); raw and verbatim strings are single tokens.
* Member chains: a chain with at least `member_chain_min_calls` calls that does not fit goes
  one call per line; the first call stays on the head's line when the head is `this`, `base`, a
  predefined type, a capitalized name or a name no longer than the indent.
* Lambda as last argument: only for block bodies. A `conditional_group` of "arguments before it on
  one line, body hanging" and the one-argument-per-line list.
* Not done: `sudsy-ignore` is honored on statements, members and list items through the shared
  shapes, not in the middle of an expression; `#if` regions are formatted only for the active
  branch (plan.md), the rest is verbatim.

### Throughput (CoreLib, 1679 files, 27.8 MB, release, single thread, system allocator)
`cargo test --release -p lang-csharp --test format_corpus corelib -- --ignored --nocapture`:

| phase | ms | share |
| --- | --- | --- |
| worker parse (Roslyn parse, mapping, pipe, decode) | 5014 | 33 % |
| layout document | 4652 | 31 % |
| print | 322 | 2 % |
| safety re-parse (the same path as the first parse) | 4631 | 31 % |
| safety tree comparison | 175 | 1 % |
| total | 15158 | 1.84 MB/s |

`sudsy format --check` on the same directory (threads for layout, mimalloc): 8.7 s, 3.2 MB/s. From
Sudsy.Bench (M2a) the Roslyn parse itself is about 0.94 s and the mapping 0.83 s of the 5.0 s: the
rest is the pipe and the decoding into rowan. The re-parse costs as much as the first parse, so
proposal 2 of "Performance" (compare tokens without a second tree) would save about 30 %. Layout
costs 4.7 s, about 180 ns per tree element, mostly red nodes and the `Vec` that `sig` makes per
node; nothing was optimized here beyond a tag table. A `conditional_group` clones its candidates, so
a chain or argument list nested in a hugged lambda doubles the size of its document per level.

## M2c: CSharpier compatibility and speed

### Presets (`core-config::presets`, `lang-csharp::format::options`)
* Superseded in M3a: presets are plain unversioned `sudsy.yaml` files, not `language/name@version` lists (see "M3a: Configuration"); the text below is the M2c state.
* `extends: [preset:csharp/csharpier-compat]` works for C#. A provider declares its presets next
  to its option schema (`OptionSchema::with_presets`); a preset is a name `language/name@version`
  and a list of option values. Resolution: schema defaults, then the presets of the file's language
  in `extends` order, then the shared `format:` section, then the language section. A name without
  `@version` means the newest version; an unknown name of the file's language is a config error
  listing the known ones; entries for other languages and non-preset entries (config files, M3)
  are skipped. (Superseded in M3a: presets are plain config files in `presets/`, all eight of the plan's
  presets exist, and there are no internal option values.)
* `csharpier-compat` sets only public options; see "M3a: Configuration".

### Differential test against CSharpier 1.3.0
* `dotnet-tools.json` pins CSharpier. `tests/conformance/csharpier-baseline.sh` copies the corpus
  to `$SUDSY_DIFF_DIR` (default `<temp>/sudsy-differential`, outside the repository because
  CSharpier honors `.gitignore` and `target/` is ignored) and formats the copy with CSharpier;
  `cargo test --release -p lang-csharp --test differential -- --ignored --nocapture` formats the
  originals with the preset, compares byte for byte, ranks the differing hunks by a coarse
  category and writes examples to `hunks.txt` and the summary to `summary.txt`.
* Files CSharpier leaves unchanged although Sudsy changes them (generated code, syntax errors,
  "too deep") are not compared (48 files at the end). A leading byte order mark is ignored.
* Trend (files identical / files compared; the start counted all 7659 files):

| step | identical |
| --- | --- |
| preset with only `line_width: 100` | 1673 / 7659 (21.8 %) |
| accessors on one line, constructor initializer on its own line, lone block in a switch section, arrow clauses, assignment layouts, initializers laid out by width, trailing commas | 3851 / 7581 (50.8 %) |
| unbreakable single type argument, attribute and simple index; blank lines between members and accessors; property initializers | 4403 / 7556 (58.3 %) |
| base lists, `operator ==(`, `[.. x]`, member chain groups, sole lambda argument hugs, switch arms, property patterns | 4952 / 7611 (65.1 %) |

  End state by corpus: aspnetcore 2192 / 3122, roslyn 1657 / 2858, runtime 1103 / 1679. 3.9 % of
  the 2.38 million output lines are inside differing hunks.
* What the preset reproduces (all in `lang-csharp::format`, behind `options::csharpier`):
  `get { return x; }` stays on a line when the body is one simple statement without comments and
  fits (else braces on their own lines); `: base(...)` always on its own indented line; the only
  block of a switch section is not indented; `=>` of a member breaks as a whole
  (`group(indent(line body))`); assignment layouts (a string, binary chain, conditional,
  collection expression or property initializer moves to the next line when it does not fit,
  everything else is "fluid": only when its first line does not fit, using the new group ids and
  `indent_if_break`); initializers and collection expressions are laid out by width alone (the
  default keeps a list expanded when the source did), `trailing_commas: multiline`; base lists
  break before the `:`, one type per line; a lone type argument, a lone attribute and `a[i]`
  never break (the attribute's arguments break instead of its brackets); a lambda is hugged only
  as the one argument of a call, with a block or an expression body (`Foo(x =>` / body / `)`);
  property patterns and nested initializers put their braces on their own line when they break;
  a member chain keeps the accesses before a call with it (`.A.B()`) and breaks at the dots when
  an earlier group has a block lambda; `} while (...)` on one line; blank lines are added before a
  method, constructor, operator or type with a body (and after one), and before a member that has
  a comment or attribute above it (not before a directive).
* Left (counts are hunks in the final report; `hunks.txt` has examples):
  * **using order** (at least 211 files differ in nothing else): CSharpier sorts `using`
    directives (ordinal, `;` included). That changes the order of the tree, so it needs a safety
    check that treats a run of usings as unordered; not done.
  * **`#if` / `#pragma`**: CSharpier formats the code of every branch and places directives
    differently; Sudsy formats the active branch only (plan.md). Several hundred hunks.
  * **Deeper layout differences** (about 2400 hunks in 474 files, "text differs" in the report;
    mostly the same code with different breaks): interpolated strings as arguments, chains inside
    lambdas, a cast followed by an object creation (`(T)` and `new ...` on separate lines),
    parenthesized operands in binary chains, `is` patterns in `when` clauses.
  * **Trailing commas**: 1569 hunks in 10 test-data files (nested initializers CSharpier keeps on
    one line) and 787 in 223 files (a comma next to a comment, `0x00, /*body*/`).
  * **Indentation** 1353 hunks in 428 files (binary operands inside parentheses, lambdas inside
    chains) and **blank lines** 1363 + 478 hunks (members after `#region`, around comments).

### Speed against CSharpier (same machine, 12 cores, release builds, `tests/conformance/speed.sh`)
`sudsy format --check` with the preset against `csharpier check --no-cache` (CSharpier 1.3.0,
multithreaded). Cold means a new process for every run (JIT, file cache as the previous run left
it). The worker of this CLI lives for one run, so there is no warm CLI number until M4 has a
long-lived worker (the warm numbers are in "M4: Editor").

| | CSharpier | Sudsy before | Sudsy now |
| --- | --- | --- | --- |
| CoreLib (1679 files, 27.8 MB) | 15.7 - 17.5 s | 8.2 - 9.2 s | 3.3 - 3.4 s |
| full corpus (7659 files, 89.9 MB) | 36.8 - 38.0 s | 19.2 s | 8.6 - 8.7 s |
| one file (process start to exit) | 1.07 s | 0.47 s | 0.42 s |

(`csharpier format` of the whole corpus, writing the files: 37.7 s.) Sudsy is 4.7x (CoreLib) and
4.3x (full corpus) faster than CSharpier, and 2.5x faster than before this milestone.

### Speed work
* **(a) `conditional_group` no longer copies its candidates.** New `Doc::Shared(Arc<..>)`
  (`doc::shared`) clones in O(1) and remembers whether it forces a break; `conditional_group`,
  `shapes::member_chain` and the last-argument hook build their candidates from shared parts, so
  the document size is linear in the nesting. Test: 24 nested hugged lambdas format in a moment
  (`presets.rs`, `nested_hugged_lambdas_do_not_grow_exponentially`). A conditional group whose
  every candidate has a forced break now forces its parents (a hugged block lambda breaks the call
  around it, as in Prettier and CSharpier); that changes the default preset's layout too
  (`Add(Foo(() => { ... }))` breaks after the first `(`).
* **(c) The worker's JIT.** `DOTNET_TC_CallCountingDelayMs=0`, set on the worker process by
  `lang_csharp::provider`: the runtime waits for 100 ms without a new tier-0 compilation before it
  starts counting calls, which in a worker that is busy from its first request means seconds in
  unoptimized code for Roslyn and the mapper. First pass over CoreLib: time inside the worker 4.4 s
  to 3.0 s. `TieredPGO=0`, `TieredCompilation=0`, GC budget settings and `QuickJitForLoops=0`
  changed nothing measurable. The pipe is not the problem: sending takes 0.1 s, decoding the
  MessagePack frame 15 ms and building the green tree 0.38 s per pass of CoreLib (26 MB per pass);
  the rest of the 3.5 s a pass waits is inside the worker (Roslyn parse 1.1 s, mapping 1.5 - 2.1 s,
  encoding 0.2 - 0.5 s). Mapping costs twice what `Sudsy.Bench` measures on a tree whose red nodes
  already exist, so what is left is red-node creation in Roslyn's public API (M2a); nothing to
  remove in the worker's I/O.
* **A pool of workers** (not on the list; the largest win): `WorkerProxy` serialized every parse
  of the process behind one worker, so the CLI's layout threads queued for it. The proxy now
  starts up to `pool_size` workers, a new one only when all started ones are busy; C# uses
  `min(cores, 4)` (`SUDSY_CSHARP_WORKERS` overrides). CoreLib: 6.1 s with one worker, 4.1 s with
  two, 3.4 s with four, 3.3 s with six; hence the cap of four. `WorkerConfig` got `env` and
  `pool_size`.
* **(b) The second parse and the layout walk: not done, measured instead.**
  * The check needs the output's tree, and the tree costs Roslyn parse plus mapping, not the
    transfer: of the 2.9 s a re-parse takes, the pipe, MessagePack and green-tree decode together
    are about 0.5 s. A check that keeps the guarantee exactly (the worker hashes the significant
    events of its mapped tree, Rust hashes the same events of the input tree, a mismatch falls back
    to today's full comparison) removes that 0.5 s and the tree comparison (0.17 s): about 5 % of
    the run, for a new protocol message, a hash definition in two languages and a provider API
    method. A token-level check (lexing only) would be several times cheaper, but cannot see
    whitespace-sensitive parsing (Roslyn merges adjacent `>` tokens into `>>` in the parser) and
    would rest on the argument that the parser ignores trivia, which is weaker than the current
    check. Neither is a clear win; the re-parse stays as it is.
  * The cursor over the green tree for the formatter (proposal 3) changes `NodeFormatter` and
    every line of `lang-csharp::format` (red nodes everywhere). Layout is 4.9 s of single-thread
    CPU for CoreLib and runs on the CLI's threads next to the pooled workers, so it is no longer
    the limit of `sudsy format`; it stays an M4 topic (the LSP formats one file at a time).

### Phases, CoreLib, one thread, release worker (`corelib_format_throughput`, default options)
| phase (ms) | M2b (debug worker) | before, this session | now |
| --- | --- | --- | --- |
| worker parse (parse, mapping, transfer) | 5014 | 4150 - 5000 | 3100 - 3300 |
| layout document | 4652 | 4700 | 4850 - 4900 |
| print | 322 | 330 | 320 |
| safety re-parse | 4631 | 3850 - 4500 | 2760 - 2960 |
| safety tree comparison | 175 | 180 | 170 |
| total | 15158 (1.84 MB/s) | 13600 - 15100 | 11800 (2.36 MB/s) |

The test now uses `SUDSY_CSHARP_WORKER` when it is set (a Release build of the worker) instead of
the debug build it makes itself; the M2b column was measured with the debug worker, the next two
with the release one. Run-to-run noise on this machine is about 10 %.

### Other changes
* The corpus gate (`format_corpus`) now formats the corpus twice, with the default options and with
  `csharp/csharpier-compat`: format, safety check and idempotency must hold for both (0 failures
  each; the preset found three idempotency problems: a trailing comma added in front of a line
  comment, which then belongs to the comma the next time, and a comment after the `:` of a base
  list). A comma is no longer added after a last list element that ends in a line comment.
* Cosmetic: a block comment after an opening delimiter sits against it and is followed by a space
  when items follow (`Foo(/* a */ 1)`, was `Foo( /* a */1)`), for every language with comments in
  lists.
* Found by the differential test and fixed for every preset: no space between a modifier and a
  tuple type's `(` (`private (int a, int b) x`), `operator ==(`, `[.. items]`, blank lines
  between comments before a closing brace, and a trailing comma after a complex element
  initializer (`{ "a", 1, }` is not C#; `trailing_commas: multiline` produced it).
* Cleanup: nothing was left over from experiments (the instrumentation used for the pipe and
  worker timings was never committed). `lists.rs` lost the unused `open`/`close` lookup in the
  last-argument hook.

## M3a: Configuration

* **Presets are plain, unversioned config files** (user decision; deviates from plan.md's `@1`
  versioning). Each preset is a `sudsy.yaml` under `crates/lang-*/presets/`, embedded with
  `include_str!` and parsed by the same loader as user configs (unknown keys error, its `extends`
  of other presets works, cycles are caught). `preset:csharp/google@1` is a config error. A preset goes through the same
  merge as an `extends` file (`ConfigChain::preset_files` turns its embedded text into `Loaded`
  configs below the chain's files), so `lint`, `overrides` and the language sections all apply;
  `config print` shows `preset <name>` as the source. A preset can extend presets only (not
  relative files) and cannot set `include`, `exclude`, `analysis` or `plugins` (error). Its
  override globs are relative to the root config's directory. Presets sit below all files, not
  interleaved at the position of the `extends` entry.
* **Presets are pure data.** Formatter code never branches on a preset or tool name. The old
  hidden `layout: csharpier` value and the `csharpier()` helper are deleted; their effects are 18
  named C# options (`blank_lines_between_members`, `assignment_layout`, `indent_nested_binary`,
  ...), JSON `collapse: opening_break`, YAML `quote_style: prefer_double` and
  `blank_lines_after_key`. Two tests keep it so: a hand-written config with a preset's values formats
  every sample like `extends: [preset:...]`, and no formatter source mentions "csharpier".
* **YAML parser for config:** `serde-saphyr` stays. `core-config` cannot depend on a language
  crate, and saphyr gives line/column, `Spanned` values and strict booleans; nothing was left
  behind to delete.
* **Paths** print relative to the working directory with `/`.
* **`.editorconfig`** maps to options per the table in `editorconfig.rs`; invalid values are ignored.
* **Keys not in plan.md** (my additions): top-level `include`/`exclude`, `analysis: fast|standard|deep`
  and the C# option `preprocessor_symbols`. Explicit file arguments bypass include/exclude.
  `analysis` is only a config key until rules arrive (M3b).
* A comment-only edit of the config re-resolves options (locations take part in equality).
* **Commands:** `sudsy config print [file]` and `sudsy init [--preset]`; the JSON Schema is
  generated by `cargo xtask codegen`, not by the CLI.

## JSON/YAML speed against other formatters

`tests/conformance/speed-json-yaml.sh` (inputs and tool versions pinned in
`tests/conformance/speed-tools/`), 12 cores, release `sudsy`, Windows. Every tool in check mode with
caches off and its multithreaded defaults, a new process per run, best of 3 (range in the table,
seconds). Tools: Prettier 3.9.10, Biome 2.6.4, dprint 0.64.3 with `dprint-plugin-json` 0.25.2 and
`g-plane/pretty_yaml` 0.6.0 (`incremental: false`), Node 24.22.0. Sudsy runs its safety check
(re-parse and tree comparison, plus the YAML data-model comparison); the CLI has no switch to turn
it off, because plan.md makes it unconditional. Inputs: the valid JSONTestSuite `y_*` files (95
files, 1.2 KB) and the valid yaml-test-suite cases (308 files, 16 KB), the corpora fetched by
`fetch-*-corpus.sh`; and the large perf-bench inputs written by `perf-bench --write DIR` (one JSON
file of 11.1 MB, one YAML file of 5.0 MB; the tools format differently, so all report changes
and the work differs a little). Biome's YAML formatter is not stable (the changelog still has it
as work in progress), so it is not in the YAML rows. Biome skips files over 1 MiB by default and
prints a diff per changed file, which for the large file takes minutes; it ran with
`--files-max-size` raised and `--max-diagnostics=0`.

| input | Sudsy | Prettier | Biome | dprint |
| --- | --- | --- | --- | --- |
| JSON, 95 small files | 0.10 (0.10 - 0.11) | 0.67 (0.67 - 0.76) | 0.36 (0.36 - 0.40) | 0.37 (0.37 - 0.43) |
| JSON, one 11.1 MB file | 2.03 (2.03 - 2.07) | 9.19 (9.19 - 9.69) | 1.45 (1.45 - 1.71) | 1.91 (1.91 - 1.96) |
| YAML, 308 small files | 0.11 (0.11 - 0.14) | 1.13 (1.13 - 1.23) | - | 0.35 (0.35 - 0.37) |
| YAML, one 5.0 MB file | 2.39 (2.39 - 2.43) | 20.19 (20.19 - 21.79) | - | 3.06 (3.06 - 3.31) |

Sudsy is the fastest on many small files (3.5x to 10x; it is a single native process with no
runtime or wasm start-up, and the others pay 0.35 s or more just to start) and about 4x to 8x
faster than Prettier on large files. On the large files it is on par with dprint (JSON 2.03 s
against 1.91 s, YAML 2.39 s against 3.06 s) and slower than Biome on JSON (2.03 s against 1.45 s,
1.4x), the one tool that beats it. The biggest cost is the layout of the document: 43% of the JSON
and 42% of the YAML time (869 and 1155 ms of 2007 and 2773 ms in the phase table of "Performance"),
mostly rowan red-node allocations. The safety check adds about a third on JSON (re-parse 313 ms,
tree comparison 135 ms) and a third on YAML (plus 625 ms for the data model), and is always on, so
a check-only run could not skip it. No optimization was done here; proposals 2 and 3 in
"Performance" still describe the next steps.

## M3b: Rules

* **Crate and layering.** `core-rules` holds the `Rule` trait with its four scopes, the diagnostics
  and fix model, suppressions, the single traversal per file, rule activation from the config and
  the fix loop. Providers hand their rules over through `LanguageProvider::rules()` and
  `body_kinds()` (default: none), so language crates depend on core, never the reverse. The rules
  every language shares (`max-line-length`) live in `core_rules::shared`. A language rule that
  reused a shared id would show up in `sudsy rules` as one id listed for two languages.
* **`Rule` is object-safe, options are data.** plan.md sketches `type Options: RuleOptions`; an
  associated type would make rules impossible to store as `&dyn Rule`. A rule instead declares
  `OptionDecl`s (name, typed default, description) in its `RuleMeta`; `RuleOptions::resolve` checks
  names and types and the rule reads typed values from `ctx.options`. Rule options are in the generated JSON Schema (`languages::rule_schemas`, one entry per rule id under `lint.rules`), and are also checked when the rules are activated and documented by `sudsy rules <id>`.
* **Rules are zero-sized statics** (`&'static [&'static dyn Rule]`). Rules hold no state; options
  and project facts arrive through the contexts.
* **Scopes.** Node (`kinds` + `check`), file (`check_file`), body (`check_body`, called for every
  node whose kind the provider lists in `body_kinds`; `BodyContext` carries the node only, the flow
  graph joins it in M5) and project (`collect` gathers string facts per file, `finish` reports
  across files, only at `analysis: deep`). No built-in rule uses body or project scope yet; the
  engine tests cover them with test rules. A project diagnostic is not subject to suppression
  comments (it belongs to no single line). Project options are those of the first file of the
  language.
* **Layers and depth.** A rule lists the layers it needs (`Layer`); it is skipped, never failed,
  when the provider does not answer one or the depth does not allow it (`fast` has none, `standard`
  all but `project`, `deep` all). Every v0.1 rule is syntactic and runs at every depth.
  `--analysis fast|standard|deep` overrides `analysis:` through `ConfigChain::with_depth`
  (the key was only stored data in M3a).
* **Rule results are queries.** `lint_settings(file)` (resolved levels, options, depth) and
  `lint(file)` (reads `parse` and the settings). A format-option edit leaves the settings equal, so
  no rule re-runs (tested). A file with syntax errors reports them as `syntax-error` errors and no
  rule runs on it.
* **Levels.** `RuleMeta::default_level` applies when the config does not mention the rule:
  `error` for `no-duplicate-keys`, `warn` for `no-empty-catch` and `async-suffix`, `off` for
  everything that is a style choice or noisy on existing repositories (`ambiguous-booleans` would
  flag every GitHub workflow's `on:`; `sorted-keys`, `document-start`, `using-order`,
  `file-scoped-namespace`, `prefer-null-coalescing`, `max-line-length`).
* **Unknown rule ids are config errors** with a position and a suggestion
  (`ConfigChain::validate_rule_ids`, called by `Project::load`), for the top-level, language and
  override `lint` sections; a `plugin/rule` id is not checked (plugins are M6). A rule named in the
  wrong language's section is accepted and ignored there, because shared sections cannot know the
  language. Invalid options are errors when the rule is activated, naming the rule and where the
  setting came from.
* **Fixes.** Kinds `Safe`, `Unsafe`, `Suggestion`. Per round `select` takes for each diagnostic the
  first fix `fix` may apply, in priority order (category, then rule id, then position), and drops
  a fix when any of its edits overlaps an accepted one; the dropped fix is found again by the next
  round. The loop stops when nothing applies, nothing changes or after 10 rounds. After applying a
  round the file is re-parsed: if it no longer parses the round is undone and the rules are named
  in a warning (`FixOutcome::broke`). `sudsy fix` then formats. "Fixes that delete code check
  behavior summaries" has no case yet: no built-in fix deletes code with possible side effects.
* **Suppressions.** `sudsy-disable-next-line [rule[, rule]...]` in a line or block comment (marker
  from the grammar's `suppress` line; `Grammar::comment_body` is shared with the formatter's
  ignore marks). No rule names suppresses all rules on that line. Plain JSON has no comments, so
  there it is config-only (`overrides` with `"off"`). A directive naming a rule that does not fire
  is not reported.
* **Built-in rules.** JSON `no-duplicate-keys` (keys compared as written between the quotes, so
  `"a"` and `"a"` differ), `sorted-keys` (option `order`, unsafe fix). YAML
  `no-duplicate-keys` (scalar values compared after unquoting; `<<` skipped), `ambiguous-booleans`
  (`yes|no|on|off` in three cases, not under a tag; unsafe quoting fix because a YAML 1.1 parser
  reads it differently afterwards), `document-start` (safe), `sorted-keys` (block mappings only,
  unsafe). C# `no-empty-catch` (no statements and no comment), `async-suffix` (async methods
  returning `Task`/`ValueTask` by name; overrides, explicit interface implementations and `Main`
  skipped; no fix since renaming needs binding), `file-scoped-namespace` (only file member, no
  nested namespace; unsafe because it needs C# 10, and the formatter re-indents afterwards),
  `using-order` (plain usings only, ordinal, `system_first` option, lists with `#` directives
  skipped; safe), `prefer-null-coalescing` (`a != null ? a : b` on names and member accesses;
  unsafe because the type can change; a non-trivial right operand is parenthesized). The semantic C#
  rules of plan.md's table wait for M5.
* **Reordering fixes** (`sorted-keys`, `using-order`) move text between the entries' source slots.
  Comments on lines of their own stay in place; a trailing comment moves with a YAML key or a using
  directive (`entry_range`), because left behind it could end up inside a block scalar. In JSON
  comments stay in place, since a trailing comment can come after the entry's comma. Moving a YAML
  anchor below its alias is why that fix is unsafe.
* **CLI.** `lint`, `fix [--unsafe]`, `check`, `rules [id]`, each with `--format
  human|json|sarif|github` where it reports, and `--analysis`. Exit codes: `lint` 1 on any
  error-level diagnostic; `fix` 1 if errors remain after fixing and formatting; `check` 1 on any
  error or unformatted file; 2 when a file could not be processed or the config is invalid. A file
  `format` would change is reported by `check` as an error diagnostic `format` at 1:1, so every
  output format carries it. JSON output is `{version, summary, diagnostics}` with byte offsets and
  line/column ranges and the fix edits; SARIF 2.1.0 has results and the rule table but no `fixes`;
  GitHub output is `::error|warning file=,line=,col=,endLine=,endColumn=,title=::message`.
  `session.rs` holds the file discovery and parallel loading that `format` already had.
* **Test harness.** `provider_api::testing` (`lint`, `render`, `fix`) runs a provider's rules
  with a `lint:` config and checks that every fix keeps the file parseable and is idempotent; the
  language crates snapshot with it. plan.md's inline-annotation fixtures and a plugin SDK copy of
  the harness come with M6.
* **Not done (later milestones):** Wasm and pattern rules (M6), body-scope rules with flow graphs and
  the semantic rules (M5), an LSP code action per fix (done in M4), SARIF `fixes`, an output file option.

## M4: Editor

### Layout
* **One binary.** The server is `sudsy lsp` (stdio). `crates/lsp` is a library that `cli` calls, so
  the extension bundles one executable; plan.md's `lsp/` exists as a crate, not as a second
  binary. It is built on `lsp-server` and `lsp-types` (rust-analyzer's synchronous scaffold)
  rather than `tower-lsp`: one thread owns the one database and handles one message at a time,
  with no async runtime, and `Connection::memory()` lets the tests drive the real server with an
  in-process client (`crates/lsp/tests`); `cli/tests/lsp.rs` checks the stdio wiring.
* **`crates/languages`** (new, not in plan.md's layout): the built-in provider list, their option
  schemas and rule lists, and `Project` (the config found from a directory, validated against the
  providers), which `cli` had in `project.rs`. `cli`, `lsp` and `xtask` all need exactly this.
* `core_incremental::fix_text` and `transient` are the fix loop over the `lint` query and the
  "same file, other text" input that `sudsy fix` had inline; `fix` and the editor's fix-all use
  the same code.

### Server behaviour (what plan.md left open is marked)
* **Sync:** incremental, positions in UTF-16 (the LSP default; no position-encoding negotiation).
* **Diagnostics are pulled** (`textDocument/diagnostic`, full reports, no result ids, no
  workspace diagnostics), as plan.md says; a client without pull support gets none. Every rule is
  syntactic today, so every pull runs the rules on the current text (memoized per version); the
  debounce plan.md wants for semantic rules and analyzers belongs to M5. Project-scope rules
  (`analysis: deep`) are not reported in the editor: they need every file of the project.
* **Quick fixes:** one action per fix of a diagnostic touching the range (unsafe ones titled
  "(unsafe)" and never preferred); "Fix all `rule` problems in this file" (**decision:** when two
  or more problems of the rule have a safe or unsafe fix, one round of non-overlapping fixes,
  offered only when the result still parses); and `source.fixAll.sudsy` for
  `editor.codeActionsOnSave` (the `fix` command's loop, safe fixes only, offered only when it
  changes the text).
* **Formatting:** the result as one edit between the common prefix and suffix, so cursors and
  folds stay put. **Decision:** a file that cannot be formatted (syntax errors, unavailable
  worker) gets no edits and a log message, not a protocol error, so format on save while typing
  does not pop up errors. Range formatting stays a stretch goal.
* **Config:** one config for the workspace (first folder, else the working directory), loaded as
  the CLI does. It is reloaded when the client reports a change of a `sudsy.yaml`/`.yml`,
  `.editorconfig` or a file the chain extends (the server registers watchers for `**/*.yaml`,
  `**/*.yml` and `**/.editorconfig` dynamically and filters), and the client is asked to refresh
  its diagnostics. An invalid config puts the same error on every file and is shown once as a
  message; a missing worker or .NET is shown once, as a warning, and JSON and YAML keep working.
  Opened files are processed like explicit CLI arguments: `include`/`exclude` do not apply.
  Documents that are not `file:` URIs are ignored. Unsaved edits of a config file take effect
  when it is saved.

### The long-lived worker
* The worker handles one `Parse` at a time statelessly, so a worker that lives for the whole
  session needed no new messages: `OpenWorkspace`, `UpdateText` and `Query` arrive with the
  semantic model in M5. What changed is the failure policy: `max_restarts` counts failures in a
  row and any reply resets it, so a worker that crashes now and then over a day of editing is
  restarted each time instead of being abandoned after the second crash of the session. After
  two failures in a row it stays unavailable until the server is restarted (the extension's
  "Restart Server" command); the failure is shown once. A missing `dotnet` says "C# needs the
  .NET runtime (10 or newer)" (`WorkerConfig::install_hint`).
* `DOTNET_TC_CallCountingDelayMs=0` stays: it costs nothing in a long session and the first file
  of every session is a batch of one. Nothing else in the worker or the proxy existed only for the
  per-run lifetime (comments that said "run" were reworded).
* The pool opens a second worker only when a request finds the first busy; the server handles one
  request at a time, so a session runs one worker process.

### VS Code extension (`editors/vscode`)
* TypeScript, `vscode-languageclient` 9 (pull diagnostics included), bundled with esbuild into
  `dist/extension.js`; it only starts `<server> lsp` and restarts it when `sudsy.serverPath` or
  `sudsy.csharpWorkerPath` change. Documents: C#, JSON, JSONC and YAML files (`.cs` also by
  pattern, so a missing C# extension does not matter). Commands: restart server, show output.
  `sudsy.trace.server` is the client's own trace setting.
* **Config schema:** the extension contributes `yamlValidation` for `sudsy.yaml`/`.yml`
  with a copy of `schemas/sudsy.schema.json` made at build time (`npm run build`, not checked
  in), so the generated file stays the only source. The schema now has the rules and their
  options. `redhat.vscode-yaml` reads it when installed; it is not a dependency (user decision:
  C#/JSON users should not need a second YAML extension that also competes as YAML formatter).
* **Format on save** is the user's choice of `editor.defaultFormatter: sudsy.sudsy` per language
  (plan.md); the extension sets no defaults. This repository's `.vscode/settings.json` does, and
  points the extension at `target/debug/sudsy` and the debug worker; `.vscode/launch.json` starts
  an extension host.
* **Tests:** the pure part (which executable and worker to start) runs under `node --test`
  (`npm test`); the language client is a thin wrapper around `vscode-languageclient`, and the
  protocol behaviour it relies on is what `crates/lsp/tests` drive. No test starts a VS Code
  instance (it would download one on every CI run); the "used daily" gate is a manual one.

### Packaging and worker discovery
* `sudsy` finds the worker as before: `SUDSY_CSHARP_WORKER`, else
  `csharp-worker/Sudsy.Worker.dll` beside the executable. A package is therefore
  `server/sudsy[.exe]` plus `server/csharp-worker/` (the framework-dependent `dotnet publish`
  output, English resources only via `SatelliteResourceLanguages`), made by
  `editors/vscode/scripts/stage-server.mjs`; a setting is needed only for a development tree.
  `npm run package -- --target <platform>` makes the platform's `.vsix` (about 7 MB).
* CI has two new jobs: `vscode` (typecheck, lint, test, build on Linux) and `package` (release
  `sudsy`, published worker, staged server, `.vsix` as an artifact for `linux-x64` and
  `win32-x64`, after a smoke test that the staged binary formats a C# file without
  `SUDSY_CSHARP_WORKER`). macOS and arm64 packages and Marketplace publishing wait for the
  release work; the Marketplace needs the GitHub org and publisher decisions that plan.md still
  lists as open.

### Speed with a warm worker (same machine and method as "M2c", release builds)
`tests/conformance/speed.sh` for CSharpier and the cold CLI (a new process per run, 3 runs), and
`perf-bench csharp DIR [PASSES]` (`tools/perf-bench/src/warm.rs`) for the warm worker: it formats
the same files with the `csharpier-compat` preset and the safety check on all 12 cores, several
times in one process on one worker pool, which is what a session of `sudsy lsp` sees after its
first file. The machine ran about 15% slower than on the M2c day (CSharpier took 42 - 43 s on the
corpus instead of 37 - 38 s), so compare ratios.

| | CSharpier | Sudsy CLI, cold | Sudsy, warm worker |
| --- | --- | --- | --- |
| CoreLib (1679 files, 27.8 MB) | 17.3 - 17.8 s | 3.8 - 3.9 s | 2.4 - 2.6 s (first pass 3.7 s) |
| full corpus (7659 files, 89.8 MB) | 42.3 - 43.3 s | 9.7 - 10.3 s | 8.0 - 8.1 s (first pass 9.8 s) |

Against CSharpier that is 4.5x (CoreLib) and 4.3x (corpus) cold, as in M2c, and 7.3x and 5.3x
warm. The first pass in the process is the worker's JIT warming up. A warm worker gains 30% on
CoreLib and 20% on the whole corpus: Roslyn's parse, the mapping and the formatter's own work
dominate, not start-up.

What an editor sees (one thread, one file at a time, warm; 4 files with syntax errors count):

| | files | total | per file, median | p95 | p99 |
| --- | --- | --- | --- | --- | --- |
| CoreLib | 1679 | 12.4 s | 1.5 ms | 25 ms | 117 ms |
| full corpus | 7659 | 38.0 s | 1.4 ms | 15 ms | 67 ms |

This includes the worker round trip, the safety re-parse and the layout, but not the editor's own
round trip. An ordinary file takes a few milliseconds; the slowest 1% are large generated files.

### Not done
* Range formatting; pull-diagnostic result ids
  (every pull recomputes from the memoized queries); (`UpdateText`, the workspace and deep diagnostics in the editor: see M5);
  Marketplace publishing, macOS and arm64 packages; a self-contained worker.

## M5: Semantics and flow

### Crates and layering
* `core-flow` (new): the flow graph model, the generic worklist solver (forward or backward, any
  lattice, with a hook for what flows along an edge) and the analyses reachability, liveness (dead
  stores), reaching definitions and definite assignment. `core-semantics` (new): capability
  layers (`Layer`, `Layers`; `provider_api::Capabilities` is now an alias of `Layers`), the facts
  a provider answers about a file (`Answers`, `Declaration`, `Reference`), symbol-id helpers and
  `BehaviorSummaries`. Both are language neutral and are also the worker's wire payload, so each
  message body has one definition on the Rust side.
* The provider trait gained `open_workspace`, `semantics(file, layers)`, `summaries()` and
  `compiler_diagnostics` (the last only for the compiler-agreement gate). plan.md's separate
  `semantic()` and `flow()` accessors became these methods: with one worker behind both, two
  oracle objects would only forward to the same proxy.
* `RuleMeta` has `uses` next to `requires`: layers a rule works better with but does not need.
  `unreachable-code` uses `Flow` and falls back to the statement after a jump at `fast`, so plan.md's
  "reports statements after `return` in fast mode and adds constant conditions and non-returning
  calls in standard mode, without duplicates" is one rule with one code path per mode.
* `FileContext.semantics` and `BodyContext.flow` carry the data. `lint(file)` reads a new
  `semantics(file)` query, which asks the provider for exactly the layers the file's active rules
  `require` or `use` that the depth allows. `fast` therefore never touches the worker's model
  (tested), a `standard` run with no semantic rule enabled asks for nothing, and the query is keyed
  on text and version so an edit re-asks only for that file.

### Worker protocol v3
* `Query` is typed now (`items` plus `diagnostic_ids`) instead of an opaque batch. Each layer is
  answered by one item: Binding -> `unused_imports`, Types -> `declarations` (async, task-returning,
  implements, override ... flags), Values -> `maybe_null`, Project -> `references`. Answers are
  file-wide enumerations, not point queries (`symbol_of(span)`): no rule needs a point query, and
  enumerating avoids a round trip per node. `FlowGraphs` returns `BodyGraph`s. The plan's
  "encoded like the syntax tree" was not done: graphs are plain MessagePack arrays of small structs.
* The proxy sends `UpdateText` with every semantic request (`WorkerProxy::analyze`: update, query
  and flow graphs are one uninterrupted exchange on the first worker). The worker ignores an update
  whose text it already has, so the CLI and the editor use the same path, and a restarted worker
  needs no replay except `OpenWorkspace`, which the proxy remembers and resends.
* Semantic state lives in the first worker only; parsing still uses the pool. Semantic requests
  therefore run one at a time.

### The workspace (the main simplification)
* **Not `MSBuildWorkspace`.** The worker builds one `CSharpCompilation` from the `.cs` files under
  the opened roots (a directory, the directory of a project, or the project directories listed in
  a solution; `bin`, `obj`, `node_modules`, `.git` skipped) plus the text it was sent, against the
  runtime's own reference assemblies. No project references, NuGet packages, target frameworks,
  source generators or `#if` symbols are evaluated. Reasons: MSBuild loading is the plan's own
  risk for this layer, needs the SDK at run time and fails on many setups, and the rules built now
  need little of it. What degrades without a real project: package types do not resolve, so the
  compiler reports errors and `remove-unused-usings` stays silent for the file (its answer is
  withheld whenever the file has errors or `#if` regions, because unresolved or inactive code
  might be the user). Flow graphs hardly depend on resolution. `MSBuildWorkspace` with the
  syntax-only fallback of plan.md can replace `Workspace.cs` without changing the protocol.
* CLI: the workspace is the paths on the command line (else the working directory). LSP: the
  workspace folder. Files outside it are still analyzed on their own.

### Flow export and analyses
* The exporter converts Roslyn's `ControlFlowGraph`: blocks of simple operations (local read,
  write, read-write, declare; calls with the callee's symbol id and attribute flags; one `Other`
  mark per root operation so that every statement is placed), edges (always, true, false) and the
  constant value of a branch condition. Exception flow: every block of a `try` gets an edge to each
  handler start. Locals used by lambdas or local functions are flagged `CAPTURED` and left out of
  dead-store analysis, as are locals whose address is taken. Parameters are not tracked (`out` and
  `ref` parameters are visible to the caller).
* Unreachable blocks stay in the export (Roslyn keeps them and marks `IsReachable`); the core
  recomputes reachability from the edges, constants and summaries.
* Rules. `unreachable-code` reports the first statement of a run, and only when none of the
  statement's operations can run (so `return c ? throw e : 0;` and the condition of a `do` that
  always returns are not reported); the fix is suggestion-only. `unused-variable` is the compiler's
  CS0168 (nothing refers to the variable) and CS0219 (only constants stored, never read) for
  declaration-statement and catch variables, with a safe fix that removes the declaration or the
  catch name. `unused-assignment` is the dead-store rule on liveness (off by default).
  `remove-unused-usings` uses the compiler's CS8019 (off by default, safe fix).
  `possible-null-dereference` reports receivers whose nullable flow state is maybe-null (off by
  default: it repeats CS8602). `unused-private-member` is the first project-scope rule (deep).
  `async-suffix` now asks the type layer whether the method really returns a `Task`/`ValueTask`
  and whether it implements an interface member; at `fast` it keeps the name-based judgment.
  `no-empty-catch` stays syntactic: nothing about it depends on binding.
* Not built: `var-preference` and `prefer-collection-expression` (in plan.md's rule table but not in
  the M5 row), and a rename fix for `async-suffix` (a fix is a set of edits in one file; a rename
  changes every file that uses the method).
* **Summaries.** `does_not_return` comes from `[DoesNotReturn]` (the exporter flags it), a
  built-in list (`Environment.Exit`, `Environment.FailFast`) and `csharp: semantics:
  does_not_return:` in the config (qualified names, every overload; in the JSON Schema). The
  compiler uses none of these, so `unreachable-code` has the option `non_returning_calls` (default
  true); false reports exactly what the compiler does, which is how the agreement gate runs.
  `pure` is read from `[Pure]` and the config and offered as `BehaviorSummaries::is_pure`, but no
  fix uses it yet (no built-in fix deletes a call); `must_use_result` is not implemented.
* `core-flow` also has reaching definitions and definite assignment with unit tests, as plan.md
  lists them; no rule uses them (the compiler already reports CS0165).

### Depths and the editor
* `fast`: syntax only. `standard` (the default): binding, types, values and flow within the file,
  against the workspace's other files. `deep`: adds the `Project` layer (references) and the
  project phase.
* CLI: `lint`, `fix` and `check` print one stderr warning per distinct reason when the semantic
  model is unavailable (worker crash, refusal) and run the other rules.
* LSP: the workspace folder is the worker's workspace; diagnostics are pulled as before (the
  client's own debounce is the idle trigger, the server adds none, and a pull re-sends only changed
  text). With `analysis: deep` the first pull reads the project's other files from disk (the
  config's include and exclude, once per session or config reload) and the project phase runs over
  all of them; a pull returns the project-scope findings for that file. Closed files are not
  watched, so an on-disk edit of one is seen after the next config reload or restart. The reason
  the semantic model is unavailable is shown once.

### Gate: flow rules against the compiler
* `crates/lang-csharp/tests/compiler_agreement.rs` runs both on a 36-method fixture (jumps, loops,
  switches, try/finally, labels, lambdas, local functions, iterators, patterns, `using`,
  deconstruction, out variables) and requires the same set of CS0162, CS0168 and CS0219 positions.
  Writing it found two bugs (statements with no tracked operation; `do { return } while (b)`).
* The corpora are warning-clean code, so they contain almost no CS0162/CS0168/CS0219 for the rules
  to agree on: the corpus run mainly shows that the rules produce no false positives.
  `perf-bench semantic DIR [LIMIT]` runs it and prints the cost per depth.
* Corpus result (the three conformance corpora, standard depth, rules `unreachable-code` with
  `non_returning_calls: false` and `unused-variable`): 0 differences in aspnetcore (3122 files) and
  Roslyn (2858 files); in CoreLib (1679 files) one CS0219 the compiler reports and we do not
  (`PortableThreadPool.GateThread.cs`: a struct local assigned `default` whose only use is reading
  a property of it; the compiler does not count that as a use, our graph does). Known and left.
  Two behaviors came from the corpus: the compiler never reports an unreachable `throw` (a
  defensive guard) or an empty statement, and a body for which Roslyn cannot build a graph (it
  has unresolved symbols) falls back to the syntactic check, which now makes the same exceptions.

### Cost of the semantic layers (release build, 12-core machine, one file at a time, `perf-bench semantic`)
All rules of this milestone on; the whole tree is one workspace; "first file" includes reading and
parsing every file of the tree and the first compilation. Semantic requests run one at a time in
one worker (parsing still uses the pool), so these are single-thread numbers.

| tree | files | fast | standard | deep | first file | standard median / p95 per file |
| --- | --- | --- | --- | --- | --- | --- |
| CoreLib | 1679, 27.8 MB | 5.1 s | 56.1 s | 57.1 s | 2.5 s | 16 ms / 110 ms |
| Roslyn | 2858, 39.6 MB | 6.1 s | 77.3 s | 82.6 s | 3.2 s | 11 ms / 72 ms |
| aspnetcore | 3122, 22.4 MB | 4.5 s | 74.1 s | 85.4 s | 3.6 s | 12 ms / 42 ms |

`standard` costs 10 - 15 times `fast`: Roslyn's binding and its flow analysis of every body dominate;
the exchange itself (text out, graphs and answers back) is a small part. `deep` adds 2 - 15% for
the references and the project phase (134, 143 and 265 project-scope findings). For the editor
the per-file median of 10 - 16 ms is what a pull after an edit costs once the workspace is loaded.
A CLI run does one request at a time on the first worker; making the worker answer semantic
requests in parallel (or one workspace per core) is the lever if CI time matters.

### Not done
* `MSBuildWorkspace` and project/NuGet-aware binding; `var-preference`,
  `prefer-collection-expression`; the `pure` summary has no consumer and `must_use_result` is not
  implemented; closed-file watching for deep diagnostics in the editor; call-graph rules and
  point queries (`symbol_of`); the flow-graph wire encoding of the syntax tree is plain
  MessagePack; the Wasm flow API is M6.

## M6: Extensibility

### Pattern rules (`core-patterns`)
* **A pattern rule is a `core_rules::Rule`.** `core-patterns` compiles an ast-grep-style YAML file
  into `PatternRule`, which implements the same trait as a built-in rule, so levels, options
  checks, suppression comments, the fix loop, `sudsy rules`, SARIF and the editor need no second
  path. Rules from pattern files and plugins reach the engine as `core_rules::Extensions` (rules
  plus the language each runs on), which `languages::Project::load` builds from the config's
  `plugins:` and the front ends hand to the database as one salsa input (`set_extensions`; an
  unchanged set invalidates nothing). `active_rules` chains the language's extension rules after
  the provider's and the shared ones.
* **Rules are leaked.** `Rule::meta` returns `&'static RuleMeta` and the engine holds
  `&'static dyn Rule`; loaded rules are leaked once per distinct file text (a cache keyed by the
  text), so reloading an unchanged config in the editor costs nothing and keeps rule objects equal.
* **File format.** One rule or several (`---`) per file: `id`, `language`, `message`, optional
  `severity` (the default level, `warn`), `category` (`style`), `description`, `example`, `rule`,
  `constraints`, `fix`, `fix-title`, `fix-kind` (`unsafe` when omitted: a template is not known to
  preserve meaning; `safe` and `suggestion` are opt-in). Unknown keys are errors. The `rule` has
  `pattern`, `kind` (node kinds of the grammar), `regex` (on the node's text without trivia),
  `inside`, `not-inside`, `has`, `not-has`, `all`, `any`, `not`; keys at one level all must hold.
  `constraints` check what a metavariable captured (`regex`, `not-regex`, `kind`). Messages and
  fixes substitute `$X` / `$$$ARGS` with the captured source text.
* **Rule ids** are used as written in the file (`team/x` is allowed). An id that is a built-in
  rule's, or one defined twice for a language, is a config error naming the file. Because pattern
  ids are now known when the config is validated, `validate_rule_ids` checks `a/b` ids too
  (M3b skipped them).
* **Patterns are parsed by the language's own provider** (`provider_api::ProviderSnippets`). A
  metavariable is first written as an identifier (`__SUDSY_S_X__`, `__SUDSY_M_X__`), or as a string
  when that does not parse (JSON). The snippet is tried as it is, with a `;` after it, and inside
  `{ }` (a JSON member), and the node spanning all of it is the pattern. The pattern tree is then
  converted to a small matcher tree without trivia. A node whose text is only a metavariable
  (optionally with its `;`) is the metavariable, so in statement position a run of statements is
  written `$$$BODY;`. A repeated name must capture equal text; `$$$` matches any run of siblings,
  separators included, by backtracking. Patterns are compiled when the config loads, so a C#
  pattern file starts the worker then, and a bad pattern is a config error with its file.
* **Dispatch.** The rule's `kinds()` is the root kind of its pattern (or the union for `any`, or
  `kind:`); an expression that pins nothing down is offered every node kind.
* **Not done.** `type_matches` (plan.md's semantic predicate): the worker answers file-wide
  enumerations, not point type queries, so it needs a protocol addition (`type_of(span)`); it waits
  for a milestone that adds point queries. `inside`/`has` drop their captures, and a pattern rule
  has no options.
* **Test harness** (`provider_api::testing`): `lint_with` / `fix_with` take the extension rules,
  `pattern_rules` / `pattern_error` load a pattern file, and `expect` checks a fixture whose
  expected diagnostics are written under the line they point at (`// ^^^ rule-id`, `#` in YAML;
  carets give the span's length when it is on one line). plan.md's "inline annotations" fixtures
  from M3b's not-done list.

### Wasm plugins (`wit/rules.wit`, `core-plugins`, `plugin-sdk`)
* **Interface.** `sudsy:rules@0.1.0`, one export `rules` with `info`, `check`, `collect`, `finish`
  (node, body and file scope in `check`; the two project phases). Unstable until v1; the plugin's
  `info().api` must equal the host's `rules@0.1` or the plugin is refused with a rebuild hint.
  Everything that is not simple (the tree, semantic answers) crosses as bytes in a format the core
  already has, so the WIT stays small: the tree is `core_syntax::wire` (kinds and varints, token text
  not sent), semantic answers are the MessagePack of `(FileSemantics, BehaviorSummaries)`
  (`core-semantics` types, now `Serialize`/`Deserialize`), options are a typed variant list.
* **Semantic queries are not host functions.** plan.md says "semantic queries are host functions,
  batched and cached". The worker already answers a file's layers in one batched exchange
  (`provider.semantics(file, layers)`), cached by the query engine, so the host sends the answers
  with the file instead of letting the plugin call back. The plugin therefore needs no imports at
  all and cannot make the host do work.
* **The flow API is the core's.** The SDK links `core-flow`: the plugin receives the same graphs the
  built-in rules read and runs the same solver and analyses (`plugin_sdk::flow`,
  `BodyContext::flow`). Likewise the pattern matcher is `plugin_sdk::patterns`; compiling a pattern
  needs a parser for the language, which a plugin does not have for C#, so v0 exposes matching and
  compilation against a `SnippetParser` the plugin supplies (JSON and YAML parsers do not link into
  plugins); a host import for parsing snippets is the follow-up if plugin authors ask.
* **One rule source, built in or Wasm.** `plugin-sdk` re-exports `core_rules::Rule` and its
  contexts, and the plugin side of the glue (`guest.rs`) runs the file through the core's own
  `run_file` / `run_project` with the plugin's rule as the only active rule. So a rule written
  against the SDK compiles natively (tests, or built in) and to Wasm without change, and
  `export_plugin!(name: "team", rules: [("csharp", &MyRule)])` is all the glue an author writes.
  `core-rules`, `core-config`, `core-flow` and the grammars compile to `wasm32-wasip2` as they are.
* **Host.** `wasmtime` with the component model; the guest is built with
  `--target wasm32-wasip2`, which imports a few WASI interfaces for `std`, so the linker has
  `wasmtime-wasi` with an empty context (no preopened directories, environment, arguments or
  inherited streams: no files, network or clock). Each call gets a fresh instance (the component is
  compiled once per distinct bytes), 2e9 fuel, 10 s of wall-clock time (an epoch ticker thread
  advances every 50 ms) and 512 MiB of memory. The tree is encoded and copied once per call, and a
  call is one rule on one file; with several rules in a plugin that is several copies of the same
  tree, accepted for v0 to keep the rule interface a plain `Rule`.
* **Failure.** A trap, an exhausted limit, an error the plugin returns, a span outside the file or
  not on a character boundary, or an unknown fix kind disables the whole plugin for the rest of
  the run and reports one diagnostic at the start of the file that hit it ("plugin `x` is disabled
  for this run: ..."), under the rule that was running. A reloaded config keeps the same plugin
  object when the bytes are unchanged (so the editor keeps it disabled until the file is rebuilt).
* **Names and ids.** The plugin's name is its prefix: `info().name` plus `/` plus the rule's id,
  checked for letters, digits, `-`, `_`. A rule's language is `csharp`, `json`, `yaml` or `*`. Rule
  options come from the plugin's declarations, so `lint.rules.team/x.options` is checked like a
  built-in's; the options JSON Schema of plugin rules is not in the generated schema (the schema is
  generated without loading plugins).
* **Grammar hash.** The host sends the hash of the grammar of the file's language; a plugin built
  against another grammar returns an error instead of reading kinds that mean something else.
* **Testing.** `plugin_sdk::testing` (feature `testing`, native) re-exports the harness the built-in
  rules use (`expect`, `lint_with`, `fix_with`) and gives a real provider. The host is tested
  against `tests/plugins/sample`, a plugin with a node rule (options, fix), a flow-graph body rule
  and a project rule, which the tests build with `cargo build --target wasm32-wasip2` (CI installs
  the target; `provider_api::testing::sample_plugin` builds it once per test process). A fourth rule
  misbehaves on request to test the limits. The sample is its own workspace (`exclude` in the root
  manifest) and its own tests run natively with the harness.
* **Cleanup.** `run_file` lost its `body_kinds` parameter: the kinds are a field of `FileContext`,
  where a plugin also needs them. `Db`'s note that the plugin host "will make the provider set
  dynamic" is replaced by the `ExtensionSet` input.

### The v0.1 release (prepared, not performed)
* **Version.** One number for the Cargo workspace, `editors/vscode/package.json` and its lock file
  (0.1.0); `scripts/check-version.sh [vX.Y.Z]` fails when they or the tag disagree or
  `CHANGELOG.md` has no section for the version, and the release workflow runs it first. The sample
  plugin stays 0.0.0 (it is a test fixture).
* **Workflow** (`.github/workflows/release.yml`, on a `vX.Y.Z` tag): version check, then all of
  `ci.yml` (made reusable with `workflow_call`; the "all earlier gates still green" requirement),
  then one build job per platform on a native runner: linux, macOS and Windows, x64 and arm64.
  Each makes a CLI archive (`scripts/package-cli.sh`: the executable, `csharp-worker/` beside it,
  LICENSE, CHANGELOG) and the platform's `.vsix`, after the smoke test that the staged binary finds
  its worker. Last, a job drafts a GitHub release with the archives and `SHA256SUMS`, notes taken
  from the changelog section. Pre-release suffixes (`-rc.1`) make a pre-release.
* **Publishing stays manual.** The workflow publishes nothing by itself. The `marketplace` job runs
  only from a hand-started run on a tag with its input ticked, with the `VSCE_PAT` secret. There is
  nothing for crates.io (every crate is `publish = false`; the SDK is used by path until the plugin
  API is stable) or NuGet (the worker ships inside the packages).
* **Open before publishing** (plan.md lists them as still open): the GitHub organisation and the
  Marketplace publisher id (`package.json` says `sudsy`), contributor sign-off (DCO or CLA), and
  the copyright line of `LICENSE` ("The Sudsy authors" is a placeholder). The macOS and arm64
  builds are untested on real hardware until the first run of the workflow.
