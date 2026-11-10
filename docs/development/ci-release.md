# CI and release

## CI (`.github/workflows/ci.yml`, reusable via `workflow_call`)
Runs on pushes to main, pull requests, dispatch and from the release workflow. A run for a push to main is never cancelled; other refs cancel their previous run. `scripts/ci-changes.sh` (job `changes`) skips the gates for changes that only touch root prose files, the license, bench history or `site/` (generated docs live in `docs/`, so those always run) and runs `package` only when `editors/vscode/`, `workers/csharp/`, `crates/lang-csharp/src/` or the workflows change (always on dispatch and tags). `scripts/gate.sh` stays the local pre-merge check.

`main` has a ruleset requiring the `CI ok` job (green when every job that ran passed); repository admins bypass it, so agents push directly. Auto-merge and branch deletion on merge are on.
| Job | Does |
|---|---|
| `changes` | what the change touches (above) |
| `dco` | pull requests only: every commit signed off by its author (`scripts/check-dco.sh`) |
| `rust` | Linux: fmt check, clippy `-D warnings`, `cargo test --workspace`, `cargo xtask codegen --check`, sample plugin fmt + tests (installs `wasm32-wasip2`) |
| `windows` | `cargo test --workspace` only (it builds and runs the worker, so there is no Windows dotnet job) |
| `bench` | Dependabot pull requests only: `scripts/bench.sh --all` tier 1 against the PR base (installs valgrind) |
| `dotnet` | Linux: `dotnet build` / `dotnet test workers/csharp` |
| `vscode` | `npm ci`, typecheck, lint, test, build (Linux) |
| `package` | release `sudsy`, published worker, staged server, `.vsix` artifact for `linux-x64`, `win32-x64` and `darwin-arm64` (3-day retention), after a smoke test that the staged binary formats C# without `SUDSY_CSHARP_WORKER` |
| `ci` | `CI ok`: the required check |

`.github/workflows/site.yml` builds the documentation site and deploys it to Pages from main, see [site.md](site.md).

Dependabot (`.github/dependabot.yml`: cargo, npm, nuget monthly, actions weekly, one grouped PR each) pull requests run this CI plus `bench`; `dependabot-automerge.yml` enables rebase auto-merge for patch and minor updates, majors wait for a human.

## Release (`.github/workflows/release.yml`, on a `vX.Y.Z` tag)
1. `scripts/check-version.sh`: one version for the Cargo workspace, `editors/vscode/package.json` + lock, the tag, and a `CHANGELOG.md` section. The sample plugin stays 0.0.0.
2. All of `ci.yml`.
3. One build job per platform on a native runner (linux, win32, darwin; x64 and arm64): packs (`scripts/pack.sh`), the pack-mode CLI embedding their hashes, the CLI archive and the offline `sudsy-full` archive with `packs/` (`scripts/package-cli.sh`), the pack files themselves as assets, and the platform `.vsix`, after the same smoke test ([language packs](../architecture/language-packs.md)).
4. A job drafts a GitHub release with archives and `SHA256SUMS`, notes from the changelog section. `-rc.N` suffixes make a pre-release.

Publishing is manual: the workflow publishes nothing by itself. The `marketplace` job runs only from a hand-started run on a tag with its input ticked, using the `VSCE_PAT` secret. Nothing goes to crates.io (every crate is `publish = false`; the SDK is used by path until the plugin API is stable) or NuGet (the worker ships inside the packages).

Supply chain: third-party actions are pinned by commit SHA (version in a comment; Dependabot bumps them weekly), workflows default to `contents: read` (only `release` writes), the tag name reaches scripts through `env`, and the `marketplace` job runs `vsce` from the `package-lock.json` install, not an unpinned download.

## Identity and contributions
- Marketplace publisher `Coox` (extension id `Coox.sudsy`); the repo stays under the owner's GitHub account.
- MIT, copyright Maia Coox; the `.vsix` ships the root `LICENSE` (copied at build).
- Logo: the Fluent UI Emoji "Soap" (MIT, Microsoft) in `assets/logo/` with its license; the `.vsix` icon (`icon.png`, copied at build), the site header logo and favicon, and the README use it. Attribution: `THIRD-PARTY-NOTICES.md`.
- Contributions need a DCO sign-off (`git commit -s`, see `CONTRIBUTING.md`); the `dco` CI job runs `scripts/check-dco.sh` on pull requests. No CLA.

## Still open
- Pack builds and `languages add --from` are proven locally on Windows x64 and Linux x64; macOS and arm64 builds are untested on real hardware until the first release run.
- Release is prepared, not performed (`CHANGELOG.md` `0.1.0` is "unreleased").
