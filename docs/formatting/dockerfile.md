# Dockerfile (`lang-dockerfile`)

## Parser
tree-sitter-containerfile 0.9 (MIT, WharfLab's maintained fork of tree-sitter-dockerfile; follows BuildKit: heredocs, `--mount`, `# escape=` and `# syntax=` directives) linked in natively and converted by `core-treesitter` ([../architecture/adding-a-language.md](../architecture/adding-a-language.md)); the node mapping is `crates/lang-dockerfile/src/mapping.rs`, the tree shape `grammars/dockerfile.grammar`. A `Document` holds `Instruction`s; an `Instruction` is its `Keyword` and its arguments as flat tokens, plus the instruction `ONBUILD` or `HEALTHCHECK` holds. Options (`--chown=...`), paths, images, `KEY=value` pairs and quoted strings are one `Word` token each, the command of `RUN`/`CMD`/`ENTRYPOINT`/`HEALTHCHECK` one `ShellCommand` token (the shell of `RUN` is parsed beside the tree, see Rules), a heredoc body one `Heredoc` token. `STOPSIGNAL`, `MAINTAINER` and `CROSS_BUILD` hide their tokens in the grammar, so each is one `Raw` token. The grammar tracks the escape character itself: after `# escape=` + backtick, the line continuation token starts with a backtick.

Files: `Dockerfile`, `Containerfile`, `Dockerfile.*`, `Containerfile.*` (`Dockerfile.dev`) and `*.dockerfile`. A `file_names` entry ending in `*` matches by prefix.

Known gap: the grammar fails on a shell line that starts with `[` right after another continued line that starts with `[` (`[ -s a ]; \` then `[ -e b ]; \`: its external scanner takes the first one for an invalid JSON array and ends the command at the `\`). The file then has a syntax error and is not formatted. In the pinned corpus this hits 20 of the 40 files of docker-library/python and the Windows images of golang (4); the fix belongs in the grammar's scanner.

## Formatting
- The instruction name and `AS` are upper case (`from x as y` is `FROM x AS y`).
- One space between arguments wherever the source has blanks; none where it has none (`USER a:b`, `--chown=1:1`). Options, paths, images, pairs, quoted strings and shell text are kept as written, including blanks inside them (`ENV KEY  some value` keeps the value's blanks).
- JSON-form arguments (`CMD`, `ENTRYPOINT`, `RUN`, `COPY`, `ADD`, `VOLUME`, `SHELL`, `HEALTHCHECK CMD`): `["a", "b"]`, no blanks inside the brackets, one after each comma.
- A line continuation ends its line with one space before it, and what follows is indented one level (`indent_size`, default 4; `indent_style`); this also re-indents the continued lines of shell text and of multi-line pairs. Lines inside a quote are kept exactly (the blanks are content). Comment lines inside a continuation stay, indented the same. The escape character is the one the line uses, so `# escape=` is honoured.
- Heredoc bodies and their delimiter line are kept as written (trailing blanks included).
- Comments and parser directives stay where they are; blank lines between instructions are kept up to `max_blank_lines`.
- Not changed: the shell code itself (`&&` chains, `;`), legacy `ENV key value`, `MAINTAINER`. The generic safety check plus `DockerfileFormatter::extra_check` allow only case and blanks to change: every token keeps its kind and text with blanks removed (keywords compared case-insensitively).

dockerfmt (reteps/dockerfmt) is the comparable tool; it rewrites shell code with mvdan/sh, converts legacy `ENV` and `MAINTAINER`, and ships Linux and macOS binaries only, so there is no parity entry: its output would differ on nearly every `RUN`, and Sudsy's conservative layout leaves shell code to a later Shell language.

## Options
<!-- generated:options -->| Option | Values | Default | Description |
|---|---|---|---|
| `indent_size` | int 1-16 | `4` | Columns per indentation level (also the width of a tab). |
| `indent_style` | `space`, `tab` | `space` | Indent with spaces or tabs. |
| `final_newline` | bool | `true` | End the file with a newline. |
| `end_of_line` | `auto`, `lf`, `crlf` | `auto` | Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break. |
| `max_blank_lines` | int 0-10 | `1` | Largest number of blank lines kept between instructions and comments. |
<!-- /generated -->

## Presets
`dockerfile/hadolint`: turns on the rules that have a hadolint equivalent ([../parity/hadolint.yaml](../parity/hadolint.yaml)) at hadolint's severities (error stays error; warning, info and style become `warn`).

## Rules
Shared: suppression comments (`# sudsy-disable-next-line <rule>`), `no-empty-comment`, `comment-space`, `max-line-length`. The grammar has no `#mapping` or `#sequence` nodes, so the key rules do not apply. The language rules are listed in [../rules/builtin.md](../rules/builtin.md); the equivalents of hadolint's DL rules are in the catalog.
- Instruction shape (`rules/instructions.yaml`): pattern rules with a regex over the instruction text (WORKDIR, FROM, EXPOSE, CMD, COPY, ADD, USER, ENV).
- Commands of `RUN` (`rules/*.yaml` naming the `run-command` family, `src/rules/run_command.rs`): the commands come from the Shell tree of the `RUN` (its `Command` nodes: the first word is the program, quotes are dropped; the script of `RUN <<EOT` counts, and so does the JSON form, which has no shell), and a rule selects a program and subcommand, then needs a flag, a pinned package, a later clean command, or just reports the call. A command inside `$(...)`, backticks or `"$(...)"` is a `Command` node of the Shell tree and is read like the others, and a script that does not parse as shell is not checked. A `--mount` of type cache or tmpfs on a directory exempts the cleanup rules for it.
- Shell in `RUN` (`src/embedded.rs`): the command of `RUN` and the body of its heredoc are Shell code ([../architecture/adding-a-language.md](../architecture/adding-a-language.md#embedded-languages)). The comment lines inside a continued `RUN`, which Docker drops, are the Dockerfile's part of it. The shell rules (`cd-without-exit`, the shellcheck ones, the `shell:` section of the config) run on it, and a `# sudsy-disable-next-line` comment of the Dockerfile silences them. `RUN` text is formatted as written (above); only the instruction's line breaks are re-indented.
- Stages (`src/rules/stages.rs`, `rules/once-per-stage.yaml`): untagged base image, last `USER` root, stage names, `COPY --from`, the first instruction, a second `CMD`, `ENTRYPOINT` or `HEALTHCHECK` in a stage.
- Rules for informational hadolint checks are off by default (`apt-get-no-recommends`, `apk-no-cache`, `no-interactive-commands`, `wget-progress`, `numeric-user-id`); the preset turns them on.
- Known gap: a `#` inside an `ADD` source (`ADD ${REPO}#${REF} /`) makes the parser take the following lines into the instruction.

## Conformance
`tests/conformance/fetch-dockerfile-corpus.sh` fetches the Dockerfiles of dotnet/dotnet-docker (samples and the 10.0 runtime, aspnet and sdk images), docker-library (golang, python, nginx, postgres), moby and buildx at pinned commits; `cargo test --profile corpus -p lang-dockerfile --test dockerfile -- --ignored --nocapture conformance` checks that each file parses (but for the gap above), round-trips, keeps its instructions and arguments (the safety check, with comments compared as well) and formats idempotently.
