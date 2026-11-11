#!/usr/bin/env bash
# Fetches the pinned CSS/SCSS/Less corpus into tests/conformance/corpus/css/<repo> (gitignored):
# Prettier's own css, scss and less format-test inputs (tag 3.9.10; no snapshots or runners) and real
# projects: Bootstrap 5.3.3 (scss and dist css), Bulma 1.0.2 (sass sources and css), ant-design 4.24.16
# (less), Semantic UI 2.5.0 (less), normalize.css 8.0.1 and the docusaurus 3.9.2 themes
# (Prettier-formatted css).
source "$(dirname "$0")/tarball-corpus.sh"
fetch_tarballs css <<'EOF'
prettier/prettier 1172b82c295c4f889fd787dc6c78c3e194ebf353 */tests/format/css/* */tests/format/scss/* */tests/format/less/*
twbs/bootstrap 6e1f75f420f68e1d52733b8e407fc7c3766c9dba */scss/*.scss */dist/css/bootstrap.css
jgthms/bulma 3fd2f6502678c09dc39d2fe37980b88c2c5bc534 */sass/*.scss */css/bulma.css
ant-design/ant-design 7c060247516925f48175d7f29cd43b626632665e */components/*.less
Semantic-Org/Semantic-UI decf1883804ef205db0ce0ea7d73b9cb6aaf31c1 */src/definitions/*.less
necolas/normalize.css fc091cce1534909334c1911709a39c22d406977b */normalize.css
facebook/docusaurus abfbe5621b08407bc3dcbe6111ff118d4c22f7a1 */packages/docusaurus-theme-classic/src/*.css
EOF
prettier="$(dirname "$0")/corpus/css/prettier"
find "$prettier" \( -name __snapshots__ -o -name _errors_ \) -prune -exec rm -rf {} +
find "$prettier" -type f ! -regex '.*\.\(css\|scss\|less\)' -delete
