//! Atomic file writes for state-of-truth files (`pilots.json`,
//! `config.json`).
//!
//! `std::fs::write` writes in-place: open the destination file,
//! truncate, write bytes, close. A power loss or process crash
//! between the truncate and the close leaves a zero-byte / partial
//! file — corruption for the user's whole pilot roster or settings
//! page. Empirically this was rare but the consequence is total
//! data loss (Bifrost's `load_or_default` would silently fall back
//! to defaults, hiding the actual stored data behind whatever the
//! defaults compute).
//!
//! The fix is the textbook "write-temp-then-rename" pattern:
//!
//!   1. Write the new contents to `<path>.tmp` in the same directory
//!   2. fsync (best-effort on Windows where it's a no-op for the
//!      target file but not the directory)
//!   3. Atomically `rename(<path>.tmp, <path>)`
//!
//! Step 3 is atomic on Windows for same-volume renames (`MoveFileEx`
//! with `MOVEFILE_REPLACE_EXISTING` semantics, which `std::fs::rename`
//! uses on Windows). On the BSDs / Linux it's atomic via POSIX
//! rename(2). Result: a reader sees either the fully-written old
//! file or the fully-written new file, never a half-rewritten one.
//!
//! The `.tmp` suffix is in the same directory rather than the OS
//! temp dir so the rename can't ever cross a filesystem boundary
//! (which would degrade to copy + unlink and lose atomicity).

use std::path::Path;

use crate::error::Result;

/// Atomically replace `path` with `contents`. Creates parent
/// directories if missing.
///
/// On failure to rename the temp file into place, the partial tmp
/// file is best-effort cleaned up so we don't litter `.tmp` files
/// in the user's AppData on repeated failures. The original file
/// (if any) is untouched on any failure path.
pub fn write_atomic(path: &Path, contents: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Append `.tmp` to the FULL path (including extension) so
    // `pilots.json.tmp` is unambiguously a temp sibling of
    // `pilots.json`. The alternative (replacing extension) would
    // produce `pilots.tmp` which is more ambiguous and could collide
    // with anything else using the stem.
    let tmp_path = {
        let mut p = path.as_os_str().to_owned();
        p.push(".tmp");
        std::path::PathBuf::from(p)
    };

    // Write + close the temp file. Scoping the File in its own block
    // makes the close explicit before the rename.
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&tmp_path)?;
        f.write_all(contents)?;
        // Best-effort sync to flush OS caches to disk before the
        // rename. Errors are ignored — Windows treats this as a
        // no-op for most filesystems, and a sync failure doesn't
        // make the rename less atomic.
        let _ = f.sync_all();
    }

    // Atomic rename. If this fails (cross-device, AV interference)
    // the original file at `path` is untouched and we should clean
    // up the tmp file so a retry doesn't see stale contents.
    if let Err(e) = std::fs::rename(&tmp_path, path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(e.into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Round-trip: write then read back. Pins the basic contract that
    /// `write_atomic` is in fact a write (vs. e.g. silently dropping
    /// on Windows for some path-handling reason).
    #[test]
    fn write_atomic_persists_contents() {
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("data.json");
        write_atomic(&path, b"hello world").expect("write");
        let read = std::fs::read(&path).expect("read");
        assert_eq!(read, b"hello world");
    }

    /// Writing over an existing file replaces its contents fully —
    /// the "rename over destination" behaviour. Catches a Windows
    /// quirk where `rename` to an existing target sometimes fails
    /// without explicit replace-existing flags.
    #[test]
    fn write_atomic_replaces_existing_file() {
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("data.json");
        std::fs::write(&path, b"original").expect("seed");
        write_atomic(&path, b"replaced").expect("write");
        let read = std::fs::read(&path).expect("read");
        assert_eq!(read, b"replaced");
    }

    /// No stray `.tmp` file left behind after a successful write —
    /// the rename consumed it. If this regresses we'd accumulate
    /// `.tmp` siblings in the user's AppData over time.
    #[test]
    fn write_atomic_cleans_up_tmp_on_success() {
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("data.json");
        write_atomic(&path, b"x").expect("write");

        let tmp_path = {
            let mut p = path.as_os_str().to_owned();
            p.push(".tmp");
            std::path::PathBuf::from(p)
        };
        assert!(
            !tmp_path.exists(),
            ".tmp file must not exist after successful write"
        );
    }

    /// Parent directories are created on demand — the first ever
    /// `save_pilots()` call on a fresh install has to work before
    /// the AppData directory exists.
    #[test]
    fn write_atomic_creates_parent_dirs() {
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("deeply/nested/dir/data.json");
        write_atomic(&path, b"x").expect("write");
        assert!(path.exists());
    }

    /// Two writes back-to-back leave only the second value visible.
    /// Pins the "no partial / interleaved state" contract that the
    /// atomic rename provides — even though this test is sequential,
    /// a regression to in-place writes would still satisfy this
    /// (the value would be the second one) so combine with the
    /// `cleans_up_tmp_on_success` test for the full guarantee.
    #[test]
    fn write_atomic_back_to_back_lands_last_value() {
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("data.json");
        write_atomic(&path, b"first").expect("first");
        write_atomic(&path, b"second").expect("second");
        let read = std::fs::read(&path).expect("read");
        assert_eq!(read, b"second");
    }
}
