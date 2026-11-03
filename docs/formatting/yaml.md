# YAML (`lang-yaml`)

## Parser (own, lossless, YAML 1.2)
Why own: existing crates are event/value based (not lossless) or bring their own tree shape and trivia placement (`yaml_parser`, `yaml-edit`, `tree-sitter-yaml`); we need our grammar's tree, our trivia rule and the yaml-test-suite as a gate.
- Recursive descent after the spec's productions; events build the tree in `parser/builder.rs`, which lexes trivia between tokens and splits it by the attachment rule.
- An error aborts the rest of its document: everything up to the next document marker becomes one `ERROR` node; the next document parses normally. Nesting beyond 100 is an error.
- Spec-strict choices: a flow collection's closing bracket must be indented like its content; `%TAG` handles checked per document; document markers inside flow collections/quoted scalars are errors; a tab may separate but never indent; the 1024-char implicit-key limit is not enforced; non-printable characters are errors.
- Tree: one node per collection, entry and scalar (`BlockMapping(Entry)`, `BlockSequence(Entry)`, `FlowMapping`, `FlowSequence`, `FlowPair`, `Scalar`, `BlockScalar`, `AliasNode`, `PropertyNode`); an empty node is absent. Indentation is trivia. Multi-line plain/quoted scalars and block scalar content are single tokens. `#restyle` tags scalar tokens and block headers; `#separator` tags `---`.

## Data model (`model.rs`)
Loaded from our tree. Core schema (`yes`, `on`, `0b1`, `1_000` are strings), tags, anchors/aliases expanded (an undefined alias loads as an opaque node, so it formats unchanged; the `no-undefined-alias` rule reports it), scalar folding with chomping and indentation indicators. Used for the safety check (before == after on value, type, tag, structure, document count). The config loader uses `serde-saphyr` instead (core-config cannot depend on a language crate; gives spans and strict booleans).

## Options
| Option | Values | Default |
|---|---|---|
| `line_width` | int | 80 |
| `indent_size` | int | 2 |
| `final_newline` | bool | true |
| `sequence_indent` | `preserve`, `indented`, `flush` | `preserve` |
| `quote_style` | `preserve`, `single`, `double`, `prefer_double` | `preserve` |
| `document_start` | `preserve`, `always`, `never` | `preserve` |
| `max_blank_lines` | int | 1 |
| `blank_lines_after_key` | `preserve`, `remove` | `preserve` |

`sequence_indent: preserve` is default because both styles are common (Kubernetes vs GitHub Actions) and the zero-config goal is the smallest diff. No `indent_style`: tabs cannot indent YAML.

## Formatter behaviour
- Kept: quotes, key order, flow vs block, comments, blank lines (max one). Normalized: spacing and indentation (`key: value`, `{ a: 1 }`, `[1, 2]`).
- `quote_style` changes a scalar only if the other style needs no more escapes than the source has; never with control characters or across lines; plain scalars are never quoted.
- `document_start`: `always` adds `---` to documents with content; `never` omits it from the first document unless it has directives or no content; comments on a removed marker stay.
- Block scalars: re-indented, never re-wrapped; trailing blanks kept; indentation indicator rewritten; blank lines after `keep` are exact.
- Multi-line plain/quoted scalars keep line structure; continuation lines are re-indented.
- Placement after `key:`, `-`, `?`, `---` (`block::node_after`): scalars stay on their line, a collection after `-` starts on the dash line, a comment after the indicator keeps the value on the next line. A flow collection that does not fit moves under its key; one used as a key prints as written.
- Hook: comments between the end of a nested block collection and the next token stay indented with the collection (printed after its last entry).
- `sudsy-ignore` before an entry and `-start`/`-end` among entries of a block collection keep them as written.
- Safety: generic tree comparison, then the data-model comparison.

## Known gaps
Long plain/quoted scalars and comments are not re-wrapped; properties split over lines with a comment between them print as written; trailing blanks at a block scalar's end and a `keep` scalar's blank lines at EOF survive only via the printer's protected output length; files whose flow closing bracket sits at its key's column are rejected as the spec requires.

## Conformance
yaml-test-suite `data` branch at `6ad3d2c` (`tests/conformance/fetch-yaml-corpus.sh`), `cargo test --release -p lang-yaml --test conformance -- --ignored`: 308 valid cases parse, round-trip and satisfy the attachment rule; 94 error cases error; 279/279 with `in.json` have an equal data model; all valid cases format with every check, also under each single option value and two combined sets.
