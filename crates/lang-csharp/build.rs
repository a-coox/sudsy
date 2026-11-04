//! Lists `rules/*.yaml` for `include_str!`, so that adding a built-in pattern rule is adding a file.

use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=rules");
    let dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("rules");
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap()
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
            format!("    include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/rules/{name}\")),
")
        })
        .collect();
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("pattern_rules.rs");
    fs::write(out, format!("&[\n{items}]\n")).unwrap();
}
