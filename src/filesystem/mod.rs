//! Filesystem abstraction for the file explorer.
//!
//! This module provides a trait-based abstraction for filesystem operations,
//! allowing the file explorer to work with both local filesystems and remote
//! filesystems (like SFTP) through a common interface.

use std::io::Result;

mod local;

pub use local::LocalFileSystem;

/// Represents a file or directory entry in the filesystem.
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// The name of the file or directory (with trailing '/' for directories)
    pub name: String,
    /// The full path to the file or directory
    pub path: String,
    /// Whether this entry is a directory
    pub is_dir: bool,
    /// Whether this entry is hidden
    pub is_hidden: bool,
    /// The size of the file in bytes (None for directories)
    pub size: Option<u64>,
    /// The last modified time of the file
    pub modified: Option<std::time::SystemTime>,
}

/// A trait for abstracting filesystem operations.
///
/// This trait allows the file explorer to work with different filesystem
/// implementations (local, SFTP, etc.) through a common interface.
///
/// All methods are async to support both local and remote filesystem operations.
pub trait FileSystem: Send + Sync {
    /// Read the contents of a directory at the given path.
    ///
    /// Returns a vector of `FileEntry` objects representing the files and
    /// directories in the specified path. The entries should be sorted with
    /// directories first, then alphabetically.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be read (e.g., permission denied,
    /// path does not exist, not a directory).
    async fn read_dir(&self, path: &str) -> Result<Vec<FileEntry>>;

    /// Check if a path exists.
    ///
    /// # Errors
    ///
    /// Returns an error if the filesystem cannot be accessed.
    async fn exists(&self, path: &str) -> Result<bool>;

    /// Check if a path is a directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the path does not exist or cannot be accessed.
    async fn is_dir(&self, path: &str) -> Result<bool>;

    /// Get the canonical/absolute path.
    ///
    /// # Errors
    ///
    /// Returns an error if the path cannot be canonicalized.
    async fn canonicalize(&self, path: &str) -> Result<String>;

    /// Get the parent directory of a path.
    ///
    /// Returns `None` if the path is the root directory or has no parent.
    fn parent(&self, path: &str) -> Option<String>;

    /// Delete a file at the given path.
    async fn delete(&self, path: &str) -> Result<()>;
}
