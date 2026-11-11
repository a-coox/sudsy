# CSS / SCSS / Less (`lang-css`)

Three languages, `css`, `scss` and `less`, in one crate: one grammar (`grammars/css.grammar`), one lexer and parser, three providers. `scss` and `less` use `css` as their config base (`config_base()`): a `css:` section applies to all three and `scss:`/`less:` override it. Extensions `.css`, `.scss`, `.less`. The Sass indented syntax (`.sass`) is not supported. The plan for the rest is [../design/wave-3b-css.md](../design/wave-3b-css.md).

## Parser
Hand-written lexer and recursive-descent parser, lossless: every byte is in a token and trivia are placed by the shared attachment rule.
- **Lexer:** CSS Syntax Level 3 tokens (identifiers with escapes, at-keywords, hashes, strings, numbers, percentages, dimensions, `url(` with an unquoted body, unicode ranges). Per dialect: `//` comments (SCSS, Less), `$name` and `#{` (SCSS), `@{` (Less). An unterminated string, comment or `url(` is an `Unknown` token and an error. A `url(` whose body is quoted, a call or an interpolation is an ordinary function.
- **Parser:** a statement is told apart by what ends it, found by a scan that balances `()`, `[]` and `#{}` (braces too inside a custom property's value): `{` makes a `Rule`; `;` or `}` a `Declaration` if a `:` came first outside any group, else a `Statement`; a statement that starts with an at-keyword is an `AtRule` (so `@if`, `@mixin`, Less `@var: value`, detached rulesets and mixin definitions with guards need no rules of their own). Preludes, properties and values are `Prelude`, `Property` and `Value` nodes of tokens, `Function`s (a name directly followed by `(`) and `Group`s (`()`, `[]`, `#{}`, and `{}` in a group). Dialect constructs that formatting or rules need get their own nodes in later steps; until then a Less mixin call `.m(1);` is a `Statement` and `.a:extend(.b);` a `Declaration`.
- **Errors:** per statement. An unmatched `)`, `]` or a `}` inside a group ends the statement as an `ERROR` node, an unclosed group makes the rest of the file one, a stray top-level `}` is one token in an `ERROR` node, a missing `}` is one diagnostic. Later statements still parse. Nesting of blocks or groups beyond 256 is an error (the rest of the file is one `ERROR` node; bounds recursion in parser, formatter and drop).

## Formatting
The formatter writes the text as written (final newline, line ending and trailing blanks aside). The printer is a later step.

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

## Conformance
The real projects of `tests/conformance/fetch-css-corpus.sh` (Bootstrap, Bulma, ant-design, Semantic UI, normalize.css, docusaurus; its Prettier fixtures belong to the parity harness and hold deliberate errors): every file parses without errors, round-trips and formats as itself. `cargo test --profile corpus -p lang-css --test css -- --ignored --nocapture conformance` after `bash tests/conformance/fetch-css-corpus.sh`.
