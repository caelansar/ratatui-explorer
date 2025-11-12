use std::sync::Arc;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, HighlightSpacing, List, ListState, StatefulWidget, WidgetRef},
};

use crate::{filesystem::FileSystem, File, FileExplorer};

type LineFactory<F> = Arc<dyn Fn(&FileExplorer<F>) -> Line<'static> + Send + Sync>;

pub struct Renderer<'a, F: FileSystem>(pub(crate) &'a FileExplorer<F>);

/// A stateful renderer that can be used with render_stateful_widget.
/// This allows tracking and updating the scroll offset state.
pub struct StatefulRenderer<'a, F: FileSystem>(pub(crate) &'a mut FileExplorer<F>);

impl<F: FileSystem> StatefulRenderer<'_, F> {
    /// Render the file explorer widget with stateful tracking of scroll position.
    pub fn render(self, area: Rect, buf: &mut Buffer) {
        // Get filtered files and selected index
        let files = self.0.files();
        let selected_idx = self.0.selected_idx();

        let mut state = ListState::default()
            .with_selected(Some(selected_idx))
            .with_offset(self.0.scroll_offset());

        // Check if current item is selected
        let current_is_selected = self.0.is_file_selected(self.0.current());

        let highlight_style = if current_is_selected {
            // If current item is selected, always use cyan foreground
            if self.0.current().is_dir() {
                self.0.theme().highlight_dir_style.fg(Color::Cyan)
            } else {
                self.0.theme().highlight_item_style.fg(Color::Cyan)
            }
        } else {
            if self.0.current().is_dir() {
                self.0.theme().highlight_dir_style
            } else {
                self.0.theme().highlight_item_style
            }
        };

        let mut list = List::new(files.iter().map(|file| {
            let is_selected = self.0.is_file_selected(file);
            file.text(self.0.theme(), is_selected)
        }))
        .style(self.0.theme().style)
        .highlight_spacing(self.0.theme().highlight_spacing.clone())
        .highlight_style(highlight_style)
        .scroll_padding(self.0.theme().scroll_padding);

        if let Some(symbol) = self.0.theme().highlight_symbol.as_deref() {
            list = list.highlight_symbol(symbol);
        }

        if let Some(block) = self.0.theme().block.as_ref() {
            let mut block = block.clone();

            for title_top in self.0.theme().title_top(self.0) {
                block = block.title_top(title_top);
            }
            for title_bottom in self.0.theme().title_bottom(self.0) {
                block = block.title_bottom(title_bottom);
            }

            list = list.block(block);
        }

        list.render(area, buf, &mut state);

        // Update scroll offset after rendering
        self.0.set_scroll_offset(state.offset());
    }
}

impl<F: FileSystem> WidgetRef for Renderer<'_, F> {
    fn render_ref(&self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        // Get filtered files and selected index
        let files = self.0.files();
        let selected_idx = self.0.selected_idx();

        let mut state = ListState::default()
            .with_selected(Some(selected_idx))
            .with_offset(self.0.scroll_offset());

        // Check if current item is selected
        let current_is_selected = self.0.is_file_selected(self.0.current());

        let highlight_style = if current_is_selected {
            // If current item is selected, always use cyan foreground
            if self.0.current().is_dir() {
                self.0.theme().highlight_dir_style.fg(Color::Cyan)
            } else {
                self.0.theme().highlight_item_style.fg(Color::Cyan)
            }
        } else {
            if self.0.current().is_dir() {
                self.0.theme().highlight_dir_style
            } else {
                self.0.theme().highlight_item_style
            }
        };

        let mut list = List::new(files.iter().map(|file| {
            let is_selected = self.0.is_file_selected(file);
            file.text(self.0.theme(), is_selected)
        }))
        .style(self.0.theme().style)
        .highlight_spacing(self.0.theme().highlight_spacing.clone())
        .highlight_style(highlight_style)
        .scroll_padding(self.0.theme().scroll_padding);

        if let Some(symbol) = self.0.theme().highlight_symbol.as_deref() {
            list = list.highlight_symbol(symbol);
        }

        if let Some(block) = self.0.theme().block.as_ref() {
            let mut block = block.clone();

            for title_top in self.0.theme().title_top(self.0) {
                block = block.title_top(title_top);
            }
            for title_bottom in self.0.theme().title_bottom(self.0) {
                block = block.title_bottom(title_bottom);
            }

            list = list.block(block);
        }

        ratatui::widgets::StatefulWidgetRef::render_ref(&list, area, buf, &mut state);
    }
}

impl File {
    /// Returns the text with the appropriate style to be displayed for the file.
    fn text<F: FileSystem>(&self, theme: &Theme<F>, is_selected: bool) -> Text<'_> {
        let style = if self.is_dir() {
            *theme.dir_style()
        } else {
            *theme.item_style()
        };

        if is_selected {
            let selected_style = style.patch(Style::default().fg(Color::Cyan));
            Span::styled(
                format!("{}{}", theme.selected_marker(), self.name()),
                selected_style,
            )
            .into()
        } else {
            Span::styled(self.name().to_string(), style).into()
        }
    }
}

/// The theme of the file explorer.
///
/// This struct is used to customize the look of the file explorer.
/// It allows to set the style of the widget and the style of the files.
/// You can also wrap the widget in a block with the [`Theme::with_block`](#method.block)
/// method and add customizable titles to it with [`Theme::with_title_top`](#method.title_top)
/// and [`Theme::with_title_bottom`](#method.title_bottom).
#[derive(Clone, educe::Educe)]
#[educe(Debug, PartialEq, Eq, Hash)]
pub struct Theme<F: FileSystem = crate::filesystem::LocalFileSystem> {
    block: Option<Block<'static>>,
    #[educe(Debug(ignore), PartialEq(ignore), Hash(ignore))]
    title_top: Vec<LineFactory<F>>,
    #[educe(Debug(ignore), PartialEq(ignore), Hash(ignore))]
    title_bottom: Vec<LineFactory<F>>,
    style: Style,
    item_style: Style,
    dir_style: Style,
    highlight_spacing: HighlightSpacing,
    highlight_item_style: Style,
    highlight_dir_style: Style,
    highlight_symbol: Option<String>,
    scroll_padding: usize,
    selected_marker: String,
}

impl<F: FileSystem> Theme<F> {
    /// Create a new empty theme.
    ///
    /// The theme will not have any style set. To get a theme with the default style, use [`Theme::default`](#method.default).
    ///
    /// # Example
    /// ```no_run
    /// # use ratatui_explorer::Theme;
    /// let theme = Theme::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            block: None,
            title_top: Vec::new(),
            title_bottom: Vec::new(),
            style: Style::new(),
            item_style: Style::new(),
            dir_style: Style::new(),
            highlight_spacing: HighlightSpacing::WhenSelected,
            highlight_item_style: Style::new(),
            highlight_dir_style: Style::new(),
            highlight_symbol: None,
            scroll_padding: 0,
            selected_marker: "[✓] ".to_string(),
        }
    }

    /// Add a top title to the theme.
    /// The title is the current working directory.
    ///
    /// # Example
    /// Suppose you have this tree file, with `passport.png` selected inside `file_explorer`:
    /// ```plaintext
    /// /
    /// ├── .git
    /// └── Documents
    ///     ├── passport.png  <- selected
    ///     └── resume.pdf
    /// ```
    /// You will end up with something like this:
    /// ```plaintext
    /// ┌/Documents────────────────────────┐
    /// │ ../                              │
    /// │ passport.png                     │
    /// │ resume.pdf                       │
    /// └──────────────────────────────────┘
    /// ```
    /// With this code:
    /// ```no_run
    /// use ratatui::widgets::*;
    /// use ratatui_explorer::{FileExplorer, Theme};
    ///
    /// let theme = Theme::default()
    ///     .with_block(Block::default().borders(Borders::ALL))
    ///     .add_default_title();
    ///
    /// let file_explorer = FileExplorer::with_theme(theme).unwrap();
    ///
    /// /* user select `password.png` */
    ///
    /// let widget = file_explorer.widget();
    /// /* render the widget */
    /// ```
    #[inline]
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn add_default_title(self) -> Self {
        self.with_title_top(|file_explorer: &FileExplorer<F>| {
            Line::from(file_explorer.cwd().display().to_string())
        })
    }

    /// Wrap the file explorer with a custom [`Block`](https://docs.rs/ratatui/latest/ratatui/widgets/block/struct.Block.html) widget.
    ///
    /// Behind the scene, it use the [`List::block`](https://docs.rs/ratatui/latest/ratatui/widgets/struct.List.html#method.block) method. See its documentation for more.
    ///
    /// You can use [`Theme::with_title_top`](#method.title_top) and [`Theme::with_title_bottom`](#method.title_bottom)
    /// to add customizable titles to the block.
    ///
    /// # Example
    /// ```no_run
    /// # use ratatui::widgets::*;
    /// # use ratatui_explorer::Theme;
    /// let theme = Theme::default().with_block(Block::default().borders(Borders::ALL));
    /// ```
    #[inline]
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn with_block(mut self, block: Block<'static>) -> Self {
        self.block = Some(block);
        self
    }

    /// Set the style of the widget.
    ///
    /// Behind the scene, it use the [`List::style`](https://docs.rs/ratatui/latest/ratatui/widgets/struct.List.html#method.style) method. See its documentation for more.
    ///
    /// # Example
    /// ```no_run
    /// # use ratatui::prelude::*;
    /// # use ratatui_explorer::Theme;
    /// let theme = Theme::default().with_style(Style::default().fg(Color::Yellow));
    /// ```
    #[inline]
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn with_style<S: Into<Style>>(mut self, style: S) -> Self {
        self.style = style.into();
        self
    }

    /// Set the style of all non directories items. To set the style of the directories, use [`Theme::with_dir_style`](#method.dir_style).
    ///
    /// Behind the scene, it use the [`Span::styled`](https://docs.rs/ratatui/latest/ratatui/text/struct.Span.html#method.styled) method. See its documentation for more.
    ///
    /// # Example
    /// ```no_run
    /// # use ratatui::prelude::*;
    /// # use ratatui_explorer::Theme;
    /// let theme = Theme::default().with_item_style(Style::default().fg(Color::White));
    /// ```
    #[inline]
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn with_item_style<S: Into<Style>>(mut self, item_style: S) -> Self {
        self.item_style = item_style.into();
        self
    }

    /// Set the style of all directories items. To set the style of the non directories, use [`Theme::with_item_style`](#method.item_style).
    ///
    /// Behind the scene, it use the [`Span::styled`](https://docs.rs/ratatui/latest/ratatui/text/struct.Span.html#method.styled) method. See its documentation for more.
    ///
    /// # Example
    /// ```no_run
    /// # use ratatui::prelude::*;
    /// # use ratatui_explorer::Theme;
    /// let theme = Theme::default().with_dir_style(Style::default().fg(Color::Blue));
    /// ```
    #[inline]
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn with_dir_style<S: Into<Style>>(mut self, dir_style: S) -> Self {
        self.dir_style = dir_style.into();
        self
    }

    /// Set the style of all highlighted non directories items. To set the style of the highlighted directories, use [`Theme::with_highlight_dir_style`](#method.highlight_dir_style).
    ///
    /// Behind the scene, it use the [`List::highlight_style`](https://docs.rs/ratatui/latest/ratatui/widgets/struct.List.html#method.highlight_style) method. See its documentation for more.
    ///
    /// # Example
    /// ```no_run
    /// # use ratatui::prelude::*;
    /// # use ratatui_explorer::Theme;
    /// let theme = Theme::default().with_highlight_item_style(Style::default().add_modifier(Modifier::BOLD));
    /// ```
    #[inline]
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn with_highlight_item_style<S: Into<Style>>(mut self, highlight_item_style: S) -> Self {
        self.highlight_item_style = highlight_item_style.into();
        self
    }

    /// Set the style of all highlighted directories items. To set the style of the highlighted non directories, use [`Theme::with_highlight_item_style`](#method.highlight_item_style).
    ///
    /// Behind the scene, it use the [`List::highlight_style`](https://docs.rs/ratatui/latest/ratatui/widgets/struct.List.html#method.highlight_style) method. See its documentation for more.
    ///
    /// # Example
    /// ```no_run
    /// # use ratatui::prelude::*;
    /// # use ratatui_explorer::Theme;
    /// let theme = Theme::default().with_highlight_dir_style(Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD));
    /// ```
    #[inline]
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn with_highlight_dir_style<S: Into<Style>>(mut self, highlight_dir_style: S) -> Self {
        self.highlight_dir_style = highlight_dir_style.into();
        self
    }

    /// Set the symbol used to highlight the selected item.
    ///
    /// Behind the scene, it use the [List::highlight_symbol](https://docs.rs/ratatui/latest/ratatui/widgets/struct.List.html#method.highlight_symbol) method. See its documentation for more.
    ///
    /// # Example
    /// ```no_run
    /// # use ratatui_explorer::Theme;
    /// let theme = Theme::default().with_highlight_symbol("> ");
    /// ```
    #[inline]
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn with_highlight_symbol(mut self, highlight_symbol: &str) -> Self {
        self.highlight_symbol = Some(highlight_symbol.to_owned());
        self
    }

    /// Set the spacing between the highlighted item and the other items.
    ///
    /// Behind the scene, it use the [`List::highlight_spacing`](https://docs.rs/ratatui/latest/ratatui/widgets/struct.List.html#method.highlight_spacing) method. See its documentation for more.
    ///
    /// # Example
    /// ```no_run
    /// # use ratatui::widgets::*;
    /// # use ratatui_explorer::Theme;
    /// let theme = Theme::default().with_highlight_spacing(HighlightSpacing::Never);
    /// ```
    #[inline]
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn with_highlight_spacing(mut self, highlight_spacing: HighlightSpacing) -> Self {
        self.highlight_spacing = highlight_spacing;
        self
    }

    /// Sets the number of items around the currently selected item that should be kept visible.
    ///
    /// /// Behind the scene, it use the [List::scroll_padding](https://docs.rs/ratatui/latest/ratatui/widgets/struct.List.html#method.scroll_padding) method. See its documentation for more.
    ///
    /// # Example
    /// ```no_run
    /// # use ratatui::widgets::*;
    /// # use ratatui_explorer::Theme;
    /// let theme = Theme::default().with_scroll_padding(1);
    /// ```
    #[inline]
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn with_scroll_padding(mut self, scroll_padding: usize) -> Self {
        self.scroll_padding = scroll_padding;
        self
    }

    /// Sets the marker string to display before selected files.
    ///
    /// By default, the marker is "[✓] ".
    ///
    /// # Example
    /// ```no_run
    /// # use ratatui_explorer::Theme;
    /// let theme = Theme::default().with_selected_marker("[x] ");
    /// ```
    #[inline]
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn with_selected_marker(mut self, marker: impl Into<String>) -> Self {
        self.selected_marker = marker.into();
        self
    }

    /// Add a top title factory to the theme.
    ///
    /// `title_top` is a function that take a reference to the current [`FileExplorer`] and returns
    /// a [`Line`](https://docs.rs/ratatui/latest/ratatui/text/struct.Line.html)
    /// to be displayed as a title at the top of the wrapping block (if it exist) of the file explorer. You can call
    /// this function multiple times to add multiple titles.
    ///
    /// Behind the scene, it use the [`Block::title_top`](https://docs.rs/ratatui/latest/ratatui/widgets/block/struct.Block.html#method.title_top) method. See its documentation for more.
    ///
    /// # Example
    /// ```no_run
    /// use ratatui::prelude::*;
    /// # use ratatui_explorer::{FileExplorer, Theme};
    /// let theme = Theme::default()
    ///     .with_title_top(|file_explorer: &FileExplorer| {
    ///         Line::from(format!("cwd - {}", file_explorer.cwd().display()))
    ///     })
    ///     .with_title_top(|file_explorer: &FileExplorer| {
    ///         Line::from(format!("{} files", file_explorer.files().len() - 1)).right_aligned()
    ///     });
    /// ```
    #[inline]
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn with_title_top(
        mut self,
        title_top: impl Fn(&FileExplorer<F>) -> Line<'static> + 'static + Send + Sync,
    ) -> Self {
        self.title_top.push(Arc::new(title_top));
        self
    }

    /// Add a bottom title factory to the theme.
    ///
    /// `title_bottom` is a function that take a reference to the current [`FileExplorer`] and returns
    /// a [`Line`](https://docs.rs/ratatui/latest/ratatui/text/struct.Line.html)
    /// to be displayed as a title at the bottom of the wrapping block (if it exist) of the file explorer. You can call
    /// this function multiple times to add multiple titles.
    ///
    /// Behind the scene, it use the [`Block::title_bottom`](https://docs.rs/ratatui/latest/ratatui/widgets/block/struct.Block.html#method.title_bottom) method. See its documentation for more.
    ///
    /// # Example
    /// ```no_run
    /// # use ratatui::prelude::*;
    /// # use ratatui_explorer::{FileExplorer, Theme};
    /// let theme = Theme::default()
    ///     .with_title_bottom(|file_explorer: &FileExplorer| {
    ///         Line::from(format!("cwd - {}", file_explorer.cwd().display()))
    ///     })
    ///     .with_title_bottom(|file_explorer: &FileExplorer| {
    ///         Line::from(format!("{} files", file_explorer.files().len() - 1)).right_aligned()
    ///     });
    /// ```
    #[inline]
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn with_title_bottom(
        mut self,
        title_bottom: impl Fn(&FileExplorer<F>) -> Line<'static> + 'static + Send + Sync,
    ) -> Self {
        self.title_bottom.push(Arc::new(title_bottom));
        self
    }

    /// Returns the wrapping block (if it exist) of the file explorer of the theme.
    #[inline]
    #[must_use]
    pub const fn block(&self) -> Option<&Block<'static>> {
        self.block.as_ref()
    }

    /// Returns the style of the widget of the theme.
    #[inline]
    #[must_use]
    pub const fn style(&self) -> &Style {
        &self.style
    }

    /// Returns the style of the non directories items of the theme.
    #[inline]
    #[must_use]
    pub const fn item_style(&self) -> &Style {
        &self.item_style
    }

    /// Returns the style of the directories items of the theme.
    #[inline]
    #[must_use]
    pub const fn dir_style(&self) -> &Style {
        &self.dir_style
    }

    /// Returns the style of the highlighted non directories items of the theme.
    #[inline]
    #[must_use]
    pub const fn highlight_item_style(&self) -> &Style {
        &self.highlight_item_style
    }

    /// Returns the style of the highlighted directories items of the theme.
    #[inline]
    #[must_use]
    pub const fn highlight_dir_style(&self) -> &Style {
        &self.highlight_dir_style
    }

    /// Returns the symbol used to highlight the selected item of the theme.
    #[inline]
    #[must_use]
    pub fn highlight_symbol(&self) -> Option<&str> {
        self.highlight_symbol.as_deref()
    }

    /// Returns the spacing between the highlighted item and the other items of the theme.
    #[inline]
    #[must_use]
    pub const fn highlight_spacing(&self) -> &HighlightSpacing {
        &self.highlight_spacing
    }

    /// Returns the number of items around the currently selected item that should be kept visible.
    #[inline]
    #[must_use]
    pub const fn scroll_padding(&self) -> usize {
        self.scroll_padding
    }

    /// Returns the marker string displayed before selected files.
    #[inline]
    #[must_use]
    pub fn selected_marker(&self) -> &str {
        &self.selected_marker
    }

    /// Returns the generated top titles of the theme.
    #[inline]
    #[must_use]
    pub fn title_top(&self, file_explorer: &FileExplorer<F>) -> Vec<Line> {
        self.title_top
            .iter()
            .map(|title_top| title_top(file_explorer))
            .collect()
    }

    /// Returns the generated bottom titles of the theme.
    #[inline]
    #[must_use]
    pub fn title_bottom(&self, file_explorer: &FileExplorer<F>) -> Vec<Line> {
        self.title_bottom
            .iter()
            .map(|title_bottom| title_bottom(file_explorer))
            .collect()
    }
}

impl<F: FileSystem> Default for Theme<F> {
    /// Return a slightly customized default theme. To get a theme with no style set, use [`Theme::new`](#method.new).
    ///
    /// The theme will have a block with all borders, a white style for the items, a light blue style for the directories,
    /// a dark gray background for all the highlighted items.
    ///
    /// # Example
    /// ```no_run
    /// # use ratatui_explorer::Theme;
    /// let theme = Theme::default();
    /// ```
    fn default() -> Self {
        Self {
            block: Some(Block::default().borders(Borders::ALL)),
            title_top: Vec::new(),
            title_bottom: Vec::new(),
            style: Style::default(),
            item_style: Style::default().fg(Color::White),
            dir_style: Style::default().fg(Color::LightBlue),
            highlight_spacing: HighlightSpacing::Always,
            highlight_item_style: Style::default().fg(Color::White).bg(Color::Cyan),
            highlight_dir_style: Style::default().fg(Color::LightBlue).bg(Color::Cyan),
            highlight_symbol: None,
            scroll_padding: 0,
            selected_marker: "[✓] ".to_string(),
        }
    }
}
