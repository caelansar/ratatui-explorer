use std::io::{self, stdout};

use crossterm::{
    event::{read, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::crossterm;
use ratatui::{prelude::*, widgets::*};

use ratatui_async_explorer::{File, FileExplorer, Theme};

#[tokio::main]
async fn main() -> io::Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let theme = Theme::default().add_default_title();
    let mut file_explorer = FileExplorer::with_theme(theme).await?;

    let mut search_input = String::new();
    let mut is_searching = false;

    loop {
        terminal.draw(|f| {
            let chunks =
                Layout::vertical([Constraint::Min(1), Constraint::Length(3)]).split(f.area());

            f.render_widget(&file_explorer.widget(), chunks[0]);

            // Render the search bar at the bottom.
            let search_text = if is_searching {
                format!("Search: {}_", search_input)
            } else if search_input.is_empty() {
                "Press '/' to search".to_string()
            } else {
                format!("Search: {} (press '/' to edit, Esc to clear)", search_input)
            };
            f.render_widget(
                Paragraph::new(search_text).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded),
                ),
                chunks[1],
            );
        })?;

        let event = read()?;
        if let Event::Key(key) = &event {
            if is_searching {
                match key.code {
                    KeyCode::Esc => {
                        // Exit search mode and clear filter.
                        is_searching = false;
                        search_input.clear();
                        file_explorer.clear_filter();
                    }
                    KeyCode::Enter => {
                        // Confirm search and exit search mode.
                        is_searching = false;
                    }
                    KeyCode::Backspace => {
                        search_input.pop();
                        apply_filter(&mut file_explorer, &search_input);
                    }
                    KeyCode::Char(c) => {
                        search_input.push(c);
                        apply_filter(&mut file_explorer, &search_input);
                    }
                    _ => {}
                }
                continue;
            }

            match key.code {
                KeyCode::Esc => {
                    is_searching = false;
                    search_input.clear();
                    file_explorer.clear_filter();
                }
                KeyCode::Char('q') => break,
                KeyCode::Char('/') => {
                    is_searching = true;
                }
                _ => {}
            }
        }

        if !is_searching {
            file_explorer.handle(&event).await?;
        }
    }

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

fn apply_filter(file_explorer: &mut FileExplorer, query: &str) {
    if query.is_empty() {
        file_explorer.clear_filter();
    } else {
        let query = query.to_lowercase();
        file_explorer.set_filter(Some(move |file: &File| {
            if file.name().to_lowercase().contains(&query) {
                Some(file.clone())
            } else {
                None
            }
        }));
    }
}
