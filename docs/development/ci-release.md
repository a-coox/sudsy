# CI and release

## CI (`.github/workflows/ci.yml`, reusable via `workflow_call`)
| Job | Does |
|---|---|
| `dco` | pull requests only: every commit signed off by its author (`scripts/check-dco.sh`) |
| `rust` | fmt check, clippy `-D warnings`, `cargo test --workspace`, sample plugin fmt + tests (installs `wasm32-wasip2`) |
| `bench` | `scripts/bench.sh` tier 1 on ubuntu-latest against the PR base (installs valgrind) |
| `dotnet` | `dotnet build` / `dotnet test workers/csharp` |
| `vscode` | `npm ci`, typecheck, lint, test, build (Linux) |
| `package` | release `sudsy`, published worker, staged server, `.vsix` artifact for `linux-x64` and `win32-x64`, after a smoke test that the staged binary formats C# without `SUDSY_CSHARP_WORKER` |

## Release (`.github/workflows/release.yml`, on a `vX.Y.Z` tag)
1. `scripts/check-version.sh`: one version for the Cargo workspace, `editors/vscode/package.json` + lock, the tag, and a `CHANGELOG.md` section. The sample plugin stays 0.0.0.
2. All of `ci.yml`.
3. One build job per platform on a native runner (linux, win32, darwin; x64 and arm64): CLI archive (`scripts/package-cli.sh`) and the platform `.vsix`, after the same smoke test.
4. A job drafts a GitHub release with archives and `SHA256SUMS`, notes from the changelog section. `-rc.N` suffixes make a pre-release.

Publishing is manual: the workflow publishes nothing by itself. The `marketplace` job runs only from a hand-started run on a tag with its input ticked, using the `VSCE_PAT` secret. Nothing goes to crates.io (every crate is `publish = false`; the SDK is used by path until the plugin API is stable) or NuGet (the worker ships inside the packages).

## Identity and contributions
- Marketplace publisher `Coox` (extension id `Coox.sudsy`); the repo stays under the owner's GitHub account.
- MIT, copyright Maia Coox; the `.vsix` ships the root `LICENSE` (copied at build).
- Contributions need a DCO sign-off (`git commit -s`, see `CONTRIBUTING.md`); the `dco` CI job runs `scripts/check-dco.sh` on pull requests. No CLA.

## Still open
- macOS and arm64 builds are untested on real hardware until the first release run.
- Release is prepared, not performed (`CHANGELOG.md` `0.1.0` is "unreleased").
