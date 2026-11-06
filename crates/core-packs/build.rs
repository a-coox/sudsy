//! Embeds the pack manifest: `$SUDSY_PACK_MANIFEST` (written by `scripts/pack.sh`), else empty.

fn main() {
    println!("cargo:rerun-if-env-changed=SUDSY_PACK_MANIFEST");
    let text = match std::env::var("SUDSY_PACK_MANIFEST") {
        Ok(path) => {
            println!("cargo:rerun-if-changed={path}");
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
        }
        Err(_) => String::new(),
    };
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("manifest.txt");
    std::fs::write(out, text).unwrap();
}
