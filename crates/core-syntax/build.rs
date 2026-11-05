//! Writes `grammars::registry`: one loader per `grammars/*.grammar` and the lookup by language
//! name, so adding a grammar file is the whole registration.

use std::{env, fs, path::PathBuf};

fn main() {
    let dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../grammars");
    println!("cargo:rerun-if-changed=../../grammars");
    let mut names: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "grammar"))
        .map(|p| p.file_stem().unwrap().to_str().unwrap().to_string())
        .collect();
    names.sort();
    let mut out = String::new();
    for name in &names {
        out += &format!(
            "pub fn {name}() -> &'static Grammar {{\n    static G: OnceLock<Grammar> = OnceLock::new();\n    \
             load(&G, include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/../../grammars/{name}.grammar\")))\n}}\n\n"
        );
    }
    out += &format!(
        "/// The built-in grammar of `language` (its `language` line), if there is one.\n\
         pub fn by_name(language: &str) -> Option<&'static Grammar> {{\n    \
         [{}].into_iter().map(|g| g()).find(|g| g.language == language)\n}}\n",
        names.join(", ")
    );
    fs::write(
        PathBuf::from(env::var("OUT_DIR").unwrap()).join("registry.rs"),
        out,
    )
    .unwrap();
}
