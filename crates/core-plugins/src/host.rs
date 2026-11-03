//! One compiled plugin, the wasmtime engine it runs in and the limits every call gets.

use crate::rule::make_rules;
use core_rules::Rule;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;
use wasmtime::component::{Component, Linker, ResourceTable};
use wasmtime::{Config, Engine, Store, StoreLimits, StoreLimitsBuilder};
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};

pub(crate) mod bindings {
    wasmtime::component::bindgen!({ path: "../../wit", world: "plugin" });
}

pub(crate) use bindings::exports::sudsy::rules::rules as w;

/// Fuel for one call: about one unit per Wasm instruction, so a few seconds of work.
const FUEL: u64 = 2_000_000_000;
/// Wall-clock limit for one call, in epoch ticks of [`TICK`].
const TIMEOUT_TICKS: u64 = 200;
const TICK: Duration = Duration::from_millis(50);
/// The most memory a plugin may grow to.
const MEMORY: usize = 512 << 20;

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct PluginError(pub String);

fn err(context: &str, e: impl std::fmt::Display) -> PluginError {
    PluginError(format!("{context}: {e}"))
}

pub(crate) struct State {
    wasi: WasiCtx,
    table: ResourceTable,
    limits: StoreLimits,
}

impl WasiView for State {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

/// One engine for all plugins. A thread advances its epoch, which is what bounds the time of a
/// call; it ends with the engine.
static ENGINE: LazyLock<Engine> = LazyLock::new(|| {
    let mut config = Config::new();
    config.consume_fuel(true).epoch_interruption(true);
    let engine = Engine::new(&config).expect("wasmtime engine");
    let weak = engine.weak();
    std::thread::spawn(move || {
        while let Some(engine) = weak.upgrade() {
            engine.increment_epoch();
            drop(engine);
            std::thread::sleep(TICK);
        }
    });
    engine
});

pub(crate) struct Plugin {
    pub(crate) name: String,
    pre: bindings::PluginPre<State>,
    /// Why the plugin is switched off for the rest of the run, once it is.
    disabled: Mutex<Option<String>>,
}

impl Plugin {
    fn store() -> Store<State> {
        // No preopened directories, environment, arguments or inherited streams: the plugin
        // can compute, and nothing else.
        let mut store = Store::new(
            &ENGINE,
            State {
                wasi: WasiCtx::builder().build(),
                table: ResourceTable::new(),
                limits: StoreLimitsBuilder::new().memory_size(MEMORY).build(),
            },
        );
        store.limiter(|state| &mut state.limits);
        store.set_fuel(FUEL).expect("fuel is enabled");
        store.set_epoch_deadline(TIMEOUT_TICKS);
        store.epoch_deadline_trap();
        store
    }

    /// Runs `f` with a fresh instance and its own fuel and time limits. An error is a trap, an
    /// exhausted limit or an error the plugin returned.
    pub(crate) fn call<T>(
        &self,
        f: impl FnOnce(&w::Guest, &mut Store<State>) -> wasmtime::Result<Result<T, String>>,
    ) -> Result<T, String> {
        let mut store = Self::store();
        let plugin = self
            .pre
            .instantiate(&mut store)
            .map_err(|e| format!("cannot start the plugin: {e}"))?;
        match f(plugin.sudsy_rules_rules(), &mut store) {
            Ok(result) => result,
            Err(trap) => Err(describe(&store, &trap)),
        }
    }

    pub(crate) fn disabled(&self) -> Option<String> {
        self.disabled.lock().expect("plugin state").clone()
    }

    /// Switches the plugin off; true when this call did (so it is reported once).
    pub(crate) fn disable(&self, reason: String) -> bool {
        let mut state = self.disabled.lock().expect("plugin state");
        let first = state.is_none();
        state.get_or_insert(reason);
        first
    }
}

fn describe(store: &Store<State>, trap: &wasmtime::Error) -> String {
    match store.get_fuel() {
        Ok(0) => "it used up its fuel (too much work for one file)".into(),
        _ => {
            let text = format!("{trap:#}");
            if text.contains("epoch") || text.contains("interrupt") {
                "it ran past its time limit".into()
            } else {
                format!("it trapped: {text}")
            }
        }
    }
}

/// Compiled components by the hash of their bytes: compiling is the expensive part of loading a
/// plugin, and each `load` still gets a plugin of its own (with its own disabled state).
static COMPONENTS: LazyLock<Mutex<HashMap<[u8; 32], Component>>> = LazyLock::new(Mutex::default);

/// Loads the component in `bytes` and returns its rules as `(language, rule)`, ids prefixed with
/// the plugin's name. `path` is only used in errors. Each call makes a new plugin: the rules of
/// one call share it, so one that misbehaves switches off the whole plugin, but not the plugin a
/// later call loads from the same bytes.
pub fn load(
    path: &Path,
    bytes: &[u8],
) -> Result<Vec<(&'static str, &'static dyn Rule)>, PluginError> {
    let key: [u8; 32] = Sha256::digest(bytes).into();
    let cached = COMPONENTS
        .lock()
        .expect("component cache")
        .get(&key)
        .cloned();
    let component = match cached {
        Some(component) => component,
        None => {
            let component = Component::new(&ENGINE, bytes).map_err(|e| {
                err(
                    &format!(
                        "{} is not a Wasm component (build with --target wasm32-wasip2)",
                        path.display()
                    ),
                    e,
                )
            })?;
            COMPONENTS
                .lock()
                .expect("component cache")
                .insert(key, component.clone());
            component
        }
    };
    let mut linker = Linker::<State>::new(&ENGINE);
    wasmtime_wasi::p2::add_to_linker_sync(&mut linker).map_err(|e| err("linking WASI", e))?;
    let pre = linker
        .instantiate_pre(&component)
        .and_then(bindings::PluginPre::new)
        .map_err(|e| {
            err(
                "the component does not implement sudsy:rules/rules@0.1 \
                 (rebuild it against this version of the SDK)",
                e,
            )
        })?;
    let mut plugin = Plugin {
        name: String::new(),
        pre,
        disabled: Mutex::new(None),
    };
    let info = plugin
        .call(|guest, store| guest.call_info(store).map(Ok))
        .map_err(|e| err("asking the plugin for its rules", e))?;
    if info.api != crate::API {
        return Err(PluginError(format!(
            "the plugin was built for {} but this Sudsy speaks {}; the plugin API is unstable, \
             rebuild the plugin against this version of the SDK",
            info.api,
            crate::API
        )));
    }
    if info.name.is_empty()
        || !info
            .name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(PluginError(format!(
            "the plugin name `{}` must be letters, digits, `-` and `_`",
            info.name
        )));
    }
    plugin.name = info.name.clone();
    make_rules(Arc::new(plugin), info)
}
