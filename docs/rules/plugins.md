# Wasm plugins (API v0, unstable)

Rust rules compiled to a Wasm component, using the same `Rule` trait as built-ins. Host: `core-plugins` (wasmtime, component model). Interface: `wit/rules.wit`, `sudsy:rules@0.2.0`; a plugin in another language needs only that file. The API changes without notice until v1; a plugin built for another API version or grammar is refused with a rebuild hint.

## Authoring
```toml
[lib]
crate-type = ["cdylib"]
[dependencies]
plugin-sdk = { path = "..." }
[dev-dependencies]
plugin-sdk = { path = "...", features = ["testing"] }   # run rules natively with the harness
```
```rust
plugin_sdk::export_plugin!(name: "team", rules: [("csharp", &NoDirectDateTime)]);
```
Build: `cargo build --release --target wasm32-wasip2`, list the `.wasm` under `plugins: wasm:`. The rule id is `team/no-direct-datetime`. Complete example: `tests/plugins/sample` (node rule with options and fix, flow-graph body rule, project rule, plus a misbehaving rule for limit tests). A rule's language is `csharp`, `json`, `yaml` or `*`; names are letters, digits, `-`, `_`.

## Interface
- One export `rules`: `info`, `check` (node, body, file scope), `layout`, `collect`, `finish` (project phases). The plugin needs no imports and cannot make the host do work.
- Data crosses as bytes the core already has: the tree is `core_syntax::wire`; semantic answers are the MessagePack of `(FileSemantics, BehaviorSummaries)`, sent with the file (no host-function queries; the worker already answers a file's layers in one cached exchange); options are a typed variant list.
- The host sends the file language's grammar hash; a plugin built against another grammar errors.
- SDK: re-exports `core_rules::Rule` and contexts; `plugin_sdk::flow` is the core's flow graph and solver; `plugin_sdk::patterns` is the matcher against a `SnippetParser` the plugin supplies (JSON/YAML parsers do not link into plugins). Guest glue runs the core's own `run_file`/`run_project` with the plugin's rule as the only active rule. Core crates compile to `wasm32-wasip2` as they are.
- Plugin rule options come from the plugin's declarations and are checked like built-ins'; they are not in the generated JSON Schema (it is generated without loading plugins).

## Layout directives
A rule that overrides `Rule::layout_role()` (`Also`/`Only`, declared in `rule-info.layout`) and calls `ctx.attach(&node, Directive::BreakAll)` in `check` attaches layout; the host calls `layout` (the SDK glue runs the core's `attached` with the rule as the only active rule) when it formats a file, and the requests (`kind`, byte range, directive text) go through the same table, levels, suppressions and conflict errors as pattern rules'. `Only` rules are not called when linting. Same limits and failure handling as `check`; a failure fails the file's formatting with the plugin's message. Example, "builder chains on `services` break one call per line":
```rust
struct ServiceChains;
static META: RuleMeta = RuleMeta { id: "service-chains", /* category, description, ... */ };
impl Rule for ServiceChains {
    fn meta(&self) -> &'static RuleMeta { &META }
    fn layout_role(&self) -> LayoutRole { LayoutRole::Only }
    fn kinds(&self) -> &[SyntaxKind] { /* [kind_of("csharp", "InvocationExpression")] */ }
    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let r = significant_range(node, ctx.file.grammar);
        if ctx.file.text[usize::from(r.start())..usize::from(r.end())].starts_with("services") {
            ctx.attach(node, Directive::BreakAll);
        }
    }
}
```
Complete example: `sample/break-arrays` in `tests/plugins/sample`.

## Sandbox and failure
- Empty WASI context: no files, network, environment, stdio. The clocks and random can be read, but the two WASI calls that wait (`wasi:io/poll.poll` and `pollable.block`, which every sleep ends in) trap, because fuel and the epoch only run while Wasm does and a host-side wait would hang the run. Per call: 2e10 fuel (~10 s of work), 10 s wall clock (epoch ticker every 50 ms). Per instance: 64 MiB of memory and 100,000 table elements. Calls run on pooled instances (about one per working thread; a trapped instance is dropped).
- A trap, exhausted limit, returned error, span outside the file or off a character boundary, or unknown fix kind disables the whole plugin for the run and reports one diagnostic at the file start ("plugin `x` is disabled for this run: ..."). A reload with unchanged bytes keeps the same (disabled) plugin object.
- The compiled component is cached in memory by byte hash and on disk by wasmtime (`SUDSY_WASM_CACHE`, default `<user cache dir>/sudsy/wasm`, `off` disables). The cache holds native code that wasmtime loads without authentication, so it is trusted like the user's own binaries: the directory comes only from the user's environment (`SUDSY_WASM_CACHE`, `XDG_CACHE_HOME`, the platform default), never from a repo's config, and must not be shared or restored from untrusted jobs (a CI cache a fork PR can write). A call is one rule on one file: the tree is encoded and copied per call, so a plugin with many rules pays the copy per rule.

## Testing
`plugin_sdk::testing` (feature `testing`, native) re-exports `expect`, `lint_with`, `fix_with` and a real provider. Host tests build the sample with `cargo build --target wasm32-wasip2` (`provider_api::testing::sample_plugin`, once per test process). The sample is its own workspace (`exclude` in the root manifest).
