# Design: one safety check, languages supply values

## Summary
The generic check (`core-format/src/safety.rs`) compares nodes and tokens. Languages loosen it with grammar tags (`#separator`, `#transparent`, `#restyle`) and then re-verify through `NodeFormatter::extra_check`. Thirteen languages have that hook. Ten of them collect the `#restyle` tokens of both trees, normalize each one, and compare the lists, so the same collect-and-compare loop is written ten times. CSS and JS each compare comments in their own hook. CSS also re-does the whole token comparison to skip `;` and `,`, because tagging them `#separator` would change shapes and rules.

This design moves that work into the generic check:
1. **Comments are compared by the core for every language:** the comment texts, each trimmed at the end, as a multiset. A formatter may move a comment, but may not drop, duplicate or edit one. Today only CSS and JS check this. In other languages a lost comment passes the check, and release builds never run the check.
2. **`#restyle` items are compared by value in the core.** `NodeFormatter` gains `fn value(&self, item: SyntaxElementRef) -> Cow<str>`, called for each `#restyle` item of both trees. The default is the text. The core compares the count, the kinds and the values in order. Each language's normalization stays where it is (`text::number`, `block_lines`, `same_value`, ...); only the collecting and comparing moves into the core.
3. **`#restyle` may tag a node.** The node's subtree is then not compared token by token: its `value` is compared instead, and it must stay where it is. This is how CSS joins and splits signs (`+ 20px` / `+20px`, `$a +2` / `$a + 2`): `lang-css` tags its math-expression node and gives a value in which a unary sign is attached to its operand and a binary operator is spaced. Whether a sign is unary is decided by the CSS code from the operator's left neighbour, not by whitespace, so `1px -2px` (a list) and `1px - 2px` (a subtraction) keep different values.
4. **`#optional` tags tokens the formatter may add or remove, for the check only:** CSS `;` and `,`, JS `;`. `#separator` keeps its meaning for shapes and rules.
5. **`extra_check` stays for checks that are not per item:** the YAML data model, Markdown block structure, JSX text. Every other hook is deleted.

## Cost
- Code: the core gains about 60 lines (comment sequence, `value` calls, the node case, the new tag). About 200 lines leave the language crates: ten hooks and their collectors, the CSS token re-comparison, and the CSS and JS comment comparisons.
- Speed: the check runs in debug builds, tests and the `corpus` profile only. `value` returns `Cow`, so it allocates only for text it actually normalizes. Measure `corpus` test time for css, js and yaml before and after, and record it in `docs/performance/numbers.md`.

## Steps
| # | Step | Crates | After |
|---|---|---|---|
| 1 | Core: comments, `value`, node `#restyle`, `#optional`. Migrate every language with a hook in the same change; the default `value` (the text) keeps the others as strict as today. Delete the hooks that become empty. Document the new tags in `docs/formatting/engine.md`. | `core-format`, `core-syntax` (tags), every `lang-*` with `#restyle`, `grammars/*.grammar` | JS step 9 is merged (it changes the JS comment check) |
| 2 | CSS sign math: tag the math node `#restyle` with its value, then let the formatter join and split signs as Prettier does. Expected to fix the 4 Prettier test files `scss/math/3945`, `scss/parens/parens`, `less/parens/parens` and `scss/scss/3757`. | `lang-css` | 1 |

## Decisions
1. **Comments are compared as a multiset (order ignored), as JS step 9 does.** What has to be caught is a lost, duplicated or edited comment. A comment moved past another comment, which JS relocations can do, is not a loss.
2. **No per-language comparison of whole trees through `extra_check`.** The hook stays only for checks that are not per item.

## Not checked
- Whether every language's comment kinds are known to `core-format` (`CommentKind`): step 1 starts by listing them.
