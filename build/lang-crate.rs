//! Shared `build` script of the language crates (`build = "../../build/lang-crate.rs"`):
//! - lists `rules/*.yaml` for `include_str!`, so that adding a built-in pattern rule is adding a file;
//! - generates the typed wrappers (`generated.rs`: kinds, AST nodes, grammar hash) from
//!   `grammars/<name>.grammar`, `<name>` being the crate name without `lang-`.

use core_syntax::{Grammar, codegen};
use std::{env, fs, path::PathBuf};

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let dir = manifest.join("rules");
    // Without the directory, watch the crate so that creating it is noticed.
    println!(
        "cargo:rerun-if-changed={}",
        if dir.exists() { "rules" } else { "." }
    );
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "yaml"))
        .collect();
    files.sort();
    let items: String = files
        .iter()
        // File names only: the crate's own directory is resolved when it compiles, so a build
        // slot shared by several checkouts never reads another checkout's files.
        .map(|p| {
            let name = p.file_name().unwrap().to_str().unwrap();
            format!("    include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/rules/{name}\")),\n")
        })
        .collect();
    fs::write(out.join("pattern_rules.rs"), format!("&[\n{items}]\n")).unwrap();

    let name = env::var("CARGO_PKG_NAME").unwrap();
    let rel = format!("grammars/{}.grammar", name.trim_start_matches("lang-"));
    println!("cargo:rerun-if-changed=../../{rel}");
    let src = fs::read_to_string(manifest.join("../..").join(&rel)).unwrap();
    let grammar = Grammar::parse(&src).unwrap_or_else(|e| panic!("{rel}: {e}"));
    let module = codegen::rust_module(&grammar, &rel).unwrap_or_else(|e| panic!("{rel}: {e}"));
    fs::write(out.join("generated.rs"), module).unwrap();
}
