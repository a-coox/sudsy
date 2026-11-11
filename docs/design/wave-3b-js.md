# Design: JavaScript / TypeScript / JSX / TSX (wave 3b)

Decided plan (section 5), being built in the steps of section 4; each step moves what it builds into `docs/formatting/js.md`, and this note is deleted when step 14 lands. Numbers marked (measured) come from the spike below; line counts are estimates.

Goal and priority: JS/TS is the language most repositories need, so the goal is a formatter and linter people switch to: Prettier's output, ESLint's core findings, migration from their configs. Speed matters only as "faster than Prettier and ESLint", which the oxc parser already gives; no optimization steps are planned beyond the per-step bench.

## Summary
1. Parser: **oxc_parser** (MIT, pinned exact version) behind a new AST-to-lossless-tree adapter, not tree-sitter. 30-40x faster, 0.8 MB instead of 3.3 MB, passes 30/30 modern-syntax probes where tree-sitter-typescript passes 27/30.
2. One language `js` for `.js .mjs .cjs .jsx .ts .mts .cts .tsx`, one grammar file, ESTree/oxc node names.
3. Layout: `core-format` already has most Prettier doc primitives; it lacks 4 small ones (group-id `if_break`, `label`, `line_suffix_boundary`, a layout cache for "print this node twice") and multi-word ignore markers. The rest (member chains, call-argument hugging, assignment layouts, JSX, ternaries, parens) is lang-js code, about 7k lines.
4. Comments: keep Sudsy's token-owned trivia model (as Biome does) and add targeted relocations; do not port Prettier's attach algorithm.
5. Parity: Prettier's own `tests/format` corpus plus a pinned real-world corpus, differential against the pinned Prettier, ESLint 9 core and typescript-eslint (non-type-aware) as catalogs; syntax-only rules first, scope rules through `oxc_semantic` second.
6. Land in 15 pushable steps; an unimplemented node prints verbatim, so every step is safe on real files. Step 14 migrates Prettier and ESLint configs through the existing `sudsy init --from` importers.

## 1. Parser

Spike (`C:/Coding/.sudsy-targets/slot2/jsbench`, not committed; Windows x64 release, one thread, 5 reps; files: lodash.js 544 KB, TypeScript parser.ts 537 KB and utilities.ts 486 KB, vue renderer.ts 72 KB, excalidraw App.tsx 354 KB, MUI Autocomplete.jsx 39 KB):

| Candidate | Parse speed (measured) | Binary added (measured) | Notes |
|---|---|---|---|
| tree-sitter-javascript 0.25 + tree-sitter-typescript 0.23.2 (js, ts, tsx grammars) | 3.5-7.7 MB/s (lodash 71-97 ms, parser.ts 110-152 ms, App.tsx 80-129 ms) | +3.3 MB (+ `core-treesitter` conversion pass, which was a third of TOML's parse) | last release Nov 2024 |
| oxc_parser 0.158 (tokens off) | 52-161 MB/s (lodash 3.4 ms, parser.ts 4.6 ms, App.tsx 6.7 ms) | +0.8 MB with parser, visitor and semantic | MIT; pre-1.0, a breaking release about weekly |
| oxc_parser with tokens + `Visit::enter_node` walk | parse 3.2-9.3 ms + walk 0.5-1.3 ms per file above | | 31-52 K nodes, 41-62 K tokens, depth 26-42 per 500 KB file |
| oxc_semantic (scopes, symbols, references) | +2-4 ms per file | within the 0.8 MB | for scope rules |
| swc_ecma_parser 47 | not run | not run | Apache-2.0; needs a separate comments store and lexer for tokens, many `swc_*` crates; slower than oxc by reputation, unmeasured |
| biome_js_parser | not run | not run | crates.io has 0.5.7 (2023); the live parser is not published |
| hand-written | n/a | n/a | JS+TS+JSX is 15-20 K lines; the roadmap keeps hand-written parsers for where tree-sitter is too slow, which is not the case yet |

Correctness (measured, 30 probes of 2022-2025 syntax: `using`, `accessor`, `satisfies`, import attributes, regex `v` flag, decorators, static blocks, `#x in o`, variance and `infer extends`, const type parameters, template literal types, `asserts`/`unique symbol`, `export type *`, enums, `declare module`, abstract/override, `?.!`, generic arrows in TSX, JSX namespaces/fragments/spread, bigint/separators, hashbang, top-level await, `import x = require`, instantiation expressions): oxc 30/30, tree-sitter 27/30. tree-sitter fails `accessor` fields, `type A<in out T> = ... infer U extends string`, and `export type * as ns from`. All three need a fork of tree-sitter-typescript (it vendors tree-sitter-javascript's `grammar.js`; two large `parser.c` files regenerated with the pinned tree-sitter CLI, see grammar-forks.md) and every new TS release will need the same again. On real files both parse all of the above corpus without errors except React's Flow-typed `ReactFiberWorkLoop.js`, which both reject (Flow is out of scope, decision 9).

Fit with the pipeline:
- Tree-sitter fits `core-treesitter` as is (grammar + mapping + 130-line formatter pattern of lang-toml). oxc needs an adapter: oxc's AST has no punctuation and no trivia, but `Parser::with_config(TokensParserConfig)` returns every token with its span and `program.comments` returns the comments. The adapter walks the AST with `Visit::enter_node/leave_node` (generic over `AstKind`, so one function, not one per node type), opens a rowan node per AST node (span, kind from a table), emits each lexer token inside the innermost open node, and fills the gaps with `Whitespace`/`Newline`/comment trivia exactly as `core-treesitter` does. About 500 lines, plus `grammars/js.grammar` (about 180 node kinds, 40 token kinds, tags `#block`, `#delimited-list`, `#separator`) and a test that every `AstKind` has a row.
- The tree keeps oxc's `ParenthesizedExpression` (default `preserve_parens`), which the formatter needs for `needs_parens`.
- An adapter has no incremental reparse; neither has `core-treesitter` today.
- Errors: oxc recovers and reports spans and messages (better messages than tree-sitter's "syntax error"); a `fatal_error` yields an empty program, which becomes one error covering the file. The formatter refuses files with errors either way.
- Maintenance: 0.x releases break the AST. Pin the exact version in the root `Cargo.toml`; a bump touches the kind table and the adapter only, because the formatter reads our kinds. The kind names are oxc's (ESTree) names, which is what ESLint rule authors and pattern rules expect.
- Binary: with packs, a tree-sitter grammar can live outside the executable and oxc cannot (it is Rust, not a C `cdylib`). It costs 0.8 MB (+2.5%) in `sudsy.exe`.

Recommendation: oxc. The cost is one 500-line adapter against a two-grammar fork we would own for every TypeScript release, and a 30-40x faster parse for the large files a linter sees.

## 2. Layout engine

`core-format` already has `text`, `line`, `softline`, `hardline`, `group` (+ ids), `indent`, `align`, `if_break`, `fill`, `conditional_group` (candidates with forced breaks are printed in break mode, as Prettier's `expandedStates`), `line_suffix`, `break_parent`, `indent_if_break`, `Shared` (O(1) clone), width by display width, and shapes for lists, blocks, key-values, binary chains and member chains; the C# formatter already hugs a last lambda argument with a `conditional_group`. Gaps, in order of size:

| Gap | Needed for | Where | Size |
|---|---|---|---|
| `if_break` that tests another group (`ifBreak(.., {groupId})`) | arrow chains, assignment layouts, type parameters, JSX parens | core-format (`Doc::IfBreak` gets `Option<GroupId>`; the printer already records group modes for `IndentIfBreak`) | 50 |
| `label` doc (a tag the parent's layout code can read from a child doc) | assignment/arrow layout choices that depend on whether the right side is a member chain | core-format | 40 |
| `line_suffix_boundary` | template literal `${}` and comments before it | core-format | 25 |
| A layout cache keyed `(node, flags)` | printing the last argument twice (normal and expanded) without 2^depth blow-up for nested callbacks; Prettier caches by node when no args are passed | core-format (`FormatCtx`), guarded by `tests/scaling.rs` | 80 |
| Several marker words per suppression role (`prettier-ignore` next to `sudsy-ignore`) | `// prettier-ignore`, `{/* prettier-ignore */}`; also closes GraphQL's roadmap item | core-format `ignore.rs` + grammar `suppress` syntax | 40 |
| Item-level ignore inside lists with a separator (object property, union member, JSX attribute, class member) | prettier-ignore parity | already "outermost node starting at that token"; check lists | 30 |
| Comment relocation: print a token's leading comments at another place (before `else` into the block, into empty `()`, after `return`) | the long tail of comment placement | core-format (`detach_trailing` exists) | 80 |
| Safety check for output that changes tokens (quotes, parens, `;`, numbers) | every JS file | lang-js via `NodeFormatter::extra_check`: reparse and compare the trees with `ParenthesizedExpression`, `;` and quote style normalized (what Prettier's `massageAstNode` does); `#restyle` already covers token text | 150 |

Belongs in lang-js (JS-specific, nothing for CSS/HTML to reuse): member-chain grouping (computed access, factory/short-name merge, call counts, `isMemberish`, retry one-line vs expanded; the shared `member_chain` shape takes the finished groups, about 250 lines), call-argument hugging (first/last argument, `shouldGroupLast`, conditional group `[flat, hug last, all broken]`; 500), assignment layouts (break-after-operator, chain, chain-tail, break-lhs, fluid, only-left; 300), binary/logical chains with per-parent indentation (200), ternaries and conditional-type chains (250), arrow chains and bodies (250), `needs_parens` (350), JSX with fill and jsx-whitespace (700; `fill` exists, the `{" "}` and word-splitting rules do not), objects/arrays/destructuring with preserved first-line break (exists as list layout `preserve`), strings, numbers and directive quotes (150), decorators, classes, TypeScript types (union hugging, intersections, mapped, conditional, function types, generics; 1500), `semi: false` ASI protection (150). Comments are token-owned (see decision 3).

Total lang-js formatter: about 7k lines of Rust (C#'s formatter is 3.0 K, its rules 4.3 K), risk of 1.5x.

## 3. Parity plan

Format (pinned Prettier, the version in `docs/parity/prettier.yaml`, installed by `scripts/parity.sh format prettier` as for JSON/YAML):
- Prettier's own corpus: tarball of the pinned tag, `tests/format/{js,jsx,typescript}/**` inputs minus the Flow-only folders, minus files Prettier's own babel/typescript parsers reject; default options; compare bytes of Prettier's output with `sudsy format`. Expect several thousand files (count at fetch time), too many for a reason per file: this corpus and the real-world one are ratchet groups of `tests/parity/prettier-js/differences.yaml` (minimum identical-file counts the run fails below; each step raises them). Staged targets: 60% byte-identical after the expression step, 85% after TypeScript and JSX, 92%+ at the end.
- Real-world corpus, pinned tarballs, gitignored, as the other `fetch-*-corpus.sh`: Prettier-formatted projects (vuejs/core, docusaurus, excalidraw, MUI, TypeScript's own `src` is not Prettier-formatted) check `sudsy(x) == x`; unformatted ones (lodash, three.js, express, TypeScript compiler, a vscode slice) check `sudsy(x) == prettier(x)`. Target 98% identical files; every file round-trips, formats idempotently, keeps meaning (the safety check). Option matrix on a subset: `semi: false`, `singleQuote`, `trailingComma none/es5`, `arrowParens avoid`, `bracketSpacing false`, `useTabs`.
- Biome's reported 97% compatibility on this corpus (with a rowan CST and token trivia, the same model) is the calibration for the end state; it is not re-measured.
- Speed: `bench/inputs/js/` (150 KB), `perf-bench` modes, recorded in `performance/numbers.md`.

Lint:
- Catalogs per `docs/parity/README.md`, one line per upstream rule: `eslint.yaml` (all core rules; the formatting rules deprecated since 8.53 are `formatter`/`wontfix`), `typescript-eslint.yaml` (syntax rules first; type-aware rules `deferred | needs the TypeScript worker`, matching the roadmap), `eslint-plugin-react.yaml` (recommended + hooks, later). `prettier.yaml` gains `js.format.*` options (`semi`, `singleQuote`, `arrowParens`, `jsxSingleQuote`, `quoteProps`, `bracketSameLine`, `singleAttributePerLine`, `trailingComma`) in place of the "not applicable" wontfix lines.
- First rules, syntax-only (patterns or small Rust; about 40 of eslint:recommended): `no-debugger`, `no-dupe-keys`, `no-dupe-class-members`, `no-dupe-else-if`, `no-duplicate-case`, `no-empty`, `no-empty-pattern`, `no-extra-boolean-cast`, `no-sparse-arrays`, `no-unsafe-negation`, `no-unsafe-finally`, `no-self-assign`, `no-with`, `no-useless-catch`, `no-useless-escape`, `no-prototype-builtins`, `use-isnan`, `valid-typeof`, `no-cond-assign`, `no-case-declarations`, `no-irregular-whitespace`, `no-loss-of-precision`, `require-yield`, `no-async-promise-executor`, `no-compare-neg-zero`, `no-regex-spaces`, `no-empty-character-class`, `no-octal`, `no-delete-var`, `no-shadow-restricted-names`, `getter-return` and `no-unreachable` after a flow graph (core-flow exists); typescript-eslint: `ban-ts-comment`, `no-explicit-any`, `no-non-null-assertion`, `no-empty-object-type`, `no-namespace`, `no-require-imports`, `prefer-as-const`, `no-inferrable-types`, `array-type`, `consistent-type-definitions`, `no-extra-non-null-assertion`, `no-misused-new`, `no-this-alias`, `triple-slash-reference`, `adjacent-overload-signatures`; `preset:js/eslint-recommended` turns on the same set ESLint's recommended does.
- Second wave, scope-aware: `no-undef`, `no-unused-vars`, `no-redeclare`, `no-const-assign`, `no-class-assign`, `no-func-assign`, `no-import-assign`, `no-shadow`, `no-use-before-define`, `consistent-type-imports`, via a semantic layer (decision 5).
- Differential runner: `scripts/parity.sh eslint` (hadolint mode): ESLint 9 flat config enabling the catalog's `rule:` entries, run on `tests/rules/js/*/*.js|ts|tsx` fixtures (annotations removed) and on the real-world corpus, compared as `(rule, line, column)`, differences in `differences.yaml` as for the other tools; needs Node, already needed for eslint-jsonc. typescript-eslint runs with its parser and only non-type-aware rules.

## 4. Staged plan

Each step is one pushable commit with `finish.sh` (gate + bench); new language code is lazy-loaded, so existing metrics stay flat. Lines = Rust src + tests + fixtures, estimated.

| # | Step | Lines | Files touched | Parallel with |
|---|---|---|---|---|
| 1 | core-format gaps: group-id `if_break`, `label`, `line_suffix_boundary`, layout cache, multi-word ignore markers (+ GraphQL `prettier-ignore`) | 400 | `core-format`, `lang-graphql` grammar | 2, 3 |
| 2 | Parser: `grammars/js.grammar`, `lang-js` crate (lang.yaml, adapter, kind table, provider), file matchers on, formatter prints everything verbatim, syntax-error diagnostics, `bench/inputs/js`, numbers | 1100 | `lang-js`, `grammars`, generated regions | 1, 3 |
| 3 | Parity harness: `fetch-js-corpus.sh`, `tests/parity/prettier-js`, `docs/parity/prettier.yaml` JS entries, `eslint.yaml` skeleton, `scripts/parity.sh` modes (kept under `scripts/`) | 500 | `scripts/`, `tests/conformance`, `docs/parity` | 1, 2 |
| 4 | Formatter core: file/statement sequences, blank lines, declarations, functions, classes, blocks, control flow, imports/exports, strings, numbers, directives, `semi` and quote options, token-owned comments; the safety `extra_check` (reparse and compare the AST, decision 11) lands here, before the first step that rewrites tokens | 1500 | `lang-js/src/format/{stmt,decl,class,literal}.rs`, `safety.rs` | none (sets the dispatch) |
| 5 | Expressions: calls and argument hugging, member chains, binary/logical, assignment layouts, ternaries, arrows, `needs_parens`, templates | 1800 | `format/{call,chain,binary,assign,ternary,arrow,parens}.rs` | 6, 7 |
| 6 | TypeScript: annotations, interfaces, type aliases, unions/intersections, generics, enums, namespaces, `declare`, decorators, abstract/overload, mapped/conditional types | 1500 | `format/ts/*.rs` | 5, 7 |
| 7 | JSX/TSX: elements, attributes, fill with jsx-whitespace, conditional chains in JSX, fragments, `jsx*` options | 700 | `format/jsx.rs` | 5, 6 |
| 8 | Objects/arrays/destructuring, preserved first-line break, `quote_props`, `trailing_commas`, `bracket_spacing`, `arrow_parens`, `bracket_same_line`, options schema and docs `formatting/js.md` | 600 | `format/object.rs`, `lang.yaml` | 5-7 (own file) |
| 9 | Comment long tail and `prettier-ignore`, ASI protection for `semi: false` | 900 | `format/comments.rs` | after 4-8 |
| 10 | Parity ratchet: fix by category until the targets; named differences in `differences.yaml`; option matrix | 1000 | `lang-js/src/format` | serial, may split by area |
| 11 | Lint, syntax-only: ESLint core pattern rules and Rust rules, hooks for the shared rules, `preset:js/eslint-recommended`, catalog, parity runner | 1800 | `lang-js/rules`, `rules.rs`, `docs/parity` | 4-10 (own dirs) |
| 12 | Lint, typescript-eslint syntax rules + preset | 900 | `lang-js/rules`, catalog | 11 |
| 13 | Embedding: JS/TS fences in Markdown formatted and linted; lint of `.vue`/HTML is later | 150 | `lang-markdown` embedded table | after 9 |
| 14 | Migration: extend the existing `sudsy init --from prettier` and `--from eslint` importers (`crates/cli/src/init/`; eslint already maps `jsonc/` rules through the catalog) to the `js.format.*` options and the ESLint core and typescript-eslint rules through their catalogs; `.prettierignore` and `.eslintignore`/`ignores` become `exclude` patterns | 300 | `crates/cli/src/init/{prettier,eslint}.rs` | after 8 and 12 |
| 15 | Scope-aware rules: semantic layer from `oxc_semantic` (re-parse at semantic time, results by span) + the second-wave rules | 1400 | `lang-js/src/semantic.rs`, `core-semantics` hook | after 11; runs beside the CSS/HTML work, not before it |

Total about 14 K lines including fixtures (formatter about 8 K, lint about 4 K, harness, parser and migration about 1.9 K). Steps 1-14 make JS/TS usable; CSS, SCSS and HTML (the rest of wave 3b) share no files with lang-js and run in other slots alongside it, not after step 15. The 4-to-5 handoff needs step 4's dispatch merged first; steps 5, 6, 7 and 8 then work in disjoint files (one registration line each in `format/mod.rs`). Steps 1, 2 and 3 share nothing but generated regions.

## 5. Decisions

1. **Parser: oxc** (user, 2026-11-10). Costs the adapter and a 0.x dependency; tree-sitter would have cost two grammar forks with 3 known gaps, a 20x slower parse and 3.3 MB.
2. **One language `js`.** One shares the options, grammar and formatter; TS-only nodes cannot occur in `.js`. Different options per extension go through `overrides`.
3. **Comments: token-owned plus relocations**, not Prettier's enclosing/preceding/following attach (about 600 lines and a second comment model in core-format). Revisit only if step 10 stalls under 90%.
4. **Register early** (user, 2026-11-10): the extensions are on from step 2 with a verbatim formatter (lossless, safe), so each step improves files in place.
5. **Scope analysis: `oxc_semantic`** (2-4 ms per 500 KB, already linked), not our own binder (about 1500 lines plus TypeScript scoping rules to own). Needed from step 15.
6. **Type-aware rules** (typescript-eslint `recommended-type-checked`): out of this wave (TypeScript compiler through a worker, roadmap); `deferred` in the catalog.
7. **Defaults and presets:** defaults equal Prettier's (width 80, 2 spaces, double quotes, `semi`, `trailing_commas: all`); presets are rule sets (`js/eslint-recommended`, `js/typescript-eslint-recommended`); a formatter preset such as `js/standard` is added when asked, as pure option values.
8. **Embedded code in templates** (`css`, `graphql`, `html` tagged templates) and `<script>` in HTML/Vue/Svelte: with those languages. Markdown fences ship in step 13.
9. **Flow:** not supported, documented; such files report a syntax error and are left unchanged.
10. **oxc updates:** pinned exact and ignored by Dependabot; bumped by a dedicated step (kind table, adapter, parity rerun), never automatically.
11. **Output verification:** the safety check as for every language (engine.md: debug builds, tests and the `corpus` profile; release builds trust the formatter). JS rewrites tokens (quotes, parens, semicolons), so its `extra_check` compares the reparsed AST with those normalized; it lands in step 4, before the first step that rewrites tokens, so every later step is checked on the corpus.

## Not checked
Prettier's corpus size and the pass rates are projections; Biome's 97% is from its documentation, not re-run; swc and Biome parsers were not built; oxc build time and its transitive crate count were not measured; Prettier's printer size in lines is not stated because it was not counted.
