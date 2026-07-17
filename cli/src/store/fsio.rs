//! Bounded reads and atomic writes — the low-level filesystem discipline
//! every other `store` submodule goes through.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

use super::MAX_FILE_BYTES;

/// Reads a file, returning `Ok(None)` if it does not exist and enforcing the
/// size cap before reading a byte of content.
pub(super) fn read_optional(path: &Path) -> Result<Option<String>> {
    match fs::metadata(path) {
        Ok(md) if md.len() > MAX_FILE_BYTES => {
            return Err(Error::FileTooLarge {
                path: path.to_owned(),
                size: md.len(),
                limit: MAX_FILE_BYTES,
            });
        }
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Error::io(path, e)),
    }
    match fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::io(path, e)),
    }
}

/// As [`read_optional`], but an absent file is an error.
pub(super) fn read_bounded(path: &Path) -> Result<String> {
    read_optional(path)?.ok_or_else(|| {
        Error::io(
            path,
            std::io::Error::new(std::io::ErrorKind::NotFound, "file not found"),
        )
    })
}

/// Writes `content` to a sibling temp file, then renames it into place, so
/// readers never observe a partial write.
pub(super) fn write_atomic(path: &Path, content: &str) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut tmp = tempfile_in(parent, path)?;
    tmp.0
        .write_all(content.as_bytes())
        .map_err(|e| Error::io(&tmp.1, e))?;
    tmp.0.sync_all().map_err(|e| Error::io(&tmp.1, e))?;
    drop(tmp.0);
    fs::rename(&tmp.1, path).map_err(|e| Error::io(path, e))?;
    Ok(())
}

/// Creates an exclusive temp file next to `target` (same filesystem, so the
/// final rename is atomic). Uses the target's file name to stay debuggable if
/// a crash ever leaves one behind.
fn tempfile_in(parent: &Path, target: &Path) -> Result<(fs::File, PathBuf)> {
    let base = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    for attempt in 0u32..1024 {
        let candidate = parent.join(format!(".{base}.tmp{attempt}"));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => return Ok((file, candidate)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(Error::io(&candidate, e)),
        }
    }
    Err(Error::io(
        parent,
        std::io::Error::new(std::io::ErrorKind::AlreadyExists, "no free temp file name"),
    ))
}
