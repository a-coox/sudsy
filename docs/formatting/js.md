# JavaScript / TypeScript / JSX / TSX (`lang-js`)

## Parser
[oxc_parser](https://github.com/oxc-project/oxc) 0.158.0 (MIT), pinned to the exact version in the root `Cargo.toml` (`=0.158.0` for the five `oxc_*` crates in use; dependabot ignores them). oxc is pre-1.0 and changes its AST about weekly, so a bump is a deliberate change of `grammars/js.grammar` and `crates/lang-js/src/adapter.rs` only (the formatter and rules read our kinds), followed by the conformance run. One language `js` and one grammar cover `.js .mjs .cjs .jsx .ts .mts .cts .tsx`; the extension picks oxc's source type (`.js` is parsed as JSX, a module unless it uses script-only syntax; `.cjs` is a script; `.d.ts` is a declaration file; text of no extension, such as a Markdown fence, as JSX). Flow (`@flow` files) is not supported: oxc rejects the syntax, so the file reports syntax errors and is not formatted.

oxc's AST has no punctuation nodes and no trivia, so `adapter.rs` builds our tree (the same shape as `core-treesitter`'s, which does this for tree-sitter) from three inputs:
- the lexer's tokens (`TokensParserConfig`: every token with its span), grouped into the token kinds of the grammar (punctuation with fixed text, `Operator`, `Keyword` for reserved words and `true false null`, `Identifier` for everything else word-like including contextual keywords, `String`, `Number`, `BigInt`, `RegExp`, `TemplateChunk`, `PrivateName`, `JsxTextRun`);
- the AST's node spans, from one `oxc_ast_visit::Visit` walk (`enter_node`/`leave_node` over every `AstKind`): each AST node is a node of the tree, named as in oxc (`CallExpression`, `TSTypeAliasDeclaration`, `JSXElement`, ...); `Program` is the root; `ParenthesizedExpression` is kept; a node with no token (`JSXEmptyExpression`) is left out;
- the text between two tokens, which holds only whitespace and comments, split into `Whitespace`, `Newline`, `LineComment` and `BlockComment` tokens (a hashbang and `<!--` / `-->` lines are line comments) and attached by the shared trivia rule.

A node opens before the first token that starts in it and closes before the first token that starts after it, so a token belongs to the innermost node containing it, and every file prints back byte for byte. A zero-width `Eof` token ends the root and owns the comments at the end of the file. oxc recovers from syntax errors and reports a message and a span for each (code `syntax-error`); after a fatal error (`fatal_error`: the program is empty) the file is one `Unknown` token and the fatal error. The formatter refuses files with syntax errors. Not covered: incremental reparse, `oxc_semantic` (scopes; later), a nesting limit (oxc and the walk recurse).

## Formatting
Not implemented yet: the formatter prints the file as written (comments and blank lines at the file's edges are normalized like in every language; `final_newline` and `end_of_line` apply). The options below are the shared ones; the language's own options arrive with the layout code.

## Options
<!-- generated:options -->
| Option | Values | Default | Description |
|---|---|---|---|
| `line_width` | int 20-1000 | `80` | Preferred maximum line width in columns. |
| `indent_size` | int 1-16 | `2` | Columns per indentation level (also the width of a tab). |
| `indent_style` | `space`, `tab` | `space` | Indent with spaces or tabs. |
| `final_newline` | bool | `true` | End the file with a newline. |
| `end_of_line` | `auto`, `lf`, `crlf` | `auto` | Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break. |
<!-- /generated -->

## Rules
Only the shared ones, which work through the grammar's tags (comments are `LineComment` and `BlockComment`). Suppression comments are `// sudsy-disable-next-line <rule>`; `// prettier-ignore` is an ignore marker for the formatter.

## Performance and size
See [../performance/numbers.md](../performance/numbers.md) (parse and format throughput on `bench/inputs/js`, binary size, build time).

## Conformance
`bench/inputs/js` (real files from MUI, Vue and Excalidraw, one file of each of `.js .jsx .ts .tsx`) and a probe file per extension with the syntax of 2022-2025 (`using`, `accessor`, `satisfies`, import attributes, decorators, class static blocks, `#x in o`, variance annotations, template literal types, enums, `declare module`, JSX namespaces and fragments, hashbang, ...) are checked on every test run (`crates/lang-js/tests/js.rs`): they parse without errors, print back byte for byte, satisfy the trivia attachment rule, contain no `Unknown` token, and format with all the shared checks.
