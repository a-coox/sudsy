//! Putting a pack into the cache directory, and taking it out.

use crate::{Entry, PackError, cache_dir, entry, load::verify};
use std::path::Path;

/// Moves `part` to `dest` if its hash matches the manifest; deletes it otherwise.
fn finish(e: &Entry, part: &Path, dest: &Path) -> Result<(), PackError> {
    let checked = std::fs::read(part)
        .map_err(PackError::from)
        .and_then(|bytes| verify(e, &bytes));
    match checked {
        Ok(()) => Ok(std::fs::rename(part, dest)?),
        Err(err) => {
            let _ = std::fs::remove_file(part);
            Err(err)
        }
    }
}

fn part(e: &Entry) -> Result<std::path::PathBuf, PackError> {
    std::fs::create_dir_all(cache_dir())?;
    Ok(cache_dir().join(format!("{}.part", e.file)))
}

/// Installs the pack from a directory holding the release's files (`add --from`).
pub fn install_from(language: &str, dir: &Path) -> Result<(), PackError> {
    let e = entry(language)?;
    let part = part(&e)?;
    std::fs::copy(dir.join(e.file), &part)?;
    finish(&e, &part, &e.path())
}

/// Downloads the pack of the running version from the release assets (needs `curl`).
pub fn download(language: &str) -> Result<(), PackError> {
    let e = entry(language)?;
    let part = part(&e)?;
    let status = std::process::Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--output",
        ])
        .arg(&part)
        .arg(e.url())
        .status()
        .map_err(|x| PackError::Download(format!("cannot run curl: {x}")))?;
    if !status.success() {
        let _ = std::fs::remove_file(&part);
        return Err(PackError::Download(format!("{} ({status})", e.url())));
    }
    finish(&e, &part, &e.path())
}

/// Deletes the installed pack, if any.
pub fn remove(language: &str) -> Result<(), PackError> {
    match std::fs::remove_file(entry(language)?.path()) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pack_whose_hash_differs_is_refused_and_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let (part, dest) = (dir.path().join("p.part"), dir.path().join("p"));
        // Not the hash of "evil".
        let e = Entry {
            language: "x",
            file: "p",
            sha256: "ff2e3a5c6ba2dd5e4d1de19cb0b8a8f2b8a5d8a4d1d6a7b8b2d9e4c8a3d1f0aa",
            size: 4,
        };
        std::fs::write(&part, "evil").unwrap();
        assert!(matches!(
            finish(&e, &part, &dest),
            Err(PackError::Hash { .. })
        ));
        assert!(!part.exists() && !dest.exists());
        let e = Entry {
            sha256: &crate::load::sha256_hex(b"pack").leak()[..],
            ..e
        };
        std::fs::write(&part, "pack").unwrap();
        finish(&e, &part, &dest).unwrap();
        assert!(dest.exists());
    }
}
