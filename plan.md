# Sudsy — Initial Implementation Plan

Nov 1, 2026 · @Maia

## Goals and scope

v0.1 ships one tool (CLI plus VS Code extension) that formats, lints and fixes C#, JSON and YAML from a single config, with every cross-language concern in a shared Rust core.

C# is the target language and stresses every part of the design: an out-of-process parser, semantic rules, Roslyn analyzers. JSON and YAML are native Rust providers that prove the provider interface is genuinely language-neutral, and they are the files teams already keep next to C# (appsettings.json, CI pipelines).

**In scope for v0.1**

- Formatting for C#, JSON (including JSONC) and YAML, with every layout decision exposed as an option.
- Lint rules with diagnostics and safe/unsafe fixes, including style-cleanup rules for C#.
- Semantic C# rules: binding, types and values from Roslyn; control and data flow computed in the shared core from a provider-exported flow graph, with an incremental engine for the editor.
- One config for all three languages, organized by language, with short rule ids (no language prefixes).
- Declarative pattern rules written by users, with fixes.
- Custom rules in Rust compiled to Wasm (plugin API v0, marked unstable).
- Presets for published style guides, plus a CSharpier-compatible preset as a migration path.
- CLI for local use and CI (text, JSON, SARIF output).
- LSP server and VS Code extension: diagnostics, quick fixes, format on save.

**Not in v0.1**

- A native C# parser (Roslyn stays the only C# parser; the design keeps the door open).
- Plugin SDKs for languages other than Rust (the raw WIT interface is documented).
- Range (selection) formatting: stretch goal.
- Refactorings beyond rule fixes, type checking, compiler diagnostics.
- Editor extensions other than VS Code (other editors can use the LSP server directly).
- Hosting third-party Roslyn analyzers. Teams keep running them in their IDE and build. Backlog: a migration aid that maps common StyleCop and .NET analyzer rule ids, with their `.editorconfig` severities, to our rules.

## Architecture

A Rust core owns everything that is not inherently language-specific; each language contributes a thin provider, and only C# parsing and semantic queries run outside Rust, in a .NET worker.

&#91;embedded content: architecture · shared core, three providers, one .NET worker\]

Everything above the provider interface is shared; the C# provider's Rust half holds the format spec and rules, and the worker only parses and answers semantic queries.

**Separation principles**

1. **No parser types cross the provider boundary.** The core sees only our syntax tree, defined by our grammar descriptions. Roslyn's `SyntaxKind`, trivia model and object identities stay inside the worker.
2. **Shared by default.** A capability goes into a provider only if it cannot be expressed generically: grammar, parser, formatting spec and hooks, semantic answers, presets.
3. **Rules are Rust, even for C#.** Built-in C# rules live in the Rust `lang-csharp` crate and use the semantic oracle when they need types. The worker never runs rules at all; it only parses and answers semantic queries.
4. **One diagnostics and fix model.** Built-in rules, pattern rules and Wasm plugins all report into it, so fixes conflict-resolve the same way.
5. **Address nodes by position.** Anything that refers to a node across a process or plugin boundary uses `(file, version, span, kind)`, so a second parser can be added later and matched against Roslyn's semantic model.
6. **Every boundary is versioned.** The grammar, the tree encoding, the worker protocol and the plugin WIT interface each carry a version checked at handshake.

## Shared syntax tree and grammar descriptions

Every language is converted into one lossless, untyped tree format whose shape is defined by our own grammar description for that language, never by the parser that produced it.

**Tree model**

- Nodes carry a kind (a 16-bit id namespaced per language), children, and a total text width; tokens carry their text. The model follows rust-analyzer's `rowan` green/red tree split: immutable shared green nodes, cheap red cursors with parent pointers and offsets.
- Lossless invariant: printing the tree reproduces the source byte for byte. Every conformance test checks it.
- Trivia (whitespace, newlines, comments, preprocessor directives, disabled code) are tokens too. Trivia can hold a structured subtree, which covers C# XML doc comments and directives without special cases in the core.
- Error recovery produces `ERROR` nodes. Guarantee: every token is somewhere in the tree, and rules and the formatter skip subtrees containing errors unless they opt in.

**Trivia attachment (one rule for every language)**

- Trailing trivia: everything after a token up to and including the end of its line.
- Leading trivia: everything else before a token.
- Providers normalize to this rule. Roslyn's rule is close but not identical, so the worker re-attaches trivia during conversion.

**Grammar descriptions** (`grammars/<lang>.grammar`)

- Node kinds, token kinds, and named child fields (`MethodDecl = modifiers:Modifier* return_type:Type name:Ident params:ParamList body:Block?`), similar to rust-analyzer's ungrammar.
- **Category tags** on kinds: `block`, `statement`, `expression`, `delimited-list`, `separator`, `string`, `comment`, `key-value`, and so on. Tags are what let the formatter, rule engine and pattern engine work generically across languages.
- Comment syntax and suppression-comment markers per language.
- A content hash of the grammar is its version; the worker and plugins must agree on it.

**Code generation**

A build step generates typed Rust wrappers from each grammar (`cs::MethodDecl::params()`, `yaml::Mapping::entries()`). Built-in rules, format hooks and the plugin SDK all use the same generated code.

**Wire encoding**

Trees cross process and plugin boundaries once per file, never per node. Token text is not sent: the receiver already has the source, so each element needs only its kind, its width in bytes and its child count. The encoding is **decided by a benchmark in M0**: encode real Roslyn trees from a pinned dotnet/runtime commit as (a) flat arrays, (b) MessagePack per element, (c) protobuf packed arrays, and (d) flat arrays as raw byte fields inside the existing MessagePack message. Measure size, C# encode time and Rust decode time against Roslyn's parse time, and pick the simplest option whose overhead stays under 10% of parsing.

## Provider interface and C# worker protocol

Every language implements one Rust trait; an out-of-process language like C# implements it through a proxy that speaks a versioned binary protocol to its worker.

```rust
pub trait LanguageProvider: Send + Sync {
    fn id(&self) -> LanguageId;
    fn grammar(&self) -> &Grammar;                       // kinds, fields, category tags
    fn file_matchers(&self) -> &[FileMatcher];           // extensions, known file names, dialects
    fn capabilities(&self) -> Capabilities;              // binding, types, values, flow, project
    fn parse(&self, file: &SourceFile) -> ParseResult;   // shared tree + parse errors
    fn formatter(&self) -> &dyn NodeFormatter;           // per-node code + hooks; tags cover the rest
    fn option_schema(&self) -> &OptionSchema;
    fn presets(&self) -> &[Preset];
    fn builtin_rules(&self) -> Vec<Box<dyn Rule>>;
    fn semantic(&self) -> Option<&dyn SemanticOracle>;    // binding, types, values, project queries
    fn flow(&self) -> Option<&dyn FlowExporter>;         // flow graphs per body; None for JSON, YAML
    fn summaries(&self) -> &BehaviorSummaries;           // built-in facts, merged with user config
}
```

For C#, only `parse`, `semantic` and `flow` are forwarded to the worker. Grammar, formatting code, options, presets, summaries and built-in rules are compiled into the Rust `lang-csharp` crate, and all flow analyses run in the core.

**Worker protocol (v1)**

Transport is the worker's stdin/stdout with length-prefixed frames; payloads use MessagePack, with trees in the encoding chosen by the M0 benchmark.

| Message | Direction | Purpose |
| --- | --- | --- |
| `Hello` | both | Protocol version, grammar hash, flow-graph format version, capabilities. Mismatch = refuse to start. |
| `OpenWorkspace` | core → worker | Solution or project paths and MSBuild properties, for semantic queries. |
| `Parse` | core → worker | File id, version, text, language version → tree + parse errors. |
| `UpdateText` | core → worker | New file text (editor edits) so the semantic model stays current. |
| `Query` | core → worker | A batch of binding, type, value and project queries → answers, in one round trip. |
| `FlowGraphs` | core → worker | A file's method bodies → their flow graphs with symbol ids, types and constants. |
| `Shutdown` | core → worker | Clean exit. |

**Semantic query vocabulary (v0.1)**

Queries are grouped into capability layers (binding, types, values, project) with language-neutral names; see Semantic analysis. Nodes are addressed by `(file, version, span, kind)` and symbols by language-neutral symbol ids.

**Worker lifecycle**

- Started lazily on the first C# file that needs it; never started for JSON or YAML.
- CLI: one worker per run. LSP: one long-lived worker with an incrementally updated Roslyn workspace.
- Workspace loading uses `MSBuildWorkspace`. If a project fails to load, the worker falls back to syntax-only mode and semantic rules are skipped with a warning, never a crash.
- Crash or hang: the core restarts the worker once, then reports C# semantic results as unavailable for that run.

## Formatting engine

One shared engine formats every language: each provider's formatting code turns the tree into a layout document using shared shapes, and a shared Wadler-style printer fits it to the line width.

**Layout document primitives**

`text`, `line`, `softline`, `hardline`, `group`, `indent`, `align`, `if_break`, `fill`, `conditional_group`, `line_suffix` (trailing comments), `break_parent`. The printer is greedy and language-unaware.

**Shared shapes**

Most syntax across languages is a handful of shapes. The core implements each once, with options, and specs compose them:

| Shape | C# example | JSON / YAML example |
| --- | --- | --- |
| Delimited list | argument list, collection expression | JSON array, YAML flow sequence |
| Block | method body, namespace | JSON object, YAML block mapping |
| Key-value | named argument, property initializer | JSON member, YAML mapping entry |
| Binary chain | `a && b \|\| c`, string concatenation | — |
| Member chain | LINQ method chains, builders | — |
| Statement sequence | statements, members | YAML documents, sequence items |
| Verbatim | raw and interpolated strings, disabled `#if` code | YAML block scalars |

**Formatting code (decided)**

Formatting reads the user's config directly, with no separate spec layer. This is the approach Prettier, CSharpier, Biome and Ruff use, plus three additions:

- **Defaults from grammar tags.** A node with no formatting code of its own uses its tag's shared shape (`#block`, `#delimited-list`, …) with the shared options. JSON's provider is expected to be almost empty.
- **Shared shape helpers.** Per-node code, where needed, builds a style from the config and calls a shared shape: `shapes::delimited_list(node, f, ListStyle { … })`.
- **Layout directives.** Each shape merges the config-derived style with any directives that rules attached to the node, so user-defined rules can influence layout (below).

**Hooks** (Rust code for cases data cannot express)

Registered per node kind, can try several candidate layouts and pick the best. Expected in v0.1: C# member chains, lambda as last argument, LINQ query expressions, binary expression trees; YAML flow versus block collections and comment placement inside sequences.

**Layout directives (how user-defined rules affect formatting)**

Rules never print layout themselves; they request it, and the core performs it. A layout rule (built-in, Wasm plugin, or a pattern rule with a `layout:` key) attaches directives to nodes from a fixed, versioned set:

| Directive | Effect |
| --- | --- |
| `BreakAll` | Break every item of a list or chain, one per line, even if it fits |
| `KeepTogether` | Don't break inside this node unless it cannot fit |
| `BreakBefore` / `BreakAfter` | Force a line break at this point |
| `BlankLineBefore(n)` | Require n blank lines before this node |
| `Align` | Align items in a group (for example the `=` in consecutive assignments) |
| `Preserve` | Leave this node exactly as written |
| `Style(…)` | Pick one of the node's built-in variants for this node only |

- All output still passes the meaning-preserved check; plugins cannot change what code does.
- Conflicting directives from different rules on the same node are reported as a config error, never resolved silently.
- Layout rules are syntax-only by default. Semantic layout rules are opt-in, because they make formatting C# require a loaded project.
- The plugin SDK test harness formats each fixture twice and fails if the output changes.
- Returning a full layout document from a plugin is deliberately left out; reconsider only if directives prove too limiting.

**Comments and ignore regions**

- Comment placement (leading, trailing, dangling) is shared code driven by the trivia attachment rule.
- An ignore directive (`sudsy-ignore`, `sudsy-ignore-start/end`) uses each language's comment syntax from its grammar.

**Safety checks (every file, every run)**

1. Re-parse the output and compare trees ignoring trivia. Any difference aborts formatting for that file and reports an internal error.
2. YAML additionally compares the loaded data model, because indentation changes can change meaning.
3. Idempotency (`format(format(x)) == format(x)`) is enforced in tests, not at runtime.

## Rules, diagnostics and fixes

Three rule sources (built-in Rust rules, declarative pattern rules, Wasm plugins) all report into one diagnostics model and one fix engine.

**Rule trait** (shared by built-in rules and Wasm plugins)

```rust
pub trait Rule: Send + Sync {
    type Options: RuleOptions;                  // typed options; schema generated from the struct

    fn meta(&self) -> RuleMeta;                 // id, category, default level, fix kind,
                                                // required capability layers
    // Implement whichever scopes the rule needs; all default to no-ops.
    fn kinds(&self) -> &[SyntaxKind] { &[] }
    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<Self::Options>) {}
    fn check_file(&self, file: &FileContext, ctx: &mut RuleContext<Self::Options>) {}
    fn check_body(&self, body: &BodyContext, ctx: &mut RuleContext<Self::Options>) {}   // flow graph + results
    fn collect(&self, file: &FileContext, facts: &mut ProjectFacts) {}                 // project scope, phase 1
    fn finish(&self, facts: &ProjectFacts, ctx: &mut ProjectContext<Self::Options>) {}  // project scope, phase 2
}
```

- Rule ids are short names (`no-empty-catch`); the config section or the file's language supplies the language. Shared rules (`max-line-length`) apply to every language, and a language-specific rule cannot reuse a shared rule's name. Plugin rules carry their plugin's name: `team/no-direct-datetime`.
- Categories: correctness, suspicious, style, cleanup, performance.
- The engine makes one traversal per file and dispatches nodes to subscribed rules; files run in parallel. File, body and project scopes are described under Semantic analysis.
- Rules declare the capability layers they require and run at whatever analysis depth provides them, degrading gracefully rather than switching off.

**Diagnostics and fixes**

- A diagnostic has a span, severity, message, rule id, related spans and zero or more fixes.
- A fix is a titled set of text edits marked **safe** (applied by `fix`), **unsafe** (only with `--unsafe` or an editor quick fix), or **suggestion-only** (editor quick fix only, never applied by `fix`; for example removing unreachable code, which usually signals a bug). Fixes that delete code check behavior summaries and keep calls with side effects.
- Fix loop: collect fixes, drop overlapping ones by rule priority, apply, re-parse, re-run rules; repeat until nothing changes or 10 iterations. Then format.
- Suppression comments (`sudsy-disable-next-line <rule>`) use each grammar's comment syntax. Plain JSON has no comments, so suppressions there are config-only.

**Declarative pattern rules**

- Patterns are snippets of the target language with metavariables (`$X`, `$$$ARGS`), parsed by that language's own provider, so they match the shared tree with no separate pattern grammar.
- Constraints: node kind, regex on text, `inside` / `not-inside` / `has`, and semantic predicates (`type_matches`) for languages with an oracle.
- Fixes are templates using captured metavariables; the formatter lays out the result.
- Syntax (decided): ast-grep-style YAML rule files, the same format as sudsy.yaml. Both pattern rules and Wasm plugins ship in v0.1: pattern rules for everyday rules without a build step (C# teams need no Rust), Wasm for complex logic. The pattern matcher is shared and also exposed in the plugin SDK, so YAML rules are a thin file format over it.

**Wasm plugins (API v0, unstable)**

- Host: `wasmtime`. Interface: `wit/rules.wit`, versioned as `rules@0.1`.
- The tree is copied into plugin memory once per file in the wire encoding; semantic queries are host functions, batched and cached.
- Each plugin gets a fuel and time limit per file; a plugin that traps or times out is disabled for the run and reported.
- The `plugin-sdk` crate exposes the same `Rule` trait and generated typed wrappers, and a macro generates the Wasm export glue, so one rule source compiles natively (built-in) or to Wasm (plugin).

**Roslyn analyzers are not hosted**

- Teams keep running analyzers where they already run: the IDE and `dotnet build`. Hosting them here would duplicate those warnings, bypass the shared tree and layout directives, and add NuGet restore and analyzer loading to the worker.
- Custom C# rules go through pattern rules and Wasm plugins instead.
- Backlog: a migration aid that maps common StyleCop and .NET analyzer rule ids, with their `.editorconfig` severities, to equivalent built-in rules.

**Initial built-in rules (examples)**

| Language | Syntactic | Semantic |
| --- | --- | --- |
| C# | `no-empty-catch`, `async-suffix`, `file-scoped-namespace` (cleanup), `using-order` (cleanup), `prefer-null-coalescing` | `remove-unused-usings`, `var-preference`, `prefer-collection-expression` |
| JSON | `no-duplicate-keys`, `sorted-keys` (opt-in) | — |
| YAML | `no-duplicate-keys`, `ambiguous-booleans` (`yes`/`no`/`on`), `document-start`, `sorted-keys` (opt-in) | — |

## Semantic analysis

Anything that would force a rewrite if added later is built in v0.1: capability layers, rule scopes, symbol ids, a shared flow graph with its analysis framework, an incremental engine, and room for embedded languages in the tree. Analyses that only add on top (taint, resource tracking, embedded-language detection) come later.

**Capability layers**

Providers declare which layers they support; rules declare which they require. JSON and YAML declare none.

| Layer | Queries | C# answered by |
| --- | --- | --- |
| Binding | `symbol_of`, `declaration_of`, `references` (file or project), symbol info: kind, modifiers, accessibility, attributes, overrides | Roslyn |
| Types | `type_of`, `converted_type`, `is_assignable`, `implements`, `call_target` (resolved overload) | Roslyn |
| Values | `constant_value`, `nullable_state` | Roslyn |
| Flow | Control-flow graph per method body, annotated with symbol ids, types and constants | Exported by the provider; analyses run in the core |
| Project | References across the solution; call graph | Roslyn (references); core (call graph built from flow graphs) |

**Rule scopes**

- **Node:** visit node kinds (existing `check`).
- **File:** line- or file-based rules (existing `check_file`).
- **Body:** called once per method, lambda or local function, with its flow graph and analysis results.
- **Project:** two phases. `collect(file)` gathers facts from each file; `finish(project)` reports across files (unused members across partial classes, unused public API, duplicate code).

**Symbol ids**

Language-neutral strings identify symbols across files and runs, for project-scope rules and cache keys. C# uses Roslyn's documentation comment ids (`M:Shop.Orders.OrderService.GetRecentAsync(System.Int32)`).

**Flow graph and analysis framework**

- Each method body is exported as a control-flow graph: basic blocks of simple operations (assign, read, call, branch, return, throw) carrying symbol ids, types and constant values.
- C# converts Roslyn's operation-based control-flow graph, batched per file and encoded like the syntax tree.
- The core runs a generic dataflow solver (forward and backward, over any lattice). v0.1 analyses: reachability, liveness, reaching definitions, definite assignment. Later analyses (resource-state tracking, taint) are new analyses on the same framework, not rewrites.
- The flow graph and solver are exposed through the Wasm plugin interface, so plugins can define their own flow analyses.
- Nullability stays a Roslyn query; reimplementing C# nullable analysis isn't worth it.

**Behavior summaries**

Per-method facts used by rules and fix preconditions: `does_not_return`, `pure`, `must_use_result`, and, reserved in the schema for later, ownership transfer and taint source or sink. Derived from attributes plus a built-in list for the .NET standard library, and extendable by users:

```yaml
csharp:
  semantics:
    does_not_return: ["MyApp.Log.Fatal"]
    pure: ["MyApp.Money.Add"]
```

**Embedded languages (room reserved)**

The tree model, wire encoding and plugin interface support a node holding a nested tree in another language from v0.1. Providers start detecting regions (regex patterns, JSON in raw strings) later, which then get linted and formatted by that language's provider.

**Incremental engine**

Core computations (parsing, config resolution, semantic requests, flow graphs, rule results) are demand-driven queries memoized by their inputs, in the style of rust-analyzer's salsa. An edit in the editor recomputes only what depends on it. Retrofitting this later would mean restructuring the core.

**Analysis depth**

| Mode | Runs | Used by |
| --- | --- | --- |
| fast | Syntax only | `format --check`, pre-commit hooks, every keystroke |
| standard | Binding, types, values and flow within each file | Editor on idle and save; CI default |
| deep | Adds project-scope rules and, later, taint | `--analysis=deep` in CI |

Rules run at whatever depth their required layers allow and degrade gracefully: unreachable code reports statements after `return` in fast mode and adds constant conditions and non-returning calls in standard mode, without duplicates.

**Built in v0.1 versus added later**

| Item | When | Why |
| --- | --- | --- |
| Capability layers and rule requirements | v0.1 | Shape of the provider interface and rule metadata |
| Rule scopes (body, project) | v0.1 | Shape of the `Rule` trait and plugin interface |
| Symbol ids | v0.1 | Part of the worker protocol and cache keys |
| Flow graph and dataflow framework | v0.1 | Rules built on Roslyn's flow answers would have to be rewritten |
| Incremental engine | v0.1 | Core structure |
| Embedded-language nodes in tree, encoding, plugin interface | v0.1 | Format changes break plugins |
| Fix categories, analysis depth modes | v0.1 | CLI and config surface |
| Taint and resource-state analyses | Later | New analyses on the framework |
| Embedded-language detection | Later | Providers start emitting the reserved node |
| Call-graph rules, more summary kinds | Later | New queries and schema keys |
| Range formatting | Later | Formatter is designed in M1 to format any subtree in its context |
| Formatting inactive `#if` branches | Later | Structured trivia already holds them |
| Native C# parser, non-Rust plugin SDKs, self-contained worker | Later | Conformance suite and WIT already allow them |

## Configuration and presets

One YAML config file at the repository root controls formatting, rules and plugins for every language, with presets as the starting point and `.editorconfig` as a fallback source.

```yaml
# sudsy.yaml (.yml also accepted)
extends:
  - preset:csharp/microsoft@1
  - preset:yaml/default@1

format:
  line_width: 100                 # shared: every language

lint:
  rules:
    max-line-length: warn         # shared rule: every language

plugins:
  patterns: ["rules/*.yaml"]
  wasm: ["rules/team-rules.wasm"]

csharp:
  format:
    line_width: 120               # overrides the shared value for C#
    brace_newline: all
    trailing_commas: multiline
    member_chain_min_calls: 3
  lint:
    rules:
      no-empty-catch: error
      max-line-length: "off"      # shared rule, off for C# only
      team/no-direct-datetime:    # plugin rule: plugin name + rule
        level: error
        options:
          allowed_in: ["**/SystemClock.cs"]

yaml:
  lint:
    rules:
      ambiguous-booleans: warn

overrides:
  - files: ["tests/**"]
    csharp:
      lint:
        rules:
          async-suffix: "off"
```

The config is read with YAML 1.2 semantics, so `yes`, `no`, `on` and `off` are plain strings, never booleans. M0 loads it with an existing Rust YAML crate; once our own YAML provider exists (M1), config files are formatted and linted by the tool itself like any other YAML.

**Resolution order** (highest first): CLI flags → matching `overrides` entries → the language section (`csharp:`, `yaml:`) → top-level shared settings → the `extends` chain → `.editorconfig` (including mapped `csharp_*` and `dotnet_*` keys) → provider defaults. `sudsy config print <file>` shows the resolved result and where each value came from.

**Option schema**

Options are declared once per provider (name, type, default, allowed values, description). From that one declaration the build generates validation, the docs pages, and a JSON Schema the VS Code extension registers for autocomplete in the config file.

**Presets in v0.1**

| Preset | Based on |
| --- | --- |
| `csharp/microsoft` | Microsoft's published C# coding conventions |
| `csharp/dotnet-runtime` | The dotnet/runtime repository's coding style |
| `csharp/google` | Google's C# style guide |
| `csharp/csharpier-compat` | CSharpier's output, for migration with no reformat churn |
| `json/default`, `json/prettier-compat` | Common two-space JSON; Prettier's JSON output |
| `yaml/default`, `yaml/prettier-compat` | Conservative, style-preserving YAML; Prettier's YAML output |

Presets are versioned (`@1`), so an updated preset never silently reformats a codebase. Each one ships a short page listing the interpretation choices made where the guide is silent or vague.

**Zero-config defaults (decided)**

With no `sudsy.yaml`, Sudsy aims for the smallest diff on first run:

- **C#:** the `csharp/microsoft` preset (Allman braces, 4-space indent), line width 120. CSharpier users opt into `csharpier-compat` with one line.
- **Existing `.editorconfig`:** `csharp_*` and `dotnet_*` formatting keys are respected automatically, so a repo already configured for Visual Studio keeps its style.
- **JSON and YAML:** 2-space indent, existing YAML quote styles kept, keys never reordered, short objects and arrays collapsed onto one line only if they already were.

## Language providers

Each provider contributes only a parser, a grammar, a format spec with hooks, rules, options and presets; this section lists what is specific to each.

### C#

- **Parser:** Roslyn (`Microsoft.CodeAnalysis.CSharp`) in a .NET worker targeting the current .NET LTS. The supported C# language version follows the bundled Roslyn version.
- **Mapping layer** (C#, inside the worker): Roslyn tree → our grammar's kinds and fields, with trivia re-attached to our rule. This layer is the only code that knows Roslyn's tree shapes.
- **Hard cases for mapping:** preprocessor directives; disabled `#if` text (kept as verbatim trivia; only the active branch is formatted in v0.1); XML doc comments (structured trivia); raw, verbatim and interpolated strings; top-level statements; file-scoped namespaces; records and primary constructors; collection expressions.
- **Semantics:** `MSBuildWorkspace` loading, with syntax-only fallback. Semantic query answers are cached per file version.
- **Format hooks:** member chains, lambda as last argument, LINQ query expressions, binary expression trees, attribute lists.
- **Distribution:** decided: framework-dependent, targeting the current .NET LTS with roll-forward to newer runtimes. Semantic features need the .NET SDK anyway (project loading uses the SDK's MSBuild). If no runtime is found, Sudsy reports that C# needs .NET installed and keeps working for JSON and YAML. A self-contained build stays on the backlog for environments without .NET.

### JSON

- **Parser:** hand-written in Rust; the grammar is small.
- **Dialects:** JSON and JSONC (comments, trailing commas). Chosen by extension and known file names (`tsconfig.json` and `.vscode/*.json` are JSONC), overridable in config.
- **Format options:** indent, trailing commas (JSONC only), keep short objects and arrays on one line, final newline.
- **Rules:** duplicate keys, optional key sorting with configurable order.

### YAML

- **Parser:** a lossless Rust parser for YAML 1.2. Most existing Rust YAML crates are event- or value-based rather than lossless, so plan to write our own; evaluate existing crates first.
- **Must preserve:** comments anywhere, anchors, aliases and tags, multiple documents, and scalar styles (plain, single-quoted, double-quoted, literal `|`, folded `>`).
- **Format options:** indent, sequence indentation inside mappings (indented or flush), quote preference (preserve, single, double), document markers, maximum blank lines. Block scalar contents are never re-wrapped.
- **Extra safety check:** the loaded data before and after formatting must be equal, in addition to the tree comparison.
- **Rules:** duplicate keys, ambiguous booleans, document start marker, optional key sorting.

## CLI, LSP server and VS Code extension

The CLI and the LSP server are two thin front ends over the same core, and the VS Code extension is a thin client of the LSP server.

**CLI commands**

| Command | What it does | Exit code |
| --- | --- | --- |
| `format [--check]` | Format files; `--check` reports without writing | 1 if any file would change |
| `lint` | Report diagnostics | 1 if any error-level diagnostic |
| `fix [--unsafe]` | Apply fixes, then format | 1 if unfixable errors remain |
| `check` | `format --check` + `lint`, for CI | 1 on any failure |
| `rules [<id>]` | List rules, or explain one with examples | 0 |
| `config print [<file>]` | Resolved config and the source of each value | 0 |
| `init [--preset <name>]` | Write a starter config | 0 |

- Output formats: human-readable, JSON, SARIF, GitHub Actions annotations.
- Cache key per file: content hash + resolved-config hash + tool version + plugin hashes.

**LSP server**

- Rust, built on an established LSP crate (`tower-lsp` or `lsp-server`).
- Pull diagnostics; syntactic rules on every change, semantic rules and analyzers debounced on idle and on save.
- Code actions: quick fix per diagnostic, fix all of a rule in a file, fix all safe.
- Document formatting (range formatting is a stretch goal).
- Watches config files and reloads; keeps one long-lived C# worker with its Roslyn workspace updated through `UpdateText`.

**VS Code extension**

- TypeScript, using `vscode-languageclient`; no logic beyond starting the server.
- Per-platform packages bundle the native binary and the framework-dependent C# worker.
- Registers the config's JSON Schema, so the config file gets autocomplete and validation. VS Code applies schemas to YAML through the YAML language extension, so either declare it as a dependency or have our own LSP server provide completion for the config file.
- Users set it as `editor.defaultFormatter` per language for format on save.

## Testing strategy

Correctness is enforced by four automatic checks on every fixture and corpus file: lossless round-trip, unchanged meaning, idempotency, and a stable expected tree.

| Test type | What it checks | Inputs |
| --- | --- | --- |
| Conformance (per language) | Parser output matches expected tree snapshots and round-trips byte for byte; any replacement parser must pass the same suite | C#: pinned commits of large open-source repos (dotnet/runtime, roslyn, aspnetcore) plus edge-case fixtures; JSON: JSONTestSuite; YAML: the official yaml-test-suite |
| Formatter snapshots | Output per shape and per option value | Hand-written fixtures, snapshot-tested with `insta` |
| Option interactions | Pairwise combinations of options stay idempotent and preserve meaning | Generated from the option schemas |
| Corpus formatting | Idempotency and meaning preservation at scale | The conformance corpora |
| Differential | `csharpier-compat` output equals CSharpier's | C# corpus |
| Rules | Expected diagnostics as inline annotations in fixture files; fix output snapshots | Per-rule fixtures; the same harness ships in the plugin SDK |
| Worker protocol | Contract tests both sides, version-mismatch refusal, crash and restart | Recorded sessions |
| Performance | Throughput and memory regressions, compared with a CSharpier baseline on the same corpus | Benchmarks in CI |

## Repository layout

One Cargo workspace plus a .NET solution for the worker; language crates depend on core crates, never the reverse.

```
crates/
  core-syntax/      tree model, embedded-language nodes, wire encoding, grammar loader, codegen
  core-incremental/ demand-driven, memoized query engine (salsa-style)
  core-format/      layout document, printer, shared shapes, layout directives
  core-rules/       rule engine and scopes, diagnostics, fix loop, suppressions
  core-semantics/   capability layers, symbol ids, query batching, behavior summaries
  core-flow/        flow graph model, dataflow solver, reachability, liveness, definite assignment
  core-patterns/    pattern matcher and YAML pattern rules
  core-plugins/     wasmtime host for Wasm rule plugins
  core-config/      config, presets, .editorconfig mapping, schema generation
  core-workspace/   file discovery, ignore files, scheduling, depth modes
  provider-api/     LanguageProvider trait, out-of-process proxy, protocol types
  lang-json/        parser, formatting, rules, presets
  lang-yaml/        parser, formatting + hooks, rules, presets
  lang-csharp/      formatting + hooks, built-in rules, presets, summaries, worker proxy
  cli/
  lsp/
  plugin-sdk/       public crate for rule authors (Rule trait, typed wrappers, matcher, flow API, test harness)
workers/
  csharp/           .NET worker: Roslyn mapping, semantic queries, flow-graph export
grammars/           csharp.grammar, json.grammar, yaml.grammar
wit/                rules.wit
editors/vscode/
tests/
  conformance/      per-language corpora and expected trees
  fixtures/
```

## Milestones

Seven phases, each closed by a gate that must pass before the next starts. JSON comes first because it exercises the whole core with the smallest parser.

&#91;embedded content: milestones · 7 phases, each closed by a gate\]

M3 and M4 can overlap once the diagnostics model is stable. Durations are not estimated yet.

## Risks and open decisions

The biggest risks are the cost of configurability and the C# boundary; each has a mitigation built into the milestones.

| Risk | Why it matters | Mitigation |
| --- | --- | --- |
| Option explosion | Every option multiplies test cases and creates interaction bugs | Pairwise interaction tests, meaning check on every run |
| Up-front scope | Building the flow framework, incremental engine and capability layers in v0.1 makes the first release larger | Each lands behind a milestone gate; analyses beyond reachability, liveness and definite assignment wait |
| Tree transfer cost for C# | Sending a tree per file between processes could erase the speed advantage | M0 benchmark against Roslyn's parse time; cache by content hash; persistent worker later |
| Flow-graph fidelity | Converting Roslyn's flow graph loses detail or disagrees with the compiler | Gate M5 on agreeing with compiler warnings (CS0162, CS0168, CS0219) across the corpus |
| Roslyn mapping fidelity | `#if` regions, structured trivia and new syntax are easy to get subtly wrong | Conformance corpus with round-trip checks; mapping layer isolated in one place |
| Lossless YAML parser | Large, fiddly piece of work with no reusable crate | Start in M1, gate on the official yaml-test-suite |
| Semantic workspace loading | `MSBuildWorkspace` is slow and fails on some project setups | Syntax-only fallback; rules degrade gracefully; load once per LSP session |
| Too many formatting hooks | Hooks erode the shared-shapes design | Track hook count per language; promote recurring hook patterns into shared shapes |
| Plugin API stability | Published plugins turn the WIT interface, tree and flow-graph formats into a public contract | Ship v0 as unstable; freeze at v1 only after real plugins exist |

**Open decisions**

- [ ] Tool name — decided: sudsy (config file sudsy.yaml). Still needed: a GitHub org name, since the sudsy username is taken; confirm npm, crates.io, NuGet and VS Code Marketplace availability
- [ ] Pattern-rule syntax — decided: ast-grep-style YAML; pattern rules and Wasm plugins both in v0.1
- [ ] Format spec authoring — decided: direct config reads, shared shapes, tag defaults, layout directives
- [ ] Tree wire encoding — decided by the M0 benchmark (see Shared syntax tree)
- [ ] C# worker distribution — decided: framework-dependent, current .NET LTS, roll-forward
- [ ] `#if` handling beyond v0.1: format inactive branches too, or keep them verbatim
- [ ] License — decided: MIT. Still open: contributor sign-off (DCO or CLA)
