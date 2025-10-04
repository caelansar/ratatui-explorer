//! Local filesystem implementation.

use super::{FileEntry, FileSystem};
use std::io::{Error, ErrorKind, Result};
use std::path::Path;
use std::time::Duration;

/// A filesystem implementation for local file operations using `tokio::fs`.
///
/// This is a zero-sized type that implements the `FileSystem` trait for
/// local filesystem operations.
#[derive(Debug, Clone, Copy)]
pub struct LocalFileSystem;

impl FileSystem for LocalFileSystem {
    async fn read_dir(&self, path: &str) -> Result<Vec<FileEntry>> {
        // Add timeout for the entire operation to prevent hanging on network mounts
        let timeout_duration = Duration::from_secs(5);

        let read_result = tokio::time::timeout(timeout_duration, async {
            let mut read_dir = tokio::fs::read_dir(path).await?;
            let mut temp_entries = Vec::new();

            while let Some(entry) = read_dir.next_entry().await? {
                let name = entry.file_name().to_string_lossy().to_string();
                let path = entry.path().to_string_lossy().to_string();

                // Use a timeout for each entry's metadata read
                // This helps with slow network mounts or inaccessible files
                // Use tokio::fs::metadata() instead of entry.metadata() to follow symlinks
                // This ensures symlinks to directories (like /bin -> /usr/bin) are recognized as directories
                let metadata_result =
                    tokio::time::timeout(Duration::from_secs(2), tokio::fs::metadata(&path)).await;

                let metadata = match metadata_result {
                    Ok(Ok(meta)) => meta,
                    Ok(Err(_)) | Err(_) => {
                        // Skip entries we can't read metadata for
                        continue;
                    }
                };

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

                temp_entries.push(FileEntry {
                    name: if is_dir { format!("{}/", name) } else { name },
                    path,
                    is_dir,
                    is_hidden,
                    size: if is_dir { None } else { Some(metadata.len()) },
                    modified: metadata.modified().ok(),
                });
            }

            Ok::<_, Error>(temp_entries)
        })
        .await;

        let mut entries = match read_result {
            Ok(Ok(temp_entries)) => temp_entries,
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                return Err(Error::new(
                    ErrorKind::TimedOut,
                    format!("Timeout reading directory: {}", path),
                ));
            }
        };

        // Sort: directories first, then alphabetically
        entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        });

        Ok(entries)
    }

    async fn exists(&self, path: &str) -> Result<bool> {
        let result = tokio::time::timeout(Duration::from_secs(2), tokio::fs::metadata(path)).await;

        Ok(matches!(result, Ok(Ok(_))))
    }

    async fn is_dir(&self, path: &str) -> Result<bool> {
        let metadata = tokio::time::timeout(Duration::from_secs(2), tokio::fs::metadata(path))
            .await
            .map_err(|_| Error::new(ErrorKind::TimedOut, "Timeout checking if path is directory"))?
            .map_err(|e| e)?;

        Ok(metadata.is_dir())
    }

    async fn canonicalize(&self, path: &str) -> Result<String> {
        let canonical = tokio::time::timeout(Duration::from_secs(2), tokio::fs::canonicalize(path))
            .await
            .map_err(|_| Error::new(ErrorKind::TimedOut, "Timeout canonicalizing path"))?
            .map_err(|e| e)?;

        Ok(canonical.to_string_lossy().to_string())
    }

    fn parent(&self, path: &str) -> Option<String> {
        Path::new(path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
    }
}
