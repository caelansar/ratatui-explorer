#![doc = include_str!("../README.md")]
//! # Features
//! - `crossterm` (default): Enables the [`From<&Event>`](enum.Input.html#method.from-2) implementation for [`Input`].
//! - `termion`: Enables the [`From<&Event>`](enum.Input.html#method.from-1) implementation for [`Input`].
//! - `termwiz`: Enables the [`From<&InputEvent>`](enum.Input.html#method.from) implementation for [`Input`].

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(rustdoc::missing_crate_level_docs)]
#![warn(rustdoc::unescaped_backticks)]
mod file_explorer;
mod icon;
mod input;
mod widget;

pub mod filesystem;

pub use file_explorer::{File, FileExplorer, FileFilter};
pub use filesystem::{FileEntry, FilePermissions, FileSystem, LocalFileSystem};
pub use icon::IconDisplay;
pub use input::Input;
pub use widget::{StatefulRenderer, Theme};
