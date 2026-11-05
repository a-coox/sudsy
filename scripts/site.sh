#!/usr/bin/env bash
# Documentation site (site/). Usage: scripts/site.sh build | dev | preview | test | scale-test [N]
#   build        npm ci + generate pages from docs/rules/rules.json + astro build -> site/dist
#   test         build + browser smoke test (site/test/smoke.mjs; Playwright drives installed Edge or Chrome)
#   dev          generate + astro dev server (no search: the index exists only in a build)
#   preview      build, then serve site/dist with search at http://localhost:4321/sudsy/
#   scale-test   build with N (default 10000) synthetic rules from a copy of rules.json in the build
#                slot ($CARGO_TARGET_DIR/site-scale); prints build time and output/search index sizes
set -euo pipefail
cd "$(dirname "$0")/../site"
case "${1:-build}" in
  build) npm ci && npm run build ;;
  dev) npm ci && npm run dev ;;
  preview) npm ci && npm run build && npm run preview ;;
  test) npm ci && npm run build && node --test test/smoke.mjs ;;
  scale-test)
    n=${2:-10000}
    dir=${CARGO_TARGET_DIR:-../target}/site-scale
    rm -rf "$dir" && mkdir -p "$dir"
    node -e '
      const [src, dst, n] = process.argv.slice(1);
      const data = JSON.parse(require("fs").readFileSync(src, "utf8"));
      const real = data.rules;
      const rules = Array.from({ length: +n }, (_, i) => {
        const r = structuredClone(real[i % real.length]);
        r.id += `-${i}`;
        r.upstream.forEach((u) => (u.id += `${i}`));
        return r;
      });
      require("fs").writeFileSync(dst, JSON.stringify({ languages: data.languages, rules }));
    ' ../docs/rules/rules.json "$dir/rules.json" "$n"
    npm ci
    export SUDSY_RULES_JSON=$(cd "$dir" && pwd)/rules.json SITE_OUT=$(cd "$dir" && pwd)/dist
    start=$(date +%s)
    npm run build >"$dir/build.log" 2>&1 || { tail -30 "$dir/build.log"; exit 1; }
    end=$(date +%s)
    echo "rules: $n  build: $((end - start)) s"
    echo "pages: $(find "$SITE_OUT" -name index.html | wc -l)"
    echo "output: $(du -sk "$SITE_OUT" | cut -f1) KB, search index: $(du -sk "$SITE_OUT/pagefind" | cut -f1) KB"
    env -u SUDSY_RULES_JSON npm run generate >/dev/null  # leave the real pages in place
    ;;
  *) echo "usage: scripts/site.sh build | dev | preview | test | scale-test [N]" >&2; exit 2 ;;
esac
