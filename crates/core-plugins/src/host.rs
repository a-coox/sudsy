//! One compiled plugin, the wasmtime engine it runs in and the limits every call gets.

use crate::rule::make_rules;
use core_rules::Rule;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;
use wasmtime::component::{Component, Linker, ResourceTable};
use wasmtime::{Cache, CacheConfig, Config, Engine, Store, StoreLimits, StoreLimitsBuilder};
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};

pub(crate) mod bindings {
    wasmtime::component::bindgen!({ path: "../../wit", world: "plugin" });
}

pub(crate) use bindings::exports::sudsy::rules::rules as w;

/// Fuel for one call: about one unit per Wasm instruction. It is the deterministic cap on a
/// runaway loop; the time limit below is what bounds the wait. Sized so that a plugin which only
/// reads a 3 MB file (decoding its tree takes ~1e9) stays far below it.
const FUEL: u64 = 20_000_000_000;
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

/// Names the directory of the compiled-plugin cache; `off` disables it.
pub const CACHE_ENV: &str = "SUDSY_WASM_CACHE";

/// Where compiled plugins are kept between runs: compiling a plugin takes longer than a small
/// lint run, loading the compiled code does not. `None` when the
/// user turned the cache off or has no home directory.
fn cache_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os(CACHE_ENV) {
        return (dir != "off").then(|| PathBuf::from(dir));
    }
    let var = |name: &str| std::env::var_os(name).map(PathBuf::from);
    let base = if cfg!(windows) {
        var("LOCALAPPDATA")
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|h| h.join("Library/Caches"))
    } else {
        var("XDG_CACHE_HOME").or_else(|| var("HOME").map(|h| h.join(".cache")))
    };
    base.map(|b| b.join("sudsy").join("wasm"))
}

fn compile_cache() -> Option<Cache> {
    let mut config = CacheConfig::new();
    config.with_directory(cache_dir()?);
    // A cache that cannot be opened (read-only home, say) only costs the speed-up.
    Cache::new(config).ok()
}

/// One engine for all plugins. A thread advances its epoch, which is what bounds the time of a
/// call; it ends with the engine.
static ENGINE: LazyLock<Engine> = LazyLock::new(|| {
    let mut config = Config::new();
    config.consume_fuel(true).epoch_interruption(true);
    config.cache(compile_cache());
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

/// A started instance of the plugin and its store.
struct Instance {
    store: Store<State>,
    plugin: bindings::Plugin,
}

pub(crate) struct Plugin {
    pub(crate) name: String,
    pre: bindings::PluginPre<State>,
    /// Instances that finished a call without trouble, for the next call to take. Starting an
    /// instance costs milliseconds (WASI linking, the module's start-up, the grammar the plugin
    /// parses on first use) against microseconds for a call, so each thread that works on files
    /// keeps its own instance. A plugin that traps loses its instance.
    idle: Mutex<Vec<Instance>>,
    /// Why the plugin is switched off for the rest of the run, once it is.
    disabled: Mutex<Option<String>>,
}

impl Plugin {
    fn new(pre: bindings::PluginPre<State>) -> Self {
        Plugin {
            name: String::new(),
            pre,
            idle: Mutex::default(),
            disabled: Mutex::default(),
        }
    }

    fn start(&self) -> Result<Instance, String> {
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
        store.epoch_deadline_trap();
        // Starting the instance runs plugin code too.
        store.set_fuel(FUEL).expect("fuel is enabled");
        store.set_epoch_deadline(TIMEOUT_TICKS);
        let plugin = self
            .pre
            .instantiate(&mut store)
            .map_err(|e| format!("cannot start the plugin: {e}"))?;
        Ok(Instance { store, plugin })
    }

    /// Runs `f` on an instance with its own fuel and time limits. An error is a trap, an
    /// exhausted limit or an error the plugin returned.
    pub(crate) fn call<T>(
        &self,
        f: impl FnOnce(&w::Guest, &mut Store<State>) -> wasmtime::Result<Result<T, String>>,
    ) -> Result<T, String> {
        let idle = self.idle.lock().expect("plugin instances").pop();
        let mut instance = match idle {
            Some(instance) => instance,
            None => self.start()?,
        };
        instance.store.set_fuel(FUEL).expect("fuel is enabled");
        instance.store.set_epoch_deadline(TIMEOUT_TICKS);
        match f(instance.plugin.sudsy_rules_rules(), &mut instance.store) {
            Ok(result) => {
                self.idle.lock().expect("plugin instances").push(instance);
                result
            }
            Err(trap) => Err(describe(&instance.store, &trap)),
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
                "the component does not implement sudsy:rules/rules@0.2 \
                 (rebuild it against this version of the SDK)",
                e,
            )
        })?;
    let mut plugin = Plugin::new(pre);
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
