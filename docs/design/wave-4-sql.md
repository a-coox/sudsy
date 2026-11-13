# Design: SQL

Decided plan with one open measurement (step 0), built in the steps of section 5; each step moves what it builds into `docs/formatting/sql.md`, and this note is deleted when the last step lands.

Goal: the formatter and linter sqlfluff and sqruff users switch to: sqlfluff's layout and capitalisation rules (which together are its "formatter"), its bug-catching rules, per-dialect parsing, and migration from `.sqlfluff` / `pyproject.toml [tool.sqlfluff]` / `.sqruff`.

## Summary
1. Parser: **sqruff's `sqruff-lib-core` + `sqruff-lib-dialects`** (Apache-2.0, crates.io, 27 of sqlfluff's 28 dialects), pinned exactly, behind a segment-tree-to-lossless-tree adapter. Fallback if step 0 rejects it: tree-sitter-sql (`tree-sitter-sequel`, MIT) as a vendored grammar for an `ansi`-like dialect only.
2. One language `sql` for `.sql`; the dialect is an option (`dialect`), not a language: sqlfluff requires one, Sudsy defaults to `ansi`.
3. Formatter: sqlfluff's layout rules (LT01-LT15) and capitalisation (CP01-CP05) as the format, with sqlfluff's defaults (`max_line_length` 80, 4 spaces, trailing commas, capitalisation `consistent`).
4. Lint: sqlfluff's non-layout rules (AL, AM, CV, RF, ST, ...) by usage; zero-config set = the bug-catching ones (ambiguous, references, structure), style rules off.
5. Templated SQL (Jinja, dbt) is not formatted or linted in the first steps: a file containing `{{`, `{%` or `{#` is reported as templated and skipped (named difference), until a later step.

## 0. Spike (an agent, before step 1; measured, not committed)
Measure on sqlfluff's `test/fixtures/dialects` (each `.sql` with its `.yml` parse tree, MIT) and the PostgreSQL regression `sql/` files:
- sqruff-lib-core: round-trip (concatenated raw segments == input) per dialect; parse failures (`Unparsable`) vs sqlfluff's `.yml`; parse speed MB/s; peak memory; binary added to `sudsy.exe`; crates added and licenses (ICU, fancy-regex).
- tree-sitter-sequel: the same numbers on the ansi/postgres/mysql/sqlite fixtures.
Decide: sqruff unless round-trip fails, it is under 5 MB/s, or it adds over 4 MB; record the table here.

## 1. Parser and tree
- **Pin:** `=0.N.M` of both sqruff crates in the root `Cargo.toml`, Dependabot-ignored like oxc and ruff; a bump touches the kind table and adapter only.
- **Adapter:** sqruff segments map 1:1 to nodes (sqlfluff's segment type names, e.g. `select_statement`, `join_clause`, `keyword`), its whitespace, newline and comment segments are trivia; Indent/Dedent meta segments are dropped (zero-width, layout is Sudsy's).
- **Errors:** an `Unparsable` segment is a syntax error at its span; the file is reported, not formatted or linted (standing decision).
- **Dialect:** option `dialect` (sqlfluff names: `ansi`, `postgres`, `mysql`, `tsql`, `bigquery`, `snowflake`, ...), default `ansi`; the importer reads sqlfluff's `dialect`.

## 2. Formatter
- Target: `sqlfluff fix` with only the LT and CP rules enabled, pinned sqlfluff version for parity; sqruff as a second check (speed comparison).
- Options (each named, presets set them): `line_width` (80), `indent_style`/`indent_size` (space, 4), `comma_position` (`trailing` | `leading`), `keyword_case`, `identifier_case`, `function_case`, `literal_case`, `type_case` (`consistent` | `upper` | `lower` | `capitalise` | `preserve`), `line_ending`.
- Embedding: SQL in Markdown fences (`sql`) and, later, SQL strings in other languages are not in scope.

## 3. Lint
- Catalog `docs/parity/sqlfluff.yaml` from sqlfluff's 79 rules (AL 10, AM 9, CP 5, CV 13, JJ 1, LT 15, RF 7, ST 12, TQ 4, Oracle/Postgres dialect rules); parity runner `scripts/parity.sh sqlfluff` against pinned sqlfluff.
- First: AM (ambiguous), RF01-RF03 (references), ST (structure), CV rules that catch bugs, AL rules that are not style. Zero-config: AM and the structure/reference bug rules on; capitalisation and aliasing style off.
- Pattern rules over sqlfluff's segment names where a rule is structural.

## 4. Parity and corpus
- Format: sqlfluff's dialect fixtures run through `sqlfluff fix --rules LT,CP` (per-dialect ratchet groups), PostgreSQL regression files (postgres dialect), and one real-world dbt-free SQL project with a checked license. Targets: 60% after step 2's first pass, 90% when step 2 closes; every difference named.
- Lint: the zero-config set on the same corpus, 0 unexplained mismatches.

## 5. Steps
| Step | Builds | Est. lines |
|---|---|---|
| 0 | Spike: parser decision with measured table (section 0) | (not committed) |
| 1 | `lang-sql`: dependency pin, adapter, lang.yaml (dialect option), provider and registration, verbatim formatter, bench input (cap 150 KB, 3 dialects), corpus fetch script and round-trip test, parse-speed numbers, Markdown `sql` fences, parity harness and catalog | 1,200 |
| 2 | Formatter to section 4's targets: layout, indentation, line breaking, capitalisation options, safety-check `value` hooks for re-cased keywords | 2,500 |
| 3 | Lint: zero-config set, first rule families, presets only where they set something | 1,500 |
| 4 | Migration: `sudsy init --from sqlfluff` / `--from sqruff` (`.sqlfluff`, `setup.cfg`, `tox.ini`, `pyproject.toml`, `.sqruff`; `exclude_rules`, `rules`, per-rule sections) | 400 |

Later (not this wave): templated SQL (Jinja/dbt rendering and mapping back, as sqlfluff's templaters); SQL embedded in other languages' strings; the soql dialect.
