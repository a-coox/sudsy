# Documentation site

Public rule reference at https://a-coox.github.io/sudsy/, built with Astro Starlight (versions pinned in `site/package.json`; latest version only). Everything outside `getting-started.md` is generated; the rest of `docs/` is for contributors and is not published.

## Build and preview
`scripts/site.sh build` (`npm ci`, generate, `astro build` into `site/dist`), `scripts/site.sh dev` for the dev server. Node 24. `scripts/site.sh scale-test [N]` builds with N synthetic rules (default 10 000) from a copy of `rules.json` in the build slot and prints build time and sizes ([performance/numbers.md](../performance/numbers.md)). The site is not part of `scripts/gate.sh`; the `site` workflow builds it.

## Data
`site/scripts/generate.mjs` writes Markdown into `site/src/content/docs/generated/` (git-ignored) before each build:
- `docs/rules/rules.json` (from `cargo xtask codegen`; `SUDSY_RULES_JSON` overrides the path): one page per rule at `/rules/<id>/` with description, languages, category, default level, fix kind, options, presets, upstream equivalents and the fixture example (input with the reported spans marked by `^`, then the fixed output); a rule of several languages has one page.
- Each upstream id (`SA1005`, `DL3008`) gets `/rules/<id>/` too: a meta refresh and canonical link to the Sudsy rule when one rule matches, a list when several do. Ids with characters outside `A-Za-z0-9_-` are slugified; an id equal to a Sudsy rule id is skipped.
- `crates/lang-*/presets/*.yaml` with the notes column of `docs/formatting/presets.md`: `/reference/presets/`.
- `schemas/sudsy.schema.json`: `/reference/format-options/`.

Hand-written: `src/content/docs/` (landing, getting started, rules index), `src/components/RuleSearch.astro`, `astro.config.mjs`.

## Search
Starlight's built-in Pagefind search covers every page. The rules index (`/rules/`) is Pagefind's default UI over the same index; filters are the `data-pagefind-filter` chips on rule pages: `language`, `category`, `level` (default level), `fix` (`none`, `safe`, `suggestion`, `unsafe`), `preset`, `upstream` (tool). `?language=csharp`, `?preset=...` and `?q=...` preselect them. The sidebar lists no rules (Starlight renders it into every page, so it would grow with the square of the rule count); it links to the index per language.

## Deploy
`.github/workflows/site.yml` builds the site and uploads the Pages artifact; it runs by hand (like `ci.yml`) while the repository is private. Its `deploy` job has `if: false`. To switch it on once the repository is public: add the push and pull request triggers listed in the workflow's comment, set Settings > Pages > Source to "GitHub Actions", then replace `false` with `github.event_name != 'pull_request'` in the job. The VS Code extension's `DOCS_BASE_URL` and diagnostic links switch on after the site is live ([roadmap](../roadmap.md)).
