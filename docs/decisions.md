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
  only the candidate's encoding of the mapped tree. One warm-up pass, then one measured pass over
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

Only (d) is in production code. The other encoders live in `workers/csharp/Sudsy.Bench` and
`tools/wire-bench`, which are marked as benchmark code and are not part of the product.

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
