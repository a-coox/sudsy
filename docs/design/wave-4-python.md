# Design: Python

Decided plan, built in the steps of section 5; each step moves what it builds into `docs/formatting/python.md`, and this note is deleted when the last step lands.

Goal: the formatter and linter Python users switch to: `ruff format` output (Black's style), ruff's default lint findings, migration from `pyproject.toml` / `ruff.toml` / Black config. Type-aware rules come later through Pyrefly in-process ([roadmap](../roadmap.md)), which parses with the same crates.

## Summary
1. Parser: **`ruff_python_parser`** from crates.io (MIT), pinned exactly to the version Pyrefly pins (`=0.0.14` today), behind an AST-to-lossless-tree adapter in the shape of `lang-js/src/adapter.rs`.
2. One language `python` for `.py .pyi .pyw`; ruff's node and token names.
3. Formatter: `ruff format` parity with its defaults (width 88, double quotes, magic trailing comma, spaces); options for what ruff and Black let users set.
4. Lint: ruff's default rule set first (pyflakes `F`, pycodestyle `E4 E7 E9`), the bug-catching ones on by default (zero-config decision), then more ruff families by usage.
5. Land in 4 large steps.

## 1. Parser (measured, spike in `C:/Coding/.sudsy-targets/slot4/spike`, not committed)

Ryzen 5 3600, Windows 11, release, one thread, machine under load (ratios hold, absolutes noisy). Corpus: CPython 3.14.10 `Lib/`, black 26.10.1 `tests/data`, ruff 0.17.4 formatter and parser fixtures, requests 2.34.2, django 5.2.8: 5,968 files, 86 MB.

| | ruff_python_parser 0.0.14 | tree-sitter-python 0.25 |
|---|---|---|
| Parse | 30-70 MB/s | 2.4-7.1 MB/s |
| Parse + lossless rowan tree (untuned walk) | 13-27 MB/s | 1.7-3.1 MB/s |
| Peak memory, 1.86 MB input, parse + tree | 23 MB | 46 MB |
| Binary added to `sudsy.exe` | +1.75 MB (+3.5%); not packable | +0.46 MB (+0.9%); packable |
| Crates added | 5 ruff + about 30 new third-party (4 proc-macro), all MIT/Apache/BSD-like | 1 |
| Round-trip failures | 0 | 0 |
| Bytes between tokens that are not whitespace or `\` continuation | 3 (one `Unknown` `€`) | 560 KB+ (string content between escapes is not tokens) |
| Valid modern syntax (28 probes: PEP 695/696/701/758, t-strings, match, `except*`) | 26/28, the 2 others are 3.15 syntax reported as version errors | fails PEP 695 tuple bounds, PEP 696 defaults, some 3.13-3.14 |
| Error recovery (300 damaged files: first error within 2 lines / statements after the damage kept) | 88-99% / 96-100% | 48-76% / 82-95% |

Decision: ruff's parser. It is the only candidate that parses current Python, recovers well enough for the editor, is fast, and is the parser Pyrefly uses, so the type worker later shares the parse. Cost: 0.0.x API churn (a bump touches the kind table and adapter only, as with oxc), +1.75 MB, no language pack.

Adapter decisions (the spike's open questions):
- **Pin:** `=0.0.N` in the root `Cargo.toml`, the version Pyrefly's latest release pins; bumped together with Pyrefly, Dependabot-ignored like oxc. Vendoring is not needed (crates.io).
- **Target version:** option `target_version` (default the newest stable, `3.14`); ruff's version errors (`lazy import` before 3.15) are syntax errors at that version. The importer reads ruff's `target-version` and `requires-python`.
- **Parentheses:** the adapter synthesizes a `Parenthesized` node around a parenthesized expression (one per pair, nested for `((x))`), as oxc's `ParenthesizedExpression`; the formatter's `needs_parens` works on it.
- **Zero-width nodes:** emitted as empty nodes (`lambda:` has an empty `Parameters`), so a kind is present wherever ruff's AST has it; error-recovery placeholders too.
- **Names:** ruff's `NodeKind` and `TokenKind` names; pattern rules use them.
- **Errors:** ruff's errors cascade; a file reports the first error of each statement. Files with syntax errors are reported, not formatted or linted (standing decision).
- **Trivia:** comments, `NonLogicalNewline` and indent tokens are trivia; a BOM is leading trivia.
- **Semantic errors:** `ruff_python_parser`'s semantic syntax errors (duplicate parameters, `return` outside a function, ...) become lint rules in step 3.

## 2. Formatter
- Target: `ruff format` 0.17.4 (pinned for parity), whose output is Black's style with documented deviations; Black 26 as a second check on the black corpus.
- Options (each a named option, presets set them): `line_width` (88), `indent_style`/`indent_size` (4 spaces), `quote_style` (`double` | `single` | `preserve`), `magic_trailing_comma` (`respect` | `ignore`), `line_ending`, `docstring_code` (format code examples in docstrings: off, as ruff). Preset `python/black` only if Black's defaults differ from ours in an option (otherwise none: presets that set nothing are not made).
- Docstrings: indentation normalized and quotes as ruff; code in docstrings through embedding when `docstring_code` is on.
- Comments: Sudsy's token-owned trivia plus relocations, as JS. `# fmt: off` / `# fmt: on` / `# fmt: skip` and `# sudsy-ignore` are ignore markers.

## 3. Lint
- Catalog `docs/parity/ruff.yaml` from ruff's rule list; parity runner `scripts/parity.sh ruff` against pinned ruff on the corpus.
- First: ruff's default set (`F`, `E4`, `E7`, `E9`), syntax- and scope-level. Scope rules (unused imports and variables, undefined names, redefinitions) need a scope pass: ruff's semantic model is not a published crate, so Sudsy builds a small one (bindings, scopes, references) over its tree in `lang-python`, in the shape of `oxc_semantic` use in `lang-js` (decide in step 3 whether `core-semantics` can host it).
- Zero-config: the bug-catching rules (undefined names, unused imports at warn, duplicate keys, `return` outside function, ...) on by default; style rules off.
- Then families by usage, a step each later: `B` (bugbear), `UP` (pyupgrade), `SIM`, `I` (import sorting, as a formatter-adjacent rule with fix), `N`, `PL`.

## 4. Parity and corpus
- Format: `scripts/parity.sh format ruff-format` with ratchet groups: ruff's formatter fixtures (with their per-fixture options), black's `tests/data` (simple cases), and real-world Black-formatted projects (django, requests, and 2-3 more checked as `sudsy(x) == x`). Targets: 60% after step 2's first pass, 90% on real-world when step 2 closes; every remaining difference named.
- Lint: findings of ruff's default set on the same real-world corpus, 0 unexplained mismatches.

## 5. Steps
| Step | Builds | Est. lines |
|---|---|---|
| 1 | `lang-python`: dependency pin, adapter (tokens + AST walk + `Parenthesized`), lang.yaml mapping, provider and registration, verbatim formatter, bench input (cap 150 KB), corpus fetch script and round-trip test over it, `target_version`, parse-speed numbers in `docs/performance/numbers.md`, Markdown `python`/`py` fences (embedding), parity harness for format and lint (catalog `ruff.yaml`) | 1,500 |
| 2 | Formatter to the targets in section 4, options, docstrings, ignore markers, safety-check `value` hooks for restyled tokens (quotes, numeric literals) | 3,000 |
| 3 | Lint: scope pass, ruff default set, semantic syntax errors as rules, zero-config defaults, presets only where they set something | 2,000 |
| 4 | Migration: `sudsy init --from ruff` and `--from black` (pyproject `[tool.ruff]`, `ruff.toml`, `[tool.black]`), `.py` in Markdown fences checked, `per-file-ignores` and `extend-exclude` as `overrides`/`exclude` | 500 |

Later (not this wave): more rule families; notebooks (`.ipynb`, ruff's `ruff_notebook` crate); Pyrefly type-aware rules after the syntax sweep.
