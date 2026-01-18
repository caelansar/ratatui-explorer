//! Local filesystem implementation.

use super::{FileEntry, FilePermissions, FileSystem};
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
                let entry_path = entry.path().to_string_lossy().to_string();

                // Use DirEntry.file_type() to check for symlinks
                // On Linux, this may be free (already from readdir syscall)
                // On other platforms, it's still more efficient than full metadata
                let file_type_result =
                    tokio::time::timeout(Duration::from_secs(2), entry.file_type()).await;

                let file_type = match file_type_result {
                    Ok(Ok(ft)) => ft,
                    Ok(Err(_)) | Err(_) => {
                        // Skip entries we can't read file type for
                        println!("Can't read file type for: {}", entry_path);
                        continue;
                    }
                };

                let is_symlink = file_type.is_symlink();

                // Read symlink target if this is a symlink
                let symlink_target = if is_symlink {
                    let target_result = tokio::time::timeout(
                        Duration::from_secs(2),
                        tokio::fs::read_link(&entry_path),
                    )
                    .await;

                    match target_result {
                        Ok(Ok(target_path)) => Some(target_path.to_string_lossy().to_string()),
                        Ok(Err(_)) | Err(_) => None,
                    }
                } else {
                    None
                };

                // For symlinks, we MUST use tokio::fs::metadata() to follow the link
                // For non-symlinks, use entry.metadata() which may be more efficient
                let metadata = if is_symlink {
                    // Follow symlink to get target metadata
                    // This is critical for symlink-to-directory navigation (e.g., /bin -> /usr/bin)
                    let meta_result = tokio::time::timeout(
                        Duration::from_secs(2),
                        tokio::fs::metadata(&entry_path),
                    )
                    .await;

                    match meta_result {
                        Ok(Ok(meta)) => meta,
                        Ok(Err(e)) => {
                            // Skip broken or inaccessible symlinks
                            println!("Broken or inaccessible symlink: {}: {}", entry_path, e);
                            continue;
                        }
                        Err(_) => {
                            println!("Error getting metadata for symlink: {}", entry_path);
                            continue;
                        }
                    }
                } else {
                    // For non-symlinks, use DirEntry.metadata() which may reuse cached data
                    // This is more efficient as it might avoid an additional syscall
                    let meta_result =
                        tokio::time::timeout(Duration::from_secs(2), entry.metadata()).await;

                    match meta_result {
                        Ok(Ok(meta)) => meta,
                        Ok(Err(_)) | Err(_) => {
                            // Skip entries we can't read metadata for
                            continue;
                        }
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

                // Get permissions
                let permissions = {
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        let mode = metadata.permissions().mode();
                        Some(FilePermissions::from_mode(mode))
                    }

                    #[cfg(not(unix))]
                    {
                        None
                    }
                };

                temp_entries.push(FileEntry {
                    name: if is_dir { format!("{}/", name) } else { name },
                    path: entry_path,
                    is_dir,
                    is_hidden,
                    size: if is_dir { None } else { Some(metadata.len()) },
                    modified: metadata.modified().ok(),
                    permissions,
                    is_symlink,
                    symlink_target,
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

    async fn delete(&self, path: &str) -> Result<()> {
        tokio::time::timeout(Duration::from_secs(5), tokio::fs::remove_file(path))
            .await
            .map_err(|_| Error::new(ErrorKind::TimedOut, "Timeout deleting file"))?
            .map_err(|e| e)?;

        Ok(())
    }
}
