use std::{fs::FileType, io::Result, path::PathBuf, sync::Arc};

use ratatui::widgets::WidgetRef;

use crate::{
    filesystem::{FileSystem, LocalFileSystem},
    input::Input,
    widget::Renderer,
    Theme,
};

/// A file explorer that allows browsing and selecting files and directories.
///
/// The `FileExplorer` struct represents a file explorer widget that can be used to navigate
/// through the file system.
/// You can obtain a renderable widget from it with the [`widget`](#method.widget) method.
/// It provides methods for handling user input from [crossterm](https://crates.io/crates/crossterm),
/// [termion](https://crates.io/crates/termion) and [termwiz](https://crates.io/crates/termwiz) (depending on what feature is enabled).
///
/// # Examples
///
/// Creating a new `FileExplorer` widget:
///
/// ```no_run
/// use ratatui_explorer::FileExplorer;
///
/// let file_explorer = FileExplorer::new().unwrap();
/// let widget = file_explorer.widget();
/// ```
///
/// Handling user input:
///
/// ```no_run
/// # fn get_event() -> ratatui_explorer::Input {
/// #   unimplemented!()
/// # }
/// use ratatui_explorer::FileExplorer;
///
/// let mut file_explorer = FileExplorer::new().unwrap();
/// let event = get_event(); // Get the event from the terminal (with crossterm, termion or termwiz)
/// file_explorer.handle(event).unwrap();
/// ```
///
/// Accessing information about the current file selected and or the current working directory:
///
/// ```no_run
/// use ratatui_explorer::FileExplorer;
///
/// let file_explorer = FileExplorer::new().unwrap();
///
/// let current_file = file_explorer.current();
/// let current_working_directory = file_explorer.cwd();
/// println!("Current Directory: {}", current_working_directory.display());
/// println!("Name: {}", current_file.name());
/// ```
#[derive(Debug, Clone)]
pub struct FileExplorer<F: FileSystem = LocalFileSystem> {
    filesystem: Arc<F>,
    cwd: PathBuf,
    files: Vec<File>,
    show_hidden: bool,
    selected: usize,
    theme: Theme<F>,
}

impl<F: FileSystem> FileExplorer<F> {
    /// Creates a new instance of `FileExplorer` with a custom filesystem implementation.
    ///
    /// This method allows you to use the file explorer with different filesystem
    /// backends (e.g., SFTP, S3, etc.) by providing an implementation of the
    /// `FileSystem` trait.
    ///
    /// # Errors
    ///
    /// Will return `Err` if the initial directory cannot be read.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::sync::Arc;
    /// use ratatui_explorer::{FileExplorer, LocalFileSystem};
    ///
    /// # async fn example() -> std::io::Result<()> {
    /// let fs = Arc::new(LocalFileSystem);
    /// let file_explorer = FileExplorer::with_fs(fs, "/home/user".to_string()).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn with_fs(filesystem: Arc<F>, initial_path: String) -> Result<Self> {
        let cwd = PathBuf::from(initial_path);

        let mut file_explorer = Self {
            filesystem,
            cwd,
            files: vec![],
            show_hidden: false,
            selected: 0,
            theme: Theme::default(),
        };

        file_explorer.get_and_set_files().await?;

        Ok(file_explorer)
    }

    /// Build a ratatui widget to render the file explorer. The widget can then
    /// be rendered with [`Frame::render_widget`](https://docs.rs/ratatui/latest/ratatui/terminal/struct.Frame.html#method.render_widget).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ratatui::{Terminal, backend::CrosstermBackend};
    /// use ratatui_explorer::FileExplorer;
    ///
    /// let mut file_explorer = FileExplorer::new().unwrap();
    ///
    /// let mut terminal = Terminal::new(CrosstermBackend::new(std::io::stdout())).unwrap();
    ///
    /// loop {
    ///     terminal.draw(|f| {
    ///         let widget = file_explorer.widget(); // Get the widget to render the file explorer
    ///         f.render_widget(&widget, f.area());
    ///     }).unwrap();
    ///
    ///     // ...
    /// }
    /// ```
    #[inline]
    #[must_use]
    pub const fn widget(&self) -> impl WidgetRef + '_ {
        Renderer(self)
    }

    /// Handles input from user and updates the state of the file explorer.
    /// The different inputs are interpreted as follows:
    /// - `Up`: Move the selection up.
    /// - `Down`: Move the selection down.
    /// - `Left`: Move to the parent directory.
    /// - `Right`: Move to the selected directory.
    /// - `Home`: Select the first entry.
    /// - `End`: Select the last entry.
    /// - `PageUp`: Scroll the selection up.
    /// - `PageDown`: Scroll the selection down.
    /// - `ToggleShowHidden`: Toggle between showing hidden files or not.
    /// - `None`: Do nothing.
    ///
    /// [`Input`](crate::input::Input) implement [`From<Event>`](https://doc.rust-lang.org/stable/std/convert/trait.From.html)
    /// for `Event` from [crossterm](https://docs.rs/crossterm/latest/crossterm/event/enum.Event.html),
    /// [termion](https://docs.rs/termion/latest/termion/event/enum.Event.html)
    /// and [termwiz](https://docs.rs/termwiz/latest/termwiz/input/enum.InputEvent.html) (`InputEvent` in this case).
    ///
    /// # Errors
    ///
    /// Will return `Err` if the new current working directory can not be listed.
    ///
    /// # Examples
    ///
    /// Suppose you have this tree file, with `passport.png` selected inside `file_explorer`:
    /// ```plaintext
    /// /
    /// ├── .git
    /// └── Documents
    ///     ├── passport.png  <- selected
    ///     └── resume.pdf
    /// ```
    /// You can handle input like this:
    /// ```no_run
    /// use ratatui_explorer::{FileExplorer, Input};
    ///
    /// let mut file_explorer = FileExplorer::new().await.unwrap();
    ///
    /// /* user select `password.png` */
    ///
    /// file_explorer.handle(Input::Down).await.unwrap();
    /// assert_eq!(file_explorer.current().name(), "resume.pdf");
    ///
    /// file_explorer.handle(Input::Up).await.unwrap();
    /// file_explorer.handle(Input::Up).await.unwrap();
    /// assert_eq!(file_explorer.current().name(), "Documents");
    ///
    /// file_explorer.handle(Input::Left).await.unwrap();
    /// assert_eq!(file_explorer.cwd().display().to_string(), "/");
    ///
    /// file_explorer.handle(Input::Right).await.unwrap();
    /// assert_eq!(file_explorer.cwd().display().to_string(), "/Documents");
    /// ```
    pub async fn handle<I: Into<Input>>(&mut self, input: I) -> Result<()> {
        const SCROLL_COUNT: usize = 12;

        let input = input.into();

        match input {
            Input::Up => {
                self.selected = self.selected.wrapping_sub(1).min(self.files.len() - 1);
            }
            Input::Down => {
                self.selected = (self.selected + 1) % self.files.len();
            }
            Input::Home => {
                self.selected = 0;
            }
            Input::End => {
                self.selected = self.files.len() - 1;
            }
            Input::PageUp => {
                self.selected = self.selected.saturating_sub(SCROLL_COUNT);
            }
            Input::PageDown => {
                self.selected = (self.selected + SCROLL_COUNT).min(self.files.len() - 1);
            }
            Input::Left => {
                let parent = self.cwd.parent();

                if let Some(parent) = parent {
                    self.cwd = parent.to_path_buf();
                    self.get_and_set_files().await?;
                    self.selected = 0;
                }
            }
            Input::Right => {
                // Use the is_dir field from File struct instead of PathBuf::is_dir()
                // This is important for remote filesystems (SFTP) where PathBuf::is_dir()
                // would check the local filesystem and always return false
                if self.files[self.selected].is_dir {
                    self.cwd = self.files.swap_remove(self.selected).path;
                    self.get_and_set_files().await?;
                    self.selected = 0;
                }
            }
            Input::ToggleShowHidden => self.set_show_hidden(!self.show_hidden).await?,
            Input::None => (),
        }

        Ok(())
    }

    /// Sets the current working directory of the file explorer.
    ///
    /// # Errors
    ///
    /// Will return `Err` if the directory `cwd` can not be listed.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ratatui_explorer::FileExplorer;
    ///
    /// let mut file_explorer = FileExplorer::new().await.unwrap();
    ///
    /// file_explorer.set_cwd("/Documents").await.unwrap();
    /// assert_eq!(file_explorer.cwd().display().to_string(), "/Documents");
    /// ```
    #[inline]
    pub async fn set_cwd<P: Into<PathBuf>>(&mut self, cwd: P) -> Result<()> {
        self.cwd = cwd.into();
        self.get_and_set_files().await?;
        self.selected = 0;

        Ok(())
    }

    /// Sets whether hidden files should be shown in the file explorer.
    ///
    /// # Errors
    ///
    /// Will return `Err` if the directory `cwd` can not be listed.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ratatui_explorer::FileExplorer;
    ///
    /// let mut file_explorer = FileExplorer::new().await.unwrap();
    ///
    /// file_explorer.set_show_hidden(true).await.unwrap();
    /// assert_eq!(file_explorer.show_hidden(), true);
    /// ```
    #[inline]
    pub async fn set_show_hidden(&mut self, show_hidden: bool) -> Result<()> {
        self.show_hidden = show_hidden;
        self.get_and_set_files().await?;
        self.selected = 0;

        Ok(())
    }

    /// Sets the theme of the file explorer.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ratatui_explorer::{FileExplorer, Theme};
    ///
    /// let mut file_explorer = FileExplorer::new().unwrap();
    ///
    /// file_explorer.set_theme(Theme::default().add_default_title());
    /// ```
    #[inline]
    pub fn set_theme(&mut self, theme: Theme<F>) {
        self.theme = theme;
    }

    /// Sets the selected file or directory index inside the current [`Vec`](https://doc.rust-lang.org/stable/std/vec/struct.Vec.html) of files
    /// and directories if the file explorer.
    ///
    /// The file explorer add the parent directory at the beginning of the
    /// [`Vec`](https://doc.rust-lang.org/stable/std/vec/struct.Vec.html) of files, so setting the selected index to 0 will select the parent directory
    /// (if the current working directory not the root directory).
    ///
    /// # Panics
    ///
    /// Panics if `selected` is greater or equal to the number of files (plus the parent directory if it exist) in the current
    /// working directory.
    ///
    /// # Examples
    ///
    /// Suppose you have this tree file, with `passport.png` selected inside `file_explorer`:
    /// ```plaintext
    /// /
    /// ├── .git
    /// └── Documents
    ///     ├── passport.png  <- selected (index 2)
    ///     └── resume.pdf
    /// ```
    /// You can set the selected index like this:
    /// ```no_run
    /// use ratatui_explorer::FileExplorer;
    ///
    /// let mut file_explorer = FileExplorer::new().unwrap();
    ///
    /// /* user select `password.png` */
    ///
    /// // Because the file explorer add the parent directory at the beginning
    /// // of the [`Vec`](https://doc.rust-lang.org/stable/std/vec/struct.Vec.html) of files, index 0 is indeed the parent directory.
    /// file_explorer.set_selected_idx(0);
    /// assert_eq!(file_explorer.current().path().display().to_string(), "/");
    ///
    /// file_explorer.set_selected_idx(1);
    /// assert_eq!(file_explorer.current().path().display().to_string(), "/Documents");
    ///
    /// #[test]
    /// #[should_panic]
    /// fn index_out_of_bound() {
    ///    let mut file_explorer = FileExplorer::new().unwrap();
    ///    file_explorer.set_selected_idx(4);
    /// }
    /// ```
    #[inline]
    pub fn set_selected_idx(&mut self, selected: usize) {
        assert!(selected < self.files.len());
        self.selected = selected;
    }

    /// Returns the current file or directory selected.
    ///
    /// # Examples
    ///
    /// Suppose you have this tree file, with `passport.png` selected inside `file_explorer`:
    /// ```plaintext
    /// /
    /// ├── .git
    /// └── Documents
    ///     ├── passport.png  <- selected
    ///     └── resume.pdf
    /// ```
    /// You can get the current file like this:
    /// ```no_run
    /// use ratatui_explorer::FileExplorer;
    ///
    /// let file_explorer = FileExplorer::new().unwrap();
    ///
    /// /* user select `password.png` */
    ///
    /// let file = file_explorer.current();
    /// assert_eq!(file.name(), "passport.png");
    /// ```
    #[inline]
    #[must_use]
    pub fn current(&self) -> &File {
        &self.files[self.selected]
    }

    /// Select a file by name in the current directory.
    ///
    /// Returns true if the file was found and selected, false otherwise.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ratatui_explorer::FileExplorer;
    ///
    /// # async fn example() -> std::io::Result<()> {
    /// let mut file_explorer = FileExplorer::new().await?;
    ///
    /// // Select a specific file
    /// if file_explorer.select_file("myfile.txt") {
    ///     println!("Selected myfile.txt");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn select_file(&mut self, filename: &str) -> bool {
        if let Some(index) = self.files.iter().position(|f| f.name == filename) {
            self.selected = index;
            true
        } else {
            false
        }
    }

    /// Returns the current working directory of the file explorer.
    ///
    /// # Examples
    ///
    /// Suppose you have this tree file, with `passport.png` selected inside `file_explorer`:
    /// ```plaintext
    /// /
    /// ├── .git
    /// └── Documents
    ///     ├── passport.png  <- selected
    ///     └── resume.pdf
    /// ```
    /// You can get the current working directory like this:
    /// ```no_run
    /// use ratatui_explorer::FileExplorer;
    ///
    /// let file_explorer = FileExplorer::new().unwrap();
    ///
    /// /* user select `password.png` */
    ///
    /// let cwd = file_explorer.cwd();
    /// assert_eq!(cwd.display().to_string(), "/Documents");
    /// ```
    #[inline]
    #[must_use]
    pub const fn cwd(&self) -> &PathBuf {
        &self.cwd
    }

    /// Indicates whether hidden files are currently visible in the file explorer.
    /// # Examples
    ///
    ///
    /// You can get the current value like this:
    /// ```no_run
    /// use ratatui_explorer::FileExplorer;
    ///
    /// let mut file_explorer = FileExplorer::new().unwrap();
    ///
    /// // By default, hidden files are not shown.
    /// assert_eq!(file_explorer.show_hidden(), false);
    ///
    /// file_explorer.set_show_hidden(true);
    /// assert_eq!(file_explorer.show_hidden(), true);
    /// ```
    #[inline]
    #[must_use]
    pub const fn show_hidden(&self) -> bool {
        self.show_hidden
    }

    /// Returns the a [`Vec`](https://doc.rust-lang.org/stable/std/vec/struct.Vec.html) of files and directories in the current working directory
    /// of the file explorer, plus the parent directory if it exist.
    ///
    /// # Examples
    ///
    /// Suppose you have this tree file, with `passport.png` selected inside `file_explorer`:
    /// ```plaintext
    /// /
    /// ├── .git
    /// └── Documents
    ///     ├── passport.png  <- selected
    ///     └── resume.pdf
    /// ```
    /// You can get the [`Vec`](https://doc.rust-lang.org/stable/std/vec/struct.Vec.html) of files and directories like this:
    /// ```no_run
    /// use ratatui_explorer::FileExplorer;
    ///
    /// let file_explorer = FileExplorer::new().unwrap();
    ///
    /// /* user select `password.png` */
    ///
    /// let files = file_explorer.files();
    /// assert_eq!(files.len(), 4); // 3 files/directory and the parent directory
    /// ```
    #[inline]
    #[must_use]
    pub const fn files(&self) -> &Vec<File> {
        &self.files
    }

    /// Returns the index of the selected file or directory in the current [`Vec`](https://doc.rust-lang.org/stable/std/vec/struct.Vec.html) of files
    /// and directories in the current working directory of the file explorer.
    ///
    /// # Examples
    ///
    /// Suppose you have this tree file, with `passport.png` selected inside `file_explorer`:
    /// ```plaintext
    /// /
    /// ├── .git
    /// └── Documents
    ///     ├── passport.png  <- selected (index 2)
    ///     └── resume.pdf
    /// ```
    /// You can get the selected index like this:
    /// ```no_run
    /// use ratatui_explorer::FileExplorer;
    ///
    /// let file_explorer = FileExplorer::new().unwrap();
    ///
    /// /* user select `password.png` */
    ///
    /// let selected_idx = file_explorer.selected_idx();
    ///
    /// // Because the file explorer add the parent directory at the beginning
    /// // of the [`Vec`](https://doc.rust-lang.org/stable/std/vec/struct.Vec.html) of files, the selected index will be 2.
    /// assert_eq!(selected_idx, 2);
    /// ```
    #[inline]
    #[must_use]
    pub const fn selected_idx(&self) -> usize {
        self.selected
    }

    /// Returns the theme of the file explorer.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ratatui_explorer::{FileExplorer, Theme};
    ///
    /// let file_explorer = FileExplorer::new().unwrap();
    ///
    /// assert_eq!(file_explorer.theme(), &Theme::default());
    /// ```
    #[inline]
    #[must_use]
    pub const fn theme(&self) -> &Theme<F> {
        &self.theme
    }

    /// Get the files and directories in the current working directory and set them in the file explorer.
    /// It add the parent directory at the beginning of the [`Vec`](https://doc.rust-lang.org/stable/std/vec/struct.Vec.html) of files if it exist.
    async fn get_and_set_files(&mut self) -> Result<()> {
        // Use the FileSystem trait to read the directory
        let entries = self
            .filesystem
            .read_dir(&self.cwd.to_string_lossy())
            .await?;

        // Convert FileEntry to File
        let mut files: Vec<File> = entries
            .into_iter()
            .filter(|entry| self.show_hidden || !entry.is_hidden)
            .map(|entry| File {
                name: entry.name,
                path: PathBuf::from(entry.path),
                is_dir: entry.is_dir,
                is_hidden: entry.is_hidden,
                file_type: None, // FileEntry doesn't include FileType
            })
            .collect();

        // Add parent directory if it exists
        if let Some(parent) = self.cwd.parent() {
            files.insert(
                0,
                File {
                    name: "../".to_owned(),
                    path: parent.to_path_buf(),
                    is_dir: true,
                    is_hidden: false,
                    file_type: None,
                },
            );
        }

        self.files = files;
        Ok(())
    }
}

// Separate impl block for FileExplorer<LocalFileSystem> for backward compatibility
impl FileExplorer<LocalFileSystem> {
    /// Creates a new instance of `FileExplorer` with the default LocalFileSystem.
    ///
    /// This method initializes a `FileExplorer` with the current working directory.
    /// By default, hidden files are not shown.
    ///
    /// # Errors
    ///
    /// Will return `Err` if the current working directory can not be listed.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ratatui_explorer::FileExplorer;
    ///
    /// # async fn example() -> std::io::Result<()> {
    /// let file_explorer = FileExplorer::new().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new() -> Result<Self> {
        let cwd = std::env::current_dir()?;
        let filesystem = Arc::new(LocalFileSystem);

        Self::with_fs(filesystem, cwd.to_string_lossy().to_string()).await
    }

    /// Creates a new instance of `FileExplorer` with a specific theme.
    ///
    /// # Errors
    ///
    /// Will return `Err` if the current working directory can not be listed.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ratatui_explorer::{FileExplorer, Theme};
    ///
    /// # async fn example() -> std::io::Result<()> {
    /// let file_explorer = FileExplorer::with_theme(Theme::default().add_default_title()).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub async fn with_theme(theme: Theme) -> Result<Self> {
        let mut file_explorer = Self::new().await?;
        file_explorer.theme = theme;
        Ok(file_explorer)
    }
}

/// A file or directory in the file explorer.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct File {
    name: String,
    path: PathBuf,
    is_dir: bool,
    is_hidden: bool,
    file_type: Option<FileType>,
}

impl File {
    /// Returns the name of the file or directory.
    ///
    /// # Examples
    /// Suppose you have this tree file, with `passport.png` selected inside `file_explorer`:
    /// ```plaintext
    /// /
    /// ├── .git
    /// └── Documents
    ///     ├── passport.png  <- selected
    ///     └── resume.pdf
    /// ```
    /// You can get the name of the selected file like this:
    /// ```no_run
    /// use ratatui_explorer::FileExplorer;
    ///
    /// let file_explorer = FileExplorer::new().unwrap();
    ///
    /// /* user select `password.png` */
    ///
    /// let file = file_explorer.current();
    /// assert_eq!(file.name(), "passport.png");
    /// ```
    #[inline]
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the path of the file or directory.
    ///
    /// # Examples
    /// Suppose you have this tree file, with `passport.png` selected inside `file_explorer`:
    /// ```plaintext
    /// /
    /// ├── .git
    /// └── Documents
    ///     ├── passport.png  <- selected
    ///     └── resume.pdf
    /// ```
    /// You can get the path of the selected file like this:
    /// ```no_run
    /// use ratatui_explorer::FileExplorer;
    ///
    /// let file_explorer = FileExplorer::new().unwrap();
    ///
    /// /* user select `password.png` */
    ///
    /// let file = file_explorer.current();
    /// assert_eq!(file.path().display().to_string(), "/Documents/passport.png");
    /// ```
    #[inline]
    #[must_use]
    pub const fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Returns `true` is the file is a directory.
    ///
    /// # Examples
    /// Suppose you have this tree file, with `passport.png` selected inside `file_explorer`:
    /// ```plaintext
    /// /
    /// ├── .git
    /// └── Documents
    ///     ├── passport.png  <- selected
    ///     └── resume.pdf
    /// ```
    /// You can know if the selected file is a directory like this:
    /// ```no_run
    /// use ratatui_explorer::FileExplorer;
    ///
    /// let file_explorer = FileExplorer::new().unwrap();
    ///
    /// /* user select `password.png` */
    ///
    /// let file = file_explorer.current();
    /// assert_eq!(file.is_dir(), false);
    ///
    /// /* user select `Documents` */
    ///
    /// let file = file_explorer.current();
    /// assert_eq!(file.is_dir(), true);
    /// ```
    #[inline]
    #[must_use]
    pub const fn is_dir(&self) -> bool {
        self.is_dir
    }

    /// Returns `true` is the file is a regular file.
    ///
    /// # Examples
    /// Suppose you have this tree file, with `passport.png` selected inside `file_explorer`:
    /// ```plaintext
    /// /
    /// ├── .git
    /// └── Documents
    ///     ├── passport.png  <- selected
    ///     └── resume.pdf
    /// ```
    /// You can know if the selected file is a directory like this:
    /// ```no_run
    /// use ratatui_explorer::FileExplorer;
    ///
    /// let file_explorer = FileExplorer::new().unwrap();
    ///
    /// /* user select `password.png` */
    ///
    /// let file = file_explorer.current();
    /// assert_eq!(file.is_file(), true);
    ///
    /// /* user select `Documents` */
    ///
    /// let file = file_explorer.current();
    /// assert_eq!(file.is_file(), false);
    /// ```
    #[inline]
    #[must_use]
    pub fn is_file(&self) -> bool {
        self.file_type.is_some_and(|f| f.is_file())
    }

    /// Returns `true` if the file or directory is hidden.
    ///
    /// # Examples
    /// Suppose you have this tree file, with `passport.png` selected inside `file_explorer`:
    /// ```plaintext
    /// /
    /// ├── .git
    /// └── Documents
    ///     ├── passport.png  <- selected
    ///     └── resume.pdf
    /// ```
    /// You can know if the selected file or directory is hidden like this:
    /// ```no_run
    /// use ratatui_explorer::FileExplorer;
    ///
    /// let file_explorer = FileExplorer::new().unwrap();
    ///
    /// /* user select `password.png` */
    ///
    /// let file = file_explorer.current();
    /// assert_eq!(file.is_hidden(), false);
    ///
    /// /* user select `.git` */
    ///
    /// let file = file_explorer.current();
    /// assert_eq!(file.is_hidden(), true);
    /// ```
    #[inline]
    #[must_use]
    pub fn is_hidden(&self) -> bool {
        self.is_hidden
    }

    /// Returns the `FileType` of the file, when available.
    ///
    /// # Examples
    /// Suppose you have this tree file, with `passport.png` selected inside `file_explorer`:
    /// ```plaintext
    /// /
    /// ├── .git
    /// └── Documents
    ///     ├── passport.png  <- selected
    ///     └── resume.pdf
    /// ```
    /// You can know if the selected file is a directory like this:
    /// ```no_run
    /// use std::os::unix::fs::FileTypeExt;
    ///
    /// use ratatui_explorer::FileExplorer;
    ///
    /// let file_explorer = FileExplorer::new().unwrap();
    ///
    /// /* user select `password.png` */
    ///
    /// let file = file_explorer.current();
    /// assert_eq!(file.file_type().unwrap().is_file(), true);
    /// assert_eq!(file.file_type().unwrap().is_socket(), false);
    ///
    /// /* user select `Documents` */
    ///
    /// let file = file_explorer.current();
    /// assert_eq!(file.file_type().unwrap().is_file(), false);
    /// assert_eq!(file.file_type().unwrap().is_socket(), false);
    /// ```
    #[inline]
    #[must_use]
    pub const fn file_type(&self) -> Option<FileType> {
        self.file_type
    }
}
