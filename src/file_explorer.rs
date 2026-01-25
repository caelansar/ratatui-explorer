use std::{collections::HashSet, io::Result, path::PathBuf, sync::Arc};

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
/// # tokio_test::block_on(async {
/// let file_explorer = FileExplorer::new().await.unwrap();
/// let widget = file_explorer.widget();
/// # })
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
/// # tokio_test::block_on(async {
/// let mut file_explorer = FileExplorer::new().await.unwrap();
/// let event = get_event(); // Get the event from the terminal (with crossterm, termion or termwiz)
/// file_explorer.handle(event).await.unwrap();
/// # })
/// ```
///
/// Accessing information about the current file selected and or the current working directory:
///
/// ```no_run
/// use ratatui_explorer::FileExplorer;
///
/// # tokio_test::block_on(async {
/// let file_explorer = FileExplorer::new().await.unwrap();
///
/// let current_file = file_explorer.current();
/// let current_working_directory = file_explorer.cwd();
/// println!("Current Directory: {}", current_working_directory.display());
/// println!("Name: {}", current_file.name());
/// # })
/// ```
#[derive(Debug, Clone)]
pub struct FileExplorer<F: FileSystem = LocalFileSystem> {
    filesystem: Arc<F>,
    cwd: PathBuf,
    files: Vec<File>,
    filtered_files: Vec<(usize, File)>,
    show_hidden: bool,
    selected: usize,
    theme: Theme<F>,
    search_filter: Option<String>,
    scroll_offset: usize,
    selected_paths: HashSet<PathBuf>,
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
            filtered_files: vec![],
            show_hidden: false,
            selected: 0,
            theme: Theme::default(),
            search_filter: None,
            scroll_offset: 0,
            selected_paths: HashSet::new(),
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
    /// # tokio_test::block_on(async {
    /// let mut file_explorer = FileExplorer::new().await.unwrap();
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
    /// #   break;
    /// }
    /// # })
    /// ```
    #[inline]
    #[must_use]
    pub const fn widget(&self) -> impl WidgetRef + '_ {
        Renderer(self)
    }

    /// Build a stateful widget that properly tracks scroll position.
    /// This is useful when you need to maintain scroll state across renders,
    /// particularly for scrollbar integration.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ratatui::{Terminal, backend::CrosstermBackend};
    /// use ratatui_explorer::FileExplorer;
    ///
    /// # tokio_test::block_on(async {
    /// let mut file_explorer = FileExplorer::new().await.unwrap();
    /// let mut terminal = Terminal::new(CrosstermBackend::new(std::io::stdout())).unwrap();
    ///
    /// loop {
    ///     terminal.draw(|f| {
    ///         let area = f.area();
    ///         file_explorer.widget_stateful().render(area, f.buffer_mut());
    ///     }).unwrap();
    ///
    ///     // ...
    /// #   break;
    /// }
    /// # })
    /// ```
    #[inline]
    #[must_use]
    pub fn widget_stateful(&mut self) -> crate::widget::StatefulRenderer<'_, F> {
        crate::widget::StatefulRenderer(self)
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
    /// # tokio_test::block_on(async {
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
    /// # })
    /// ```
    pub async fn handle<I: Into<Input>>(&mut self, input: I) -> Result<()> {
        const SCROLL_COUNT: usize = 12;

        let input = input.into();

        match input {
            Input::Up => {
                if !self.filtered_files.is_empty() {
                    let current_filtered_idx = self.filtered_selected_idx().unwrap_or(0);
                    let new_filtered_idx = current_filtered_idx
                        .wrapping_sub(1)
                        .min(self.filtered_files.len() - 1);
                    if let Some((original_idx, _)) = self.filtered_files.get(new_filtered_idx) {
                        self.selected = *original_idx;
                    }
                }
            }
            Input::Down => {
                if !self.filtered_files.is_empty() {
                    let current_filtered_idx = self.filtered_selected_idx().unwrap_or(0);
                    let new_filtered_idx = (current_filtered_idx + 1) % self.filtered_files.len();
                    if let Some((original_idx, _)) = self.filtered_files.get(new_filtered_idx) {
                        self.selected = *original_idx;
                    }
                }
            }
            Input::Home => {
                if let Some((original_idx, _)) = self.filtered_files.first() {
                    self.selected = *original_idx;
                }
            }
            Input::End => {
                if let Some((original_idx, _)) = self.filtered_files.last() {
                    self.selected = *original_idx;
                }
            }
            Input::PageUp => {
                if !self.filtered_files.is_empty() {
                    let current_filtered_idx = self.filtered_selected_idx().unwrap_or(0);
                    let new_filtered_idx = current_filtered_idx.saturating_sub(SCROLL_COUNT);
                    if let Some((original_idx, _)) = self.filtered_files.get(new_filtered_idx) {
                        self.selected = *original_idx;
                    }
                }
            }
            Input::PageDown => {
                if !self.filtered_files.is_empty() {
                    let current_filtered_idx = self.filtered_selected_idx().unwrap_or(0);
                    let new_filtered_idx =
                        (current_filtered_idx + SCROLL_COUNT).min(self.filtered_files.len() - 1);
                    if let Some((original_idx, _)) = self.filtered_files.get(new_filtered_idx) {
                        self.selected = *original_idx;
                    }
                }
            }
            Input::Left => {
                let parent = self.cwd.parent();

                if let Some(parent) = parent {
                    // Remember the current directory name to select it after navigating back
                    let current_dir_name = self
                        .cwd
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| format!("{}/", s));

                    self.cwd = parent.to_path_buf();
                    self.get_and_set_files().await?;

                    // Try to select the directory we just came from
                    if let Some(dir_name) = current_dir_name {
                        if !self.select_file(&dir_name) {
                            // If we can't find it (shouldn't happen), default to first item
                            self.selected = 0;
                        }
                    } else {
                        self.selected = 0;
                    }
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
            Input::Delete => {
                // Get the currently selected file
                let current_file = &self.files[self.selected];

                // Skip deletion for directories and parent directory (..)
                if !current_file.is_dir {
                    let file_path = current_file.path.to_string_lossy().to_string();

                    // Attempt to delete the file
                    self.filesystem.delete(&file_path).await?;

                    // Refresh the file list
                    self.get_and_set_files().await?;

                    // Adjust selection: stay on the same index if possible, or move to the last item
                    if self.selected >= self.files.len() && !self.files.is_empty() {
                        self.selected = self.files.len() - 1;
                    }
                }
            }
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
    /// # tokio_test::block_on(async {
    /// let mut file_explorer = FileExplorer::new().await.unwrap();
    ///
    /// file_explorer.set_cwd("/Documents").await.unwrap();
    /// assert_eq!(file_explorer.cwd().display().to_string(), "/Documents");
    /// # })
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
    /// # tokio_test::block_on(async {
    /// let mut file_explorer = FileExplorer::new().await.unwrap();
    ///
    /// file_explorer.set_show_hidden(true).await.unwrap();
    /// assert_eq!(file_explorer.show_hidden(), true);
    /// # })
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
    /// # tokio_test::block_on(async {
    /// let mut file_explorer = FileExplorer::new().await.unwrap();
    ///
    /// file_explorer.set_theme(Theme::default().add_default_title());
    /// # })
    /// ```
    #[inline]
    pub fn set_theme(&mut self, theme: Theme<F>) {
        self.theme = theme;
    }

    /// Sets the search filter to filter files and directories by name.
    ///
    /// When a search filter is set, only files and directories whose names contain
    /// the filter string (case-insensitive) will be displayed. Set to `None` to clear
    /// the filter and show all files.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ratatui_explorer::FileExplorer;
    ///
    /// # tokio_test::block_on(async {
    /// let mut file_explorer = FileExplorer::new().await.unwrap();
    ///
    /// // Filter to show only files containing "test"
    /// file_explorer.set_search_filter(Some("test".to_string()));
    ///
    /// // Clear the filter
    /// file_explorer.set_search_filter(None);
    /// # })
    /// ```
    #[inline]
    pub fn set_search_filter(&mut self, filter: Option<String>) {
        self.search_filter = filter;
        self.filtered_files = self.compute_filtered_files();
    }

    /// Returns the current search filter, if any.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ratatui_explorer::FileExplorer;
    ///
    /// # tokio_test::block_on(async {
    /// let mut file_explorer = FileExplorer::new().await.unwrap();
    /// assert_eq!(file_explorer.search_filter(), None);
    ///
    /// file_explorer.set_search_filter(Some("test".to_string()));
    /// assert_eq!(file_explorer.search_filter(), Some("test"));
    /// # })
    /// ```
    #[inline]
    #[must_use]
    pub fn search_filter(&self) -> Option<&str> {
        self.search_filter.as_deref()
    }

    /// Returns the cached filtered files with their original indices.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ratatui_explorer::FileExplorer;
    ///
    /// # async fn example() -> std::io::Result<()> {
    /// let mut file_explorer = FileExplorer::new().await?;
    /// file_explorer.set_search_filter(Some("test".to_string()));
    /// let filtered_files = file_explorer.filtered_files();
    /// println!("Found {} filtered files", filtered_files.len());
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    #[must_use]
    pub fn filtered_files(&self) -> &[(usize, File)] {
        &self.filtered_files
    }

    /// Sets the selected file or directory index in the filtered view.
    /// When no search filter is active, this works the same as before.
    /// When a search filter is active, this accepts an index within the filtered list
    /// and converts it to the corresponding original index.
    ///
    /// The file explorer add the parent directory at the beginning of the
    /// [`Vec`](https://doc.rust-lang.org/stable/std/vec/struct.Vec.html) of files, so setting the selected index to 0 will select the parent directory
    /// (if the current working directory not the root directory).
    ///
    /// # Panics
    ///
    /// Panics if `selected` is greater or equal to the number of files in the current view
    /// (filtered files if a filter is active, or all files if no filter).
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
    /// # tokio_test::block_on(async {
    /// let mut file_explorer = FileExplorer::new().await.unwrap();
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
    /// # })
    /// ```
    ///
    /// Note: Attempting to set an index >= the number of files will cause a panic.
    #[inline]
    pub fn set_selected_idx(&mut self, selected: usize) {
        assert!(selected < self.filtered_files.len());

        if let Some((original_idx, _)) = self.filtered_files.get(selected) {
            self.selected = *original_idx;
        }
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
    /// # tokio_test::block_on(async {
    /// let file_explorer = FileExplorer::new().await.unwrap();
    ///
    /// /* user select `password.png` */
    ///
    /// let file = file_explorer.current();
    /// assert_eq!(file.name(), "passport.png");
    /// # })
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
    /// # tokio_test::block_on(async {
    /// let file_explorer = FileExplorer::new().await.unwrap();
    ///
    /// /* user select `password.png` */
    ///
    /// let cwd = file_explorer.cwd();
    /// assert_eq!(cwd.display().to_string(), "/Documents");
    /// # })
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
    /// # tokio_test::block_on(async {
    /// let mut file_explorer = FileExplorer::new().await.unwrap();
    ///
    /// // By default, hidden files are not shown.
    /// assert_eq!(file_explorer.show_hidden(), false);
    ///
    /// file_explorer.set_show_hidden(true).await.unwrap();
    /// assert_eq!(file_explorer.show_hidden(), true);
    /// # })
    /// ```
    #[inline]
    #[must_use]
    pub const fn show_hidden(&self) -> bool {
        self.show_hidden
    }

    /// Returns the a [`Vec`](https://doc.rust-lang.org/stable/std/vec/struct.Vec.html) of files and directories in the current working directory
    /// of the file explorer, plus the parent directory if it exist.
    /// When a search filter is active, returns only the filtered files.
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
    /// # tokio_test::block_on(async {
    /// let file_explorer = FileExplorer::new().await.unwrap();
    ///
    /// /* user select `password.png` */
    ///
    /// let files = file_explorer.files();
    /// assert_eq!(files.len(), 4); // 3 files/directory and the parent directory
    /// # })
    /// ```
    #[inline]
    #[must_use]
    pub fn files(&self) -> Vec<&File> {
        self.filtered_files.iter().map(|(_, file)| file).collect()
    }

    /// Returns all files and directories in the current working directory
    /// without any filtering applied. This is useful when you need access
    /// to the complete file list regardless of search filters.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ratatui_explorer::FileExplorer;
    ///
    /// # tokio_test::block_on(async {
    /// let mut file_explorer = FileExplorer::new().await.unwrap();
    /// file_explorer.set_search_filter(Some("test".to_string()));
    ///
    /// let all_files = file_explorer.all_files();
    /// let filtered_files = file_explorer.files();
    /// // all_files.len() >= filtered_files.len()
    /// # })
    /// ```
    #[inline]
    #[must_use]
    pub const fn all_files(&self) -> &Vec<File> {
        &self.files
    }

    /// Returns the index of the selected file or directory in the filtered view.
    /// When no search filter is active, this returns the same as the original index.
    /// When a search filter is active, this returns the index within the filtered list.
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
    /// You can get the selected index like this:
    /// ```no_run
    /// use ratatui_explorer::FileExplorer;
    ///
    /// # tokio_test::block_on(async {
    /// let file_explorer = FileExplorer::new().await.unwrap();
    ///
    /// /* user select `password.png` */
    ///
    /// let selected_idx = file_explorer.selected_idx();
    ///
    /// // Because the file explorer add the parent directory at the beginning
    /// // of the [`Vec`](https://doc.rust-lang.org/stable/std/vec/struct.Vec.html) of files, the selected index will be 2.
    /// assert_eq!(selected_idx, 2);
    /// # })
    /// ```
    #[inline]
    #[must_use]
    pub fn selected_idx(&self) -> usize {
        self.filtered_selected_idx().unwrap_or(0)
    }

    /// Returns the original index of the selected file in the complete file list.
    /// This is useful when you need to know the actual position in the unfiltered list.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ratatui_explorer::FileExplorer;
    ///
    /// # tokio_test::block_on(async {
    /// let mut file_explorer = FileExplorer::new().await.unwrap();
    /// file_explorer.set_search_filter(Some("test".to_string()));
    ///
    /// let filtered_idx = file_explorer.selected_idx();
    /// let original_idx = file_explorer.original_selected_idx();
    /// // original_idx >= filtered_idx
    /// # })
    /// ```
    #[inline]
    #[must_use]
    pub const fn original_selected_idx(&self) -> usize {
        self.selected
    }

    /// Returns the theme of the file explorer.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ratatui_explorer::{FileExplorer, Theme};
    ///
    /// # tokio_test::block_on(async {
    /// let file_explorer = FileExplorer::new().await.unwrap();
    ///
    /// assert_eq!(file_explorer.theme(), &Theme::default());
    /// # })
    /// ```
    #[inline]
    #[must_use]
    pub const fn theme(&self) -> &Theme<F> {
        &self.theme
    }

    /// Returns the current scroll offset of the file explorer.
    ///
    /// This represents the index of the first visible item in the list.
    #[inline]
    #[must_use]
    pub const fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Sets the scroll offset for the file explorer.
    ///
    /// This is used internally by the widget renderer to maintain scroll state.
    #[inline]
    pub(crate) fn set_scroll_offset(&mut self, offset: usize) {
        self.scroll_offset = offset;
    }

    /// Sets the paths of files that should be displayed as selected.
    ///
    /// This allows external state management of file selection, useful for
    /// multi-file operations like batch copying or deleting.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::collections::HashSet;
    /// use std::path::PathBuf;
    /// use ratatui_explorer::FileExplorer;
    ///
    /// # async fn example() -> std::io::Result<()> {
    /// let mut file_explorer = FileExplorer::new().await?;
    ///
    /// let mut selected = HashSet::new();
    /// selected.insert(PathBuf::from("/home/user/file1.txt"));
    /// selected.insert(PathBuf::from("/home/user/file2.txt"));
    ///
    /// file_explorer.set_selected_paths(selected);
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn set_selected_paths(&mut self, paths: HashSet<PathBuf>) {
        self.selected_paths = paths;
    }

    /// Returns a reference to the set of selected file paths.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ratatui_explorer::FileExplorer;
    ///
    /// # async fn example() -> std::io::Result<()> {
    /// let file_explorer = FileExplorer::new().await?;
    /// let selected = file_explorer.selected_paths();
    /// println!("Number of selected files: {}", selected.len());
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    #[must_use]
    pub const fn selected_paths(&self) -> &HashSet<PathBuf> {
        &self.selected_paths
    }

    /// Checks if a specific file is marked as selected.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ratatui_explorer::FileExplorer;
    ///
    /// # async fn example() -> std::io::Result<()> {
    /// let file_explorer = FileExplorer::new().await?;
    /// let current_file = file_explorer.current();
    ///
    /// if file_explorer.is_file_selected(current_file) {
    ///     println!("Current file is selected");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    #[must_use]
    pub fn is_file_selected(&self, file: &File) -> bool {
        self.selected_paths.contains(&file.path)
    }

    /// Compute filtered files with their original indices, returning owned File objects.
    /// This method clones the files to cache them in the filtered_files field.
    fn compute_filtered_files(&self) -> Vec<(usize, File)> {
        if let Some(filter) = self.search_filter() {
            let filter_lower = filter.to_lowercase();
            self.files
                .iter()
                .enumerate()
                .filter(|(_, file)| file.name().to_lowercase().contains(&filter_lower))
                .map(|(idx, file)| (idx, file.clone()))
                .collect()
        } else {
            self.files.iter().cloned().enumerate().collect()
        }
    }

    /// Convert the current selected index (which is stored as original index) to filtered index.
    fn filtered_selected_idx(&self) -> Option<usize> {
        self.filtered_files
            .iter()
            .position(|(original_idx, _)| *original_idx == self.selected)
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
                is_file: entry.is_file,
                is_hidden: entry.is_hidden,
                size: entry.size,
                modified: entry.modified,
                permissions: entry.permissions,
                symlink_target: entry.symlink_target,
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
                    is_file: false,
                    is_hidden: false,
                    size: None,
                    modified: None,
                    permissions: None,
                    symlink_target: None,
                },
            );
        }

        self.files = files;
        self.filtered_files = self.compute_filtered_files();
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
    is_file: bool,
    is_dir: bool,
    is_hidden: bool,
    size: Option<u64>,
    modified: Option<std::time::SystemTime>,
    permissions: Option<crate::filesystem::FilePermissions>,
    symlink_target: Option<String>,
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
    /// # tokio_test::block_on(async {
    /// let file_explorer = FileExplorer::new().await.unwrap();
    ///
    /// /* user select `password.png` */
    ///
    /// let file = file_explorer.current();
    /// assert_eq!(file.name(), "passport.png");
    /// # })
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
    /// # tokio_test::block_on(async {
    /// let file_explorer = FileExplorer::new().await.unwrap();
    ///
    /// /* user select `password.png` */
    ///
    /// let file = file_explorer.current();
    /// assert_eq!(file.path().display().to_string(), "/Documents/passport.png");
    /// # })
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
    /// # tokio_test::block_on(async {
    /// let file_explorer = FileExplorer::new().await.unwrap();
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
    /// # })
    /// ```
    #[inline]
    #[must_use]
    pub const fn is_dir(&self) -> bool {
        self.is_dir
    }

    /// Returns the target of the symbolic link if the file is a symbolic link.
    #[inline]
    #[must_use]
    pub fn symlink_target(&self) -> Option<&str> {
        self.symlink_target.as_deref()
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
    /// You can know if the selected file is a regular file like this:
    /// ```no_run
    /// use ratatui_explorer::FileExplorer;
    ///
    /// # tokio_test::block_on(async {
    /// let file_explorer = FileExplorer::new().await.unwrap();
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
    /// # })
    /// ```
    #[inline]
    #[must_use]
    pub fn is_file(&self) -> bool {
        self.is_file
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
    /// # tokio_test::block_on(async {
    /// let file_explorer = FileExplorer::new().await.unwrap();
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
    /// # })
    /// ```
    #[inline]
    #[must_use]
    pub fn is_hidden(&self) -> bool {
        self.is_hidden
    }

    /// Returns the size of the file in bytes.
    ///
    /// Returns `None` for directories.
    #[inline]
    #[must_use]
    pub const fn size(&self) -> Option<u64> {
        self.size
    }

    /// Returns the last modified time of the file.
    #[inline]
    #[must_use]
    pub const fn modified(&self) -> Option<std::time::SystemTime> {
        self.modified
    }

    /// Returns the file permissions.
    #[inline]
    #[must_use]
    pub const fn permissions(&self) -> Option<crate::filesystem::FilePermissions> {
        self.permissions
    }
}
