//! Filesystem abstraction for the file explorer.
//!
//! This module provides a trait-based abstraction for filesystem operations,
//! allowing the file explorer to work with both local filesystems and remote
//! filesystems (like SFTP) through a common interface.

use std::io::Result;

mod local;

pub use local::LocalFileSystem;

/// Unix-style file permissions representation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct FilePermissions {
    /// User read permission
    pub user_read: bool,
    /// User write permission
    pub user_write: bool,
    /// User execute permission
    pub user_execute: bool,
    /// Group read permission
    pub group_read: bool,
    /// Group write permission
    pub group_write: bool,
    /// Group execute permission
    pub group_execute: bool,
    /// Others read permission
    pub others_read: bool,
    /// Others write permission
    pub others_write: bool,
    /// Others execute permission
    pub others_execute: bool,
}

impl FilePermissions {
    /// Create permissions from a Unix mode value (e.g., 0o755)
    #[cfg(unix)]
    pub fn from_mode(mode: u32) -> Self {
        Self {
            user_read: mode & 0o400 != 0,
            user_write: mode & 0o200 != 0,
            user_execute: mode & 0o100 != 0,
            group_read: mode & 0o040 != 0,
            group_write: mode & 0o020 != 0,
            group_execute: mode & 0o010 != 0,
            others_read: mode & 0o004 != 0,
            others_write: mode & 0o002 != 0,
            others_execute: mode & 0o001 != 0,
        }
    }

    /// Format permissions as a Unix-style string (e.g., "rwxr-xr-x")
    pub fn to_string(&self, _is_dir: bool) -> String {
        format!(
            "{}{}{}{}{}{}{}{}{}",
            if self.user_read { "r" } else { "-" },
            if self.user_write { "w" } else { "-" },
            if self.user_execute { "x" } else { "-" },
            if self.group_read { "r" } else { "-" },
            if self.group_write { "w" } else { "-" },
            if self.group_execute { "x" } else { "-" },
            if self.others_read { "r" } else { "-" },
            if self.others_write { "w" } else { "-" },
            if self.others_execute { "x" } else { "-" },
        )
    }
}

/// Represents a file or directory entry in the filesystem.
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// The name of the file or directory (with trailing '/' for directories)
    pub name: String,
    /// The full path to the file or directory
    pub path: String,
    /// Whether this entry is a directory
    pub is_dir: bool,
    /// Whether this entry is a file
    pub is_file: bool,
    /// Whether this entry is hidden
    pub is_hidden: bool,
    /// The size of the file in bytes (None for directories)
    pub size: Option<u64>,
    /// The last modified time of the file
    pub modified: Option<std::time::SystemTime>,
    /// File permissions
    pub permissions: Option<FilePermissions>,
    /// Whether this is a symbolic link
    pub is_symlink: bool,
    /// The target path if this is a symbolic link
    pub symlink_target: Option<String>,
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
