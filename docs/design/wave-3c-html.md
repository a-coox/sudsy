# Design: HTML, Vue, Svelte (wave 3c)

Plan for the markup family after JS and CSS. Each step moves what it builds into `docs/formatting/html.md`; this note is deleted when the last step lands. Line counts are estimates.

Goal: replace Prettier (with prettier-plugin-svelte) for `.html`, `.htm`, `.vue` and `.svelte`, and replace the most-used markup linters, with migration from their configs. Script and style code inside markup is formatted and linted by the existing `js` and `css` providers.

## Usage (npm weekly downloads, 2026-11-11)
| Tool | Downloads |
|---|---|
| vue-eslint-parser / eslint-plugin-vue | 11.3M / 9.3M |
| @angular-eslint/eslint-plugin-template | 4.7M |
| prettier-plugin-svelte / eslint-plugin-svelte | 2.4M / 2.3M |
| eslint-plugin-vuejs-accessibility | 0.83M |
| html-validate | 0.78M |
| @html-eslint/eslint-plugin | 0.37M |
| htmlhint | 0.33M |
| markuplint | 0.07M |

Vue is the largest audience, Angular templates the second, Svelte the third. Plain-HTML linters are small next to the framework ones.

## 1. Scope and order
1. **HTML first.** Every other dialect is HTML plus template syntax, so the HTML parser, layout and embedding come first.
2. **Vue second:** single-file components (`<template>`, `<script>`, `<script setup>`, `<style scoped>`), `{{ }}` interpolation, and directive expressions (`v-if`, `:prop`, `@event`, `v-for`, `v-slot`) formatted as JS expressions.
3. **Svelte third:** `{#if}`/`{#each}`/`{#await}`/`{#key}`/`{#snippet}` blocks, `{expr}`, `{@html}`/`{@render}`/`{@const}`, Svelte 5 runes in script (JS already parses them).
4. **Angular templates:** not in the roadmap's list. With 4.7M weekly downloads they rank above Svelte. Decision 6 asks to add them after Svelte. They are `.html` files, so they need either a file pattern (Prettier uses an `overrides` entry for `*.component.html`) or a config switch.
5. Out of scope: Astro, Handlebars/Glimmer, LWC, Razor (its own roadmap item, with the C# worker).

## 2. Parser: tree-sitter, three forked grammars (measured)

Measured by the step 0 spike. The corpus: Prettier 3.9.10's `tests/format/{html,vue}`, prettier-plugin-svelte 3.5.2's tests, Bootstrap 5.3.3 and mdn/learning-area HTML, vuejs/core and element-plus SFCs, and sveltejs/kit and svelte.dev components. Measured on Windows x64, release, one thread. Clean means no ERROR or MISSING node.

| Grammar on its files | Prettier fixtures clean | Real files clean | MB/s | Binary |
|---|---|---|---|---|
| tree-sitter-html 0.23.2 | 135/148 | 233/257: 153/154 of genuine HTML; the rest are Bootstrap's Hugo templates | 5-7 | +127 KB |
| tree-sitter-vue (tree-sitter-grammars, vendored at ce8011a; not on crates.io) | 76/83, all test-harness markers or deliberately unclosed tags | 201/201 | 8-20 | +137 KB |
| tree-sitter-svelte-ng 1.0.2 | 75/76: `{@attach}`, Svelte 5.29 | 200/200 | 4-5 | +155 KB |

- **html5ever's tokenizer gives line numbers only, no byte spans.** It decodes references and lowercases names, so it cannot serve a lossless tree.
- **A hand-written parser was estimated at 1,500-1,900 lines** from the constructs the corpus uses.

Decision: **tree-sitter, with the three grammars vendored and patched** as for bash, make and ini (`grammars/tree-sitter-*`, `FORK.md`). The fixes are about 5 rules plus one scanner edit:
- html: alias the doctype body so it is a node; CDATA; `textarea`/`title` as RCDATA, since today `<textarea><b>` silently nests a `<b>` element: wrong without an error;
- svelte: `{@attach}`, and the literal text of a quoted attribute that mixes text and `{expr}`, which is hidden today.

Each grammar maps onto one `grammars/html.grammar` through `core-treesitter` mappings, so the formatter and rules see one tree for the three languages. Whitespace between nodes is the usual gap trivia. Expression bodies (`{{ }}`, directive values, `{...}`) and `<script>`/`<style>` stay raw text and go to `lang-js`/`lang-css` through embedding (section 4).

Trade-off accepted: 4-20 MB/s against the 16-60 MB/s a hand-written parser reaches. Each grammar regeneration also adds a large `parser.c` diff to the history.

## 3. Languages and options
Three languages in one crate: `html`, plus `vue` and `svelte` with `html` as config base (`config_base()`). Options, with Prettier's defaults:
- the shared `line_width` and indentation;
- `whitespace_sensitivity: css | strict | ignore` (Prettier's `htmlWhitespaceSensitivity`);
- `bracket_same_line`;
- `single_attribute_per_line`;
- `vue.indent_script_and_style` (Prettier's `vueIndentScriptAndStyle`);
- `svelte.indent_script_and_style` and `svelte.sort_order` (prettier-plugin-svelte);
- `embedded_language_formatting`, already declared by hosts.

Presets are rule sets only.

## 4. Formatting
Prettier's printer-html is the reference.
- **Whitespace model:** whitespace between inline content is significant. Prettier keeps it, or moves it inside the tag syntax (`><`, the "ugly" closing bracket) so lines can break without adding or removing rendered spaces. An element's display (block, inline, inline-block, none, `pre`) comes from a table of HTML element defaults, as in Prettier, and is overridden by a `<!-- display: block -->` comment. `whitespace_sensitivity: ignore` treats everything as block; `strict` treats everything as inline. This model is about half of the work and most of the parity risk.
- **Elements:**
  - children break one per line when they don't fit, and block children always break;
  - attributes fill the line or go one per line (`single_attribute_per_line`);
  - the closing `>` goes on its own line or the last attribute's line (`bracket_same_line`);
  - void elements are kept as written (`<br>` or `<br />`), as Prettier does;
  - `pre`, `textarea` and raw text keep their content.
- **Attributes:** values are double-quoted unless the value holds `"`. `class` values have their whitespace collapsed. `style` values are formatted as CSS declarations, and `srcset`/`sizes` lists are broken.
- **Embedded code** goes through the engine's existing embedding (`docs/architecture/adding-a-language.md`, Embedded languages): `<script>` (JS or TS by `type`/`lang`, JSON for `application/json`/`importmap`, kept for unknown types), `<style>` (CSS, SCSS or Less by `lang`), `on*` attributes and `style` attributes. Two additions to `Embedded`:
  - **A mode.** A region can be a program, an expression (Vue `{{ }}` and directives, Svelte `{...}`), a declaration list (`style=""`), or a binding pattern (`v-for`, `v-slot`). `lang-js` and `lang-css` gain entry points for these.
  - **Start column and width.** The embedded formatter gets the column it starts at and the width it has, so an expression breaks inside its host line.

  Script and style content is indented one level in HTML, and in Vue and Svelte only with `indent_script_and_style`.
- **Comments, ignore and safety:** comments stay where they are. `<!-- prettier-ignore -->` and `<!-- sudsy-ignore -->` keep the next node as written. The safety check is the shared one ([safety-check.md](safety-check.md)): `#restyle` on text runs and attribute values, compared by a value with whitespace collapsed where the element's display allows it. `pre` and raw text are compared exactly.

## 5. Lint
- **Catalogs, one line per upstream rule:**
  - `html-validate.yaml`, the primary plain-HTML catalog: most used, about 90 rules;
  - `html-eslint.yaml`;
  - `htmlhint.yaml`;
  - `eslint-plugin-vue.yaml`, template rules first; rules that need script analysis come with the JS semantic layer;
  - `eslint-plugin-svelte.yaml`;
  - `eslint-plugin-vuejs-accessibility.yaml`.
- **First rules, tree-only:**
  - duplicate attributes and ids;
  - deprecated and unknown elements and attributes, from a generated data table like CSS's;
  - void element style;
  - missing `alt`, `lang`, `title`, `<label>` association, `button` `type`;
  - heading order;
  - inline styles;
  - Vue's `v-for` key, `v-if` with `v-for`, and directive validity;
  - Svelte's `{#each}` key.

  These are mostly pattern rules over the markup tree, and families as data where shapes repeat.
- **Parity runners:** html-validate, html-eslint, HTMLHint, eslint-plugin-vue and eslint-plugin-svelte, as for ESLint. Differences are named `gap`s (CLAUDE.md).
- **Embedded lint:** script code is linted by the `js` rules and style code by the `css` rules, with no new code (the embedding mechanism).

## 6. Parity plan
- **Format:** `prettier-html` mode with ratchet groups `prettier-html`, `prettier-vue`, `prettier-svelte` (prettier-plugin-svelte's tests) and `real-world`. Staged targets: 60% after step 3, 85% after step 6, 92% at the end.
- **Speed:** `bench/inputs/{html,vue,svelte}`, recorded in `docs/performance/numbers.md`.

## 7. Steps
| # | Step | Lines | Where | After |
|---|---|---|---|---|
| 0 | Parser spike (done: section 2) | - | - | - |
| 1 | Vendor and patch tree-sitter-html (doctype alias, CDATA, RCDATA), `grammars/html.grammar` and its mapping, `lang-html` with the `html` provider, verbatim formatter, registration, bench input, round-trip over the corpus | 400 + fork | `grammars/tree-sitter-html`, `lang-html` | 0 |
| 2 | Parity harness: `prettier-html` format mode, html-validate lint mode, catalogs | 400 + catalogs | `scripts/`, `tests/parity`, `docs/parity` | 1 |
| 3 | Formatter: elements, attributes, the whitespace model, comments, ignore, options | 2,000 | `lang-html/src/format` | 2 |
| 4 | Embedding: script and style regions, `Embedded` mode, start column and width; JS expression and CSS declaration-list entry points | 400 | `core-syntax`, `lang-js`, `lang-css`, `lang-html` | 3 |
| 5 | Vue: vendor tree-sitter-vue, its mapping; SFC blocks, interpolation, directives as embedded expressions, options | 600 | `grammars/tree-sitter-vue`, `lang-html` | 4 |
| 6 | Svelte: vendor and patch tree-sitter-svelte-ng (`{@attach}`, attribute text), its mapping; blocks, tags, options | 600 | `grammars/tree-sitter-svelte`, `lang-html` | 4 |
| 7 | Lint: html-validate core rules, data table, presets; Vue and Svelte template rules | 1,500 | `lang-html/rules` | 3 |
| 8 | Migration: `init --from` for html-validate, HTMLHint and html-eslint; Prettier HTML/Vue/Svelte options in the Prettier importer | 300 | `crates/cli/src/init` | 5, 6, 7 |
| 9 | Markdown: `html` fences (likely already free, as `css` was) and HTML blocks in Markdown kept as written | 20 | `lang-markdown` | 3 |

Steps 5, 6 and 7 can run in parallel in separate slots once step 4 lands.

## 8. Decisions
1. **One crate, three languages,** as for CSS: the dialects share the tree and the layout.
2. **No WHATWG tree construction.** The tree is the source as written, plus implied end tags. A document the browser would repair differently (misnested `<b><i></b></i>`) formats as written and gets a lint finding.
3. **The whitespace model follows Prettier's `css` mode by default,** so output is byte-compatible for Prettier users. `strict` and `ignore` are options, not presets.
4. **Expressions in templates are formatted by `lang-js`,** never re-parsed in `lang-html` (CLAUDE.md).
5. **Void elements and attribute quotes** follow Prettier. Choices that differ from Prettier are named `choice` entries.
6. **Angular templates (user decision needed):** add them after Svelte as a fourth language, `angular`, selected by pattern `*.component.html` and an `overrides` entry. The alternative is to leave them out of this wave.

## Not checked
- Whether `vue-eslint-parser`'s template AST positions can be mapped to ours for the parity runner without per-rule work.
- prettier-plugin-svelte's current test-corpus layout and size.
- Biome's experimental HTML support (Vue, Svelte, Astro partial, `experimentalFullSupportEnabled`) is a reference point only; it is not measured.
