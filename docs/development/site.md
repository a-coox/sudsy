# Documentation site

Public rule reference at https://a-coox.github.io/sudsy/, built with Astro Starlight (versions pinned in `site/package.json`; latest version only). Everything outside `getting-started.md` is generated; the rest of `docs/` is for contributors and is not published.

## Build and preview
`scripts/site.sh build` (`npm ci`, generate, `astro build` into `site/dist`), `scripts/site.sh dev` for the dev server (no search index there; the rules index says so), `scripts/site.sh preview` to build and serve the site with search at http://localhost:4321/sudsy/. `scripts/site.sh test` builds, serves `site/dist` with `astro preview` and runs `site/test/smoke.mjs` (Playwright against the installed Edge or Chrome; no browser download): rule search, every filter, query-parameter preselection, the single Rules sidebar entry, a rule page (squiggles, configuration), an upstream-id redirect, the header search, and no console or network errors. Node 24. `scripts/site.sh scale-test [N]` builds with N synthetic rules (default 10 000) from a copy of `rules.json` in the build slot and prints build time and sizes ([performance/numbers.md](../performance/numbers.md)). The site is not part of `scripts/gate.sh`; the `site` workflow builds it.

## Data
`site/scripts/generate.mjs` writes Markdown into `site/src/content/docs/generated/` (git-ignored) before each build:
- `docs/rules/rules.json` (from `cargo xtask codegen`; `SUDSY_RULES_JSON` overrides the path): one page per rule at `/rules/<id>/` with description, languages, category, fix kind, options, upstream equivalents, the fixture example (input with the reported spans as red wavy underlines whose hover text is the rule id, then the fixed output) and one Configuration section (a `sudsy.yaml` snippet with the level and every option at its default, the options table, the presets that enable it and the suppression comment); a rule of several languages has one page. The underlines are the `squiggles="line:column:length,..." tip="..."` meta of a code block, handled by the small Expressive Code plugin in `site/ec.config.mjs`.
- Each upstream id (`SA1005`, `DL3008`) gets `/rules/<id>/` too: a meta refresh and canonical link to the Sudsy rule when one rule matches, a list when several do. Ids with characters outside `A-Za-z0-9_-` are slugified; an id equal to a Sudsy rule id is skipped.
- `crates/lang-*/presets/*.yaml` with the notes column of `docs/formatting/presets.md`: `/reference/presets/`.
- `schemas/sudsy.schema.json`: `/reference/format-options/`.

Hand-written: `src/content/docs/` (landing, getting started, rules index), `src/components/RuleSearch.astro`, `astro.config.mjs`.

## Search
Starlight's built-in Pagefind search covers every page. The rules index (`/rules/`, `RuleSearch.astro`) queries the same index through Pagefind's JS API (its default UI shows nothing without a search term): a search box, checkbox filters from the `data-pagefind-filter` chips on rule pages (`language`, `category`, `fix` (`none`, `safe`, `suggestion`, `unsafe`), `preset`, `upstream` (tool); several values of one filter match any), and the first 50 hits (sorted by id without a term). `?language=csharp`, `?preset=...` and `?q=...` preselect them. Only pages with a category (rule pages) are listed. A rule for `all` languages matches every language filter. The sidebar lists no rules (Starlight renders it into every page, so it would grow with the square of the rule count); it has one "Rules" entry, the index. The default level is not a filter: levels are configurable.

## Deploy
`.github/workflows/site.yml` builds the site and uploads the Pages artifact. It runs on pull requests and pushes to main that touch the site inputs (`site/**`, `docs/rules/rules.json`, `docs/formatting/presets.md`, the schema, preset files) and fails when `site/dist` exceeds 800 MB (the Pages limit is 1 GB); a push to main also deploys (Settings > Pages > Source is "GitHub Actions"). The VS Code extension's `DOCS_BASE_URL` and diagnostic links switch on after the site is live ([roadmap](../roadmap.md)).
