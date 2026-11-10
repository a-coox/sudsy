#!/usr/bin/env bash
# Fetches the pinned GraphQL corpus into tests/conformance/corpus/graphql/<repo> (gitignored):
# Prettier's own GraphQL format tests, graphql-js's benchmark documents (a 368 KB GitHub schema),
# GraphiQL's schemas and queries, Apollo Federation's supergraph schemas and graphql-eslint's mocks.
source "$(dirname "$0")/tarball-corpus.sh"
fetch_tarballs graphql <<'EOF'
prettier/prettier 303363c6fa0298d8032eac15f116bc3faae23831 */tests/format/graphql/[!_]*/*.graphql
graphql/graphql-js 56a9830a16226ef8daa6ccb1375624e1c1933df4 */benchmark/*.graphql
graphql/graphiql d0d57d45717bf3639c01f5bbab1b582e7b5c01f3 */packages/graphql-language-service/benchmark/fixtures/*.graphql */packages/graphiql-toolkit/src/graphql-helpers/__tests__/*/*.graphql */packages/codemirror-graphql/src/__tests__/*.graphql */packages/vscode-graphql-syntax/tests/__fixtures__/*.graphql
apollographql/federation 85f9c11f4e6a95a4b8b27bc1704d549f9d519869 */internals-js/src/__tests__/*.graphql */query-planner-js/src/__tests__/*/*.graphql
graphql-hive/graphql-eslint 0f57486cad62cc2026e2c0f0224b3dec955c4a66 */packages/plugin/__tests__/mocks/*.graphql */packages/plugin/__tests__/mocks/*.gql */examples/*/*.graphql
EOF
