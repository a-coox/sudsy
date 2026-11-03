//! Writes a file so a crash or full disk never leaves it truncated.

use std::io::Write;
use std::path::Path;

/// Replaces `path` with `bytes`: a temporary file in the same directory is written, synced and
/// renamed over the target, keeping its permissions. A symlink is resolved first, so the link
/// stays a link and its target is rewritten.
pub fn write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let target = std::fs::canonicalize(path)?;
    let dir = target.parent().unwrap_or(Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(dir)?;
    temp.write_all(bytes)?;
    temp.as_file()
        .set_permissions(std::fs::metadata(&target)?.permissions())?;
    temp.as_file().sync_all()?;
    temp.persist(&target).map_err(|e| e.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::os::unix::fs::symlink as link_file;
    #[cfg(windows)]
    use std::os::windows::fs::symlink_file as link_file;

    fn symlink(to: &std::path::Path, at: &std::path::Path) -> bool {
        link_file(to, at).is_ok()
    }

    #[test]
    fn replaces_content_keeps_the_symlink_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.cs");
        std::fs::write(&file, "old").unwrap();
        super::write(&file, b"new").unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"new");
        let link = dir.path().join("link.cs");
        // Creating a symlink needs privileges on Windows; skip the link part there.
        if symlink(&file, &link) {
            super::write(&link, b"via link").unwrap();
            assert!(std::fs::symlink_metadata(&link).unwrap().is_symlink());
            assert_eq!(std::fs::read(&file).unwrap(), b"via link");
        }
        let names = std::fs::read_dir(dir.path()).unwrap().count();
        assert_eq!(names, if link.exists() { 2 } else { 1 });
    }
}
