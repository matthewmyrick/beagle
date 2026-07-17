//! The crate-wide error type.
//!
//! One closed enum, built with `thiserror`, so callers can match on failure
//! modes and every I/O failure carries the path it happened on.

use std::io;
use std::path::PathBuf;

/// Every way this crate can fail.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// An I/O operation on a specific file or directory failed.
    #[error("i/o error at `{path}`: {source}")]
    Io {
        /// The file or directory being touched when the error occurred.
        path: PathBuf,
        /// The underlying OS error.
        #[source]
        source: io::Error,
    },

    /// Terminal setup, drawing, or event polling failed.
    #[error("terminal error: {0}")]
    Terminal(#[from] io::Error),

    /// An RCA identifier contained characters outside `[a-z0-9-]`.
    #[error("invalid RCA id `{0}`: ids are lowercase slugs matching [a-z0-9-], 1..=64 chars")]
    InvalidId(String),

    /// An `rca.toml` manifest failed to parse.
    #[error("failed to parse manifest `{path}`: {source}")]
    ParseManifest {
        /// Path of the offending manifest.
        path: PathBuf,
        /// The TOML deserialization error.
        #[source]
        source: Box<toml::de::Error>,
    },

    /// A manifest failed to serialize (should only occur on programmer error).
    #[error("failed to serialize manifest: {0}")]
    SerializeManifest(#[from] toml::ser::Error),

    /// Scaffolding refused to overwrite an existing workspace.
    #[error("RCA `{0}` already exists")]
    AlreadyExists(String),

    /// A section or diagram file exceeded the size cap.
    #[error("`{path}` is {size} bytes, over the {limit}-byte limit; refusing to load")]
    FileTooLarge {
        /// Path of the oversized file.
        path: PathBuf,
        /// Actual size in bytes.
        size: u64,
        /// The enforced limit in bytes.
        limit: u64,
    },

    /// The filesystem watcher could not be created or attached.
    #[error("filesystem watcher error: {0}")]
    Watch(#[from] notify::Error),

    /// The beagle config file failed to parse or validate.
    #[error("invalid config `{path}`: {source}")]
    ParseConfig {
        /// Path of the offending config file.
        path: PathBuf,
        /// The TOML deserialization error.
        #[source]
        source: Box<toml::de::Error>,
    },

    /// An external tool (curl, tar, shasum, the editor) failed or was
    /// missing. Self-update and config editing shell out rather than adding
    /// heavyweight dependencies, so their failures surface here.
    #[error("{tool}: {message}")]
    Tool {
        /// The tool that failed (`curl`, `tar`, ...).
        tool: &'static str,
        /// What went wrong, including any stderr worth showing.
        message: String,
    },

    /// The GitHub releases feed could not be understood.
    #[error("could not parse the releases feed: {0}")]
    ParseReleases(String),

    /// A downloaded artifact's checksum did not match the published one.
    #[error("checksum mismatch for `{path}`: expected {expected}, got {actual}")]
    ChecksumMismatch {
        /// The downloaded file.
        path: PathBuf,
        /// The checksum published alongside the release asset.
        expected: String,
        /// The checksum computed from the download.
        actual: String,
    },

    /// No prebuilt release binary exists for this platform.
    #[error(
        "no prebuilt binary for {target}; install from source instead: \
         cargo install --git {repo}"
    )]
    UnsupportedTarget {
        /// The compile-time target triple.
        target: &'static str,
        /// The repository URL, for the `cargo install` hint.
        repo: &'static str,
    },
}

impl Error {
    /// Wraps an [`io::Error`] with the path being operated on.
    pub fn io(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}

/// Crate-wide result alias.
pub type Result<T> = std::result::Result<T, Error>;
