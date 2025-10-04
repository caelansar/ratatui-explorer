//! Local filesystem implementation.

use super::{FileEntry, FileSystem};
use std::io::Result;
use std::path::Path;

/// A filesystem implementation for local file operations using `tokio::fs`.
///
/// This is a zero-sized type that implements the `FileSystem` trait for
/// local filesystem operations.
#[derive(Debug, Clone, Copy)]
pub struct LocalFileSystem;

impl FileSystem for LocalFileSystem {
    async fn read_dir(&self, path: &str) -> Result<Vec<FileEntry>> {
        let mut entries = Vec::new();
        let mut read_dir = tokio::fs::read_dir(path).await?;

        while let Some(entry) = read_dir.next_entry().await? {
            let metadata = entry.metadata().await?;
            let name = entry.file_name().to_string_lossy().to_string();
            let path = entry.path().to_string_lossy().to_string();
            let is_dir = metadata.is_dir();

            // Determine if file is hidden
            let is_hidden = {
                #[cfg(unix)]
                {
                    name.starts_with('.')
                }

                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
                    metadata.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0
                }

                #[cfg(not(any(unix, windows)))]
                {
                    name.starts_with('.')
                }
            };

            entries.push(FileEntry {
                name: if is_dir { format!("{}/", name) } else { name },
                path,
                is_dir,
                is_hidden,
                size: if is_dir { None } else { Some(metadata.len()) },
                modified: metadata.modified().ok(),
            });
        }

        // Sort: directories first, then alphabetically
        entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        });

        Ok(entries)
    }

    async fn exists(&self, path: &str) -> Result<bool> {
        Ok(tokio::fs::metadata(path).await.is_ok())
    }

    async fn is_dir(&self, path: &str) -> Result<bool> {
        let metadata = tokio::fs::metadata(path).await?;
        Ok(metadata.is_dir())
    }

    async fn canonicalize(&self, path: &str) -> Result<String> {
        let canonical = tokio::fs::canonicalize(path).await?;
        Ok(canonical.to_string_lossy().to_string())
    }

    fn parent(&self, path: &str) -> Option<String> {
        Path::new(path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
    }
}
