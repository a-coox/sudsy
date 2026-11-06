//! Loading an installed pack: hash check, `dlopen`, ABI check.

use crate::{Entry, PackError, entry};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Mutex;
use tree_sitter::Language;
use tree_sitter_language::LanguageFn;

type Raw = unsafe extern "C" fn() -> *const ();

/// Libraries stay loaded for the life of the process (the parser tables are static data).
static LIBS: Mutex<Option<HashMap<&'static str, &'static libloading::Library>>> = Mutex::new(None);

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub(crate) fn verify(e: &Entry, bytes: &[u8]) -> Result<(), PackError> {
    let found = sha256_hex(bytes);
    if found == e.sha256 {
        return Ok(());
    }
    Err(PackError::Hash {
        file: e.file.into(),
        found,
        expected: e.sha256.into(),
    })
}

/// The grammar of `language` from its installed pack (`suffix` selects the symbol
/// `sudsy_language<suffix>`). Fails when the pack is missing, differs from the manifest, or its
/// tree-sitter ABI is not the runtime's.
#[allow(unsafe_code)] // dlopen and the call of the exported grammar function
pub fn load(language: &str, suffix: &str) -> Result<Language, PackError> {
    let e = entry(language)?;
    let mut libs = LIBS.lock().unwrap();
    let lib = match libs.get_or_insert_default().get(e.language) {
        Some(lib) => *lib,
        None => {
            let bytes = std::fs::read(e.path()).map_err(|err| match err.kind() {
                std::io::ErrorKind::NotFound => PackError::NotInstalled(language.into()),
                _ => err.into(),
            })?;
            verify(&e, &bytes)?;
            let load = |m: String| PackError::Load(e.file.into(), m);
            // SAFETY: the file is the one the release build produced (hash checked); its
            // initializers are the C grammar's, which run no code of their own.
            let lib =
                unsafe { libloading::Library::new(e.path()) }.map_err(|x| load(x.to_string()))?;
            let lib: &'static _ = Box::leak(Box::new(lib));
            libs.get_or_insert_default().insert(e.language, lib);
            lib
        }
    };
    let name = format!("sudsy_language{suffix}");
    // SAFETY: the symbol is `extern "C" fn() -> *const TSLanguage`, the packs' only export.
    let raw: Raw = *unsafe { lib.get::<Raw>(name.as_bytes()) }
        .map_err(|x| PackError::Load(e.file.into(), x.to_string()))?;
    // SAFETY: `raw` returns a pointer to the grammar's static `TSLanguage`.
    let lang = Language::from(unsafe { LanguageFn::from_raw(raw) });
    let abi = lang.abi_version();
    let (min, max) = (
        tree_sitter::MIN_COMPATIBLE_LANGUAGE_VERSION,
        tree_sitter::LANGUAGE_VERSION,
    );
    if !(min..=max).contains(&abi) {
        return Err(PackError::Abi(e.file.into(), abi, min, max));
    }
    Ok(lang)
}
