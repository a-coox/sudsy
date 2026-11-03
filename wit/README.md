# wit/

`rules.wit` is the Wasm plugin interface (`sudsy:rules@0.2.0`, unstable until v1). The host is
`crates/core-plugins`; the Rust SDK for authors is `crates/plugin-sdk`. A plugin in another
language needs only this file: build a component that exports `sudsy:rules/rules`, and read the
comments in the file for what crosses the boundary.
