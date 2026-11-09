# Language packs

Every language is installable and removable. JSON, YAML and the C# worker are in the release by default; each tree-sitter grammar (TOML, XML, Markdown, EditorConfig, INI, Gitignore, Dockerfile, Shell, PowerShell) is a native shared library, a **pack**, that is installed per user and loaded on first use. A pack parses at the speed of a linked-in grammar (same C code, same optimization level); Wasm grammars were rejected as slower. The language's Rust code (formatter, rules, mapping) stays in the `sudsy` binary; a pack holds only `parser.c` and the scanner.

## Two builds
- **`static-grammars`** (cargo feature of `cli`, `lsp`, `languages` and the `lang-*` crates; on by default, so `cargo build`, tests, the dev loop and `cargo install` are unchanged): the grammars are linked in. Every language is installed unless removed.
- **Pack mode** (`--no-default-features`): the grammar crates are not compiled; `core_packs::language!` in each `lang-*` provider loads the pack instead.

`Spec::language` is a function, resolved on the first parse, so building all providers loads nothing.

## Pack
- `crates/pack` is a `cdylib` with one cargo feature per language (the grammar crate behind it) exporting `sudsy_language` (Markdown also `sudsy_language_inline` for its inline grammar). The grammar crate's own `tree_sitter_<lang>` symbol is not exported: a Rust cdylib would define it twice.
- File name: `sudsy-<version>-<language>-<target triple>.<dll|so|dylib>`. Packs are locked to the Sudsy version by the name; a different version looks for a different file.
- `scripts/pack.sh <out> [language...]` builds them for this machine and writes `<out>/manifest.txt`.
- Release: each platform's build job runs `pack.sh`, builds the CLI with `--no-default-features` and `SUDSY_PACK_MANIFEST` pointing at that manifest (so the hashes in the binary are those of the packs uploaded next to it), and publishes the pack files as release assets (their names carry the target triple, so platforms do not collide). Archives: `sudsy-<version>-<platform>` (no packs; `languages add` downloads them) and `sudsy-full-<version>-<platform>` (same binary plus `packs/`, for offline use: `sudsy languages add --all --from packs`). The VS Code `.vsix` carries the pack-mode binary.

## Manifest and checks (`core-packs`)
- The binary embeds `manifest.txt` (`language file sha256 size` per line; `$SUDSY_PACK_MANIFEST` at build time, empty otherwise, so a build without one lists no pack).
- Load order: read the file, compare its SHA-256 with the manifest (mismatch: refused, the file is not loaded), `dlopen`, resolve the symbol, compare `Language::abi_version()` with the runtime's `MIN_COMPATIBLE_LANGUAGE_VERSION..=LANGUAGE_VERSION`. The library is kept loaded for the process. Hash and `dlopen` read the file separately, so someone who can write the cache directory could swap it in between; they can also replace the binary.
- `add` verifies the hash before the file reaches the cache directory (`<name>.part`, then renamed).

## Installed set
- Directory: `$SUDSY_CACHE_DIR`, else `<platform cache dir>/sudsy` (`dirs`): the packs and `state.json` (`{"removed": [language...]}`).
- A language is usable when it is not in `removed` and, in pack mode, its pack loads (JSON, YAML and C# need no pack). `remove` adds it to `removed` and deletes its pack; `add` clears the flag and, if the pack does not load, installs it. In a static build `add`/`remove` only flip the flag.
- Download: `curl` on `https://github.com/a-coox/sudsy/releases/download/v<version>/<file>` (`core_packs::RELEASE_URL`, the one place). Nothing downloads unless the user runs `sudsy languages add`.

## Commands
[config/cli.md](../config/cli.md): `sudsy languages list|add|remove|sync`. `list` shows installed or available, and the pack size (`built in` for the hand-written languages, `no pack in this build` when the manifest lacks one).

## Missing language
`format`, `lint`, `fix` and `check` skip the files of a known language that is not usable, print one stderr line per language (`warning: 2 toml file(s) skipped: not installed; run `sudsy languages add toml``; a pack that fails its checks gives the reason instead) and exit 3 when nothing worse happened. A language listed `ignored` under `languages:` in `sudsy.yaml` is skipped silently with the normal status ([config/sudsy-yaml.md](../config/sudsy-yaml.md)). The LSP keeps the document anyway (invisible to diagnostics, formatting and actions) and sends one `window/showMessageRequest` per language and session with the action `Install`. Choosing it runs the code of `sudsy languages add <language>` (a download blocks the message loop meanwhile), then serves the waiting documents and asks the client to refresh diagnostics; a failure is a `window/showMessage` error. Nothing is downloaded before the choice. A client that ignores the prompt keeps the document skipped.
