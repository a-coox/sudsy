# Design: CSS / SCSS / Less (wave 3b)

Plan for the second language family of wave 3b, built in the steps of section 5 alongside JS/TS ([wave-3b-js.md](wave-3b-js.md)), in other slots. Each step moves what it builds into `docs/formatting/css.md`; this note is deleted when step 8 lands. Numbers marked (measured) come from a parser spike on Windows x64, release, one thread; line counts are estimates.

Goal: replace Prettier (format) and stylelint with its scss/less plugins (lint) for `.css`, `.scss` and `.less`, with migration from their configs. Speed only needs to beat them, which any native parser does.

## 1. Parser: hand-written, lossless

Candidates (measured; corpus pinned: Bootstrap 5.3.3 scss, Bulma 1.0.2 sass, ant-design 4.24.16 less, Bootstrap 3.4.1 less, normalize.css, three dist CSS files of 281-763 KB, Prettier 3.9.10 `tests/format/{css,scss,less}`):

| | Real files parsed | Prettier fixtures parsed | Speed | Binary | Lossless |
|---|---|---|---|---|---|
| raffia 0.13 (CSS, SCSS, Sass, Less) | 503/504 | 70% css, 89% scss, 72% less | 16-62 MB/s | +1.0 MB | spans only: no whitespace, no punctuation nodes, private tokenizer, 271 typed AST structs with no generic walk |
| tree-sitter css 0.25 + scss 1.0 + less 1.0 | 15-34% of scss/less files; 0 of the 3 dist CSS files (one error such as `--x: ;` or `\9` fails the whole file) | 51% css, 18% scss, 51% less | 1.4-7 MB/s | +0.4 MB | yes |
| biome_css_parser 0.5.8 | CSS only (crate from 2024-12) | 62% css | 5.5-17 MB/s | +0.8 MB | yes |

Tree-sitter would need three deep grammar forks (the failures are core syntax: `!default`, `@use ... as`, mixin definitions, empty custom properties), and Biome's published parser has no SCSS or Less. raffia parses almost everything, but would need an adapter of 500-900 lines: a gap re-lexer, because its tokenizer is private; a walk over 271 types; and comment merging. That is on top of a one-maintainer 0.x crate with a breaking release every few months, and its open gaps are in Less interpolation.

Decision: **a hand-written lexer and parser in `lang-css`**, like JSON's (714 lines of lexer and parser). CSS's structure is generic, so one parser serves the three dialects:
- **Lexer:** CSS Syntax Level 3 tokens, plus per-dialect extras: `//` comments, `$var`, `#{...}` and `@{...}` interpolation, `%placeholder`, Less `~"escape"` and `@@var`. About 450 lines.
- **Parser:** a stylesheet is a list of rules, at-rules and declarations; blocks nest (CSS Nesting, SCSS, Less). A statement is told apart by what ends it: `{` makes a rule or nested rule, `;` or `}` a declaration, so `a:hover {` and `color: red;` need no grammar knowledge. Preludes and values stay **component-value trees**: tokens, plus `()`, `[]` and function blocks. That is enough for the formatter, as postcss plus postcss-value-parser is for Prettier. Dialect constructs get their own nodes only where formatting or rules need them:
  - SCSS: `@if/@else` chains, `@mixin`/`@include` arguments, `@each`/`@for`/`@while`, maps `(k: v)`, nested properties `font: { }`, `!default`/`!global` flags, `@use`/`@forward`.
  - Less: mixin definitions with guards (`.m(@a) when (...)`), mixin calls, `@var: value`, detached rulesets.
  - About 900 lines.
- **Error recovery** is per statement: an unparseable statement becomes an `ERROR` node up to its `;` or matching `}`. One bad declaration never fails the whole file, which is tree-sitter's failure mode here.
- **Sass indented syntax** (`.sass`): not supported. Prettier does not format it, and it is rare.

## 2. Languages and options

Three languages in one crate, `css`, `scss` and `less`, with `scss` and `less` using `css` as their config base (`LanguageProvider::config_base()`): a `css:` section applies to all three, and `scss:`/`less:` override it. Extensions `.css`, `.scss`, `.less`. Options are Prettier's for CSS: shared `line_width`, indentation and end of line, plus `single_quote`. Defaults equal Prettier's. Presets are rule sets only: `css/stylelint-recommended`, `css/stylelint-standard`, and `scss/...` and `less/...` from the plugins' shared configs.

## 3. Formatting

Prettier's printer-postcss is the reference. Most of it is per-node and local:
- Rules: one selector per line in a selector list. Combinators get single spaces. Attribute selectors, pseudo-classes and `:is()` arguments are normalized. A block always opens `{` on the selector line, with one declaration per line.
- Declarations: `prop: value;` with a lowercase property (custom properties kept). The value is printed from the component-value tree: commas and spaces normalized, long comma lists broken one per line (`grid-template-columns`, `transition`, `src` of `@font-face`). `url()` and strings are kept, with quotes per `single_quote`. Numbers drop trailing zeros and get a leading zero; hex colors and keywords keep their case, as Prettier does.
- At-rules: `@media`/`@supports`/`@container` preludes normalized (spaces around `and` and `:`), `@import`/`@use` lists, and SCSS control flow laid out like blocks.
- Comments stay where they are. Blank lines between statements are kept up to `max_blank_lines`, defaulting to 1 like Prettier. `/* prettier-ignore */` and `sudsy-ignore` keep the next statement as written: the several markers per ignore role from JS step 1.
- The safety check is the usual one; it compares tokens with whitespace and quote style normalized.

Estimate: about 1,500 lines for the common printer and 600 for SCSS/Less constructs. core-format needs nothing new.

## 4. Parity and lint

- **Format:** Prettier's `tests/format/{css,scss,less}` inputs, and a real-world corpus (the spike's projects plus Prettier-formatted ones) run with each project's own Prettier config. Both are ratchet groups in `tests/parity/prettier-css/differences.yaml`. The `.css` fixtures that hold SCSS or Less syntax on purpose are expected parse failures, and are listed under one `gap` or `choice` name.
- **Lint:** catalogs for `stylelint.yaml` (153 core rules), `stylelint-scss.yaml` and `stylelint-less.yaml`, and a stylelint differential runner using the hadolint-style mode.
  - About 120 core rules need only the tree and their options. Several are families written once as data:
    - `*-allowed-list` / `*-disallowed-list` / `*-required-list` over properties, units, functions, at-rules and selectors
    - `*-no-empty`
    - `*-case`
    - `max-*` nesting and compound counts
    - `*-no-duplicate-*`
  - About 33 rules need data tables: the `*-no-unknown` rules (known properties, at-rules, units, pseudo-classes) and the `*-no-deprecated` rules. They take their data from `known-css-properties` and `mdn-data` (MIT/CC0), generated into a compact table and loaded lazily.

## 5. Steps

| # | Step | Lines | Files | Parallel with |
|---|---|---|---|---|
| 1 | Lexer, parser, `grammars/css.grammar`, `lang-css` crate with the three providers, verbatim formatter, syntax errors, registration, `bench/inputs/css`, round-trip conformance over a pinned real corpus (`tests/conformance/fetch-css-corpus.sh`, every file parses) | 1,500 | `lang-css`, `grammars/css.grammar` | JS steps; step 2 |
| 2 | Parity harness: `prettier-css` format mode (ratchet), stylelint lint mode, the three catalogs | 400 + catalogs | `scripts/parity_tools.json`, `tests/parity`, `docs/parity` | 1 |
| 3 | Formatter: rules, selectors, declarations, values, at-rules, comments, blank lines, ignore markers, options | 1,500 | `lang-css/src/format/*` | 5 |
| 4 | SCSS/Less formatting and parity ratchet up to about 85% on Prettier's fixtures | 900 | `lang-css/src/format/{scss,less}.rs` | 5 |
| 5 | Lint, tree-only stylelint rules (families as data first), presets | 1,500 | `lang-css/rules`, `core-rules` families | 3, 4 |
| 6 | Data-table rules (`*-no-unknown`, `*-no-deprecated`) | 600 | `lang-css/data`, rules | 4 |
| 7 | Migration: `sudsy init --from stylelint` (new importer: rules through the catalogs, `extends` to presets, `.stylelintignore` to `exclude`), and CSS options from the Prettier importer | 300 | `crates/cli/src/init/` | after 5 |
| 8 | Embedding: `css`/`scss`/`less` fences in Markdown. `<style>` in HTML/Vue/Svelte and `css` template tags in JS come with those languages | 50 | `lang-markdown` | after 3 |

Total about 6.5 K lines with fixtures. HTML (and with it Vue/Svelte) gets its own design after this one is under way. It will reuse this parser for `<style>` and JS's for `<script>`.

## Decisions
1. **Parser:** hand-written (section 1), not raffia, tree-sitter or Biome.
2. **Three languages, one crate, `css` as config base:** dialect-specific rules and options stay separate, and shared settings are written once.
3. **Sass indented syntax:** not supported.
4. **Defaults equal Prettier's; presets are rule sets** (as for JS).
5. **Data-table rules use generated tables** from `known-css-properties`/`mdn-data`, pinned and refreshed deliberately, like oxc.
