//! The Wasm plugin host: loads components that implement
//! `wit/rules.wit` and makes their rules ordinary [`core_rules::Rule`]s, so a plugin rule goes
//! through the same engine, configuration, suppressions and fix loop as a built-in one.
//!
//! * [`host`]: the wasmtime engine, one compiled plugin and its limits.
//! * [`rule`]: [`WasmRule`], the `Rule` that forwards a file to its plugin.
//!
//! API v0 is unstable; see `wit/rules.wit`.

mod host;
mod rule;

pub use host::{PluginError, load};
pub use rule::WasmRule;

/// The interface version this host speaks; a plugin built against another is refused.
pub const API: &str = "rules@0.2";
