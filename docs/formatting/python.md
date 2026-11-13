# Python (`lang-python`)

## Parser
[`ruff_python_parser`](https://github.com/astral-sh/ruff) 0.0.14 (MIT) with `ruff_python_ast` and `ruff_text_size`, pinned to the exact version in the root `Cargo.toml` (the version Pyrefly pins; dependabot ignores the `ruff_*` crates). The crates are 0.0.x and change weekly, so a bump is a deliberate change of `grammars/python.grammar` and `crates/lang-python/src/adapter.rs`.

`adapter.rs` builds our tree (the shape of `lang-js`'s) from three inputs:
- the lexer's tokens, one token kind per ruff `TokenKind` (names as ruff's: `Name`, `Lpar`, `If`, `String`, `FStringMiddle`, ...). Comments are `Comment` trivia; logical and non-logical newlines, indents and dedents are `Newline` and `Whitespace` trivia, and a backslash that ends a line is `Continuation`;
- the AST's node spans, from one `SourceOrderVisitor` walk: each AST node is a node of the tree, named as in ruff (`StmtFunctionDef`, `ExprCall`, `Arguments`, `Parameters`, ...), `ModModule` is the root. A node with no token (a name error recovery adds) is an empty node;
- the text between two tokens, split into trivia tokens and attached by the shared trivia rule. A byte-order mark is leading trivia; text that is none of these is an `Unknown` token and an error.

ruff's expression ranges exclude parentheses, so the adapter makes a `Parenthesized` node (tagged `#transparent`) around each parenthesized expression or pattern, one per pair, nested for `((x))`. The parentheses of a call or of a class's bases belong to `Arguments` and make none.

Errors: ruff recovers from syntax errors and its errors cascade, so a file reports the first error of each statement (code `syntax-error`), and the version errors of the option `target_version` (`lazy import` before 3.15) are syntax errors. A file with errors is reported, not formatted.

<!-- generated:options -->
| Option | Values | Default | Description |
|---|---|---|---|
| `line_width` | int 20-1000 | `88` | Preferred maximum line width in columns. |
| `indent_size` | int 1-16 | `4` | Columns per indentation level (also the width of a tab). |
| `indent_style` | `space`, `tab` | `space` | Indent with spaces or tabs. |
| `final_newline` | bool | `true` | End the file with a newline. |
| `end_of_line` | `auto`, `lf`, `crlf` | `auto` | Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break. |
| `quote_style` | `double`, `single`, `preserve` | `double` | Quotes of strings (and of f-string and t-string literals): `double` or `single` rewrite a string to that quote unless the other one needs fewer escapes, `preserve` keeps what the source has. |
| `magic_trailing_comma` | `respect`, `ignore` | `respect` | `respect` keeps a collection, call or parameter list that has a trailing comma expanded one item per line; `ignore` joins it when it fits and removes the comma. |
| `docstring_code` | bool | `false` | Format the code of doctest examples (`>>>`) in docstrings. |
| `target_version` | `3.9`, `3.10`, `3.11`, `3.12`, `3.13`, `3.14`, `3.15` | `3.14` | The Python version whose syntax the file must follow: newer syntax is a syntax error (`lazy import` needs 3.15). |
<!-- /generated -->
