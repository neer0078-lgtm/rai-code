//! The top-level Ratatui App — holds AppState, renders the status bar + content
//! area + key hint bar, and runs the event loop.
//!
//! T49: App::new + TestBackend render (status bar shows "RAI Code" + "RAI Labs").
//! T55: event loop (crossterm async + tokio::select! — stubbed for tests).

use crate::panes::{BrowserPaneState, ChatPaneState, DiffPaneState, FileTreePaneState};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// The RAI Code TUI application state.
pub struct App {
    /// The chat pane state.
    pub chat: ChatPaneState,
    /// The diff pane state.
    pub diff: DiffPaneState,
    /// The file tree pane state.
    pub file_tree: FileTreePaneState,
    /// The browser pane state.
    pub browser: BrowserPaneState,
    /// Whether the browser pane is visible.
    pub show_browser: bool,
    /// Whether the diff pane is visible.
    pub show_diff: bool,
    /// The status bar text (model, mode, ctx, etc.).
    pub status: String,
    /// The key hint bar text.
    pub key_hints: String,
}

impl App {
    /// Construct a new App with default state.
    pub fn new() -> Self {
        Self {
            chat: ChatPaneState::default(),
            diff: DiffPaneState::default(),
            file_tree: FileTreePaneState::default(),
            browser: BrowserPaneState::default(),
            show_browser: false,
            show_diff: false,
            status: "RAI Code · RAI Labs · model: qwen3-coder-32b · mode: Approval".into(),
            key_hints: "[Space] commands  [d] diff  [b] browser  [?] help  [Shift+Tab] mode".into(),
        }
    }

    /// Set the status bar text.
    pub fn with_status(mut self, status: impl Into<String>) -> Self {
        self.status = status.into();
        self
    }

    /// Show/hide the browser pane.
    pub fn show_browser(mut self, show: bool) -> Self {
        self.show_browser = show;
        self
    }

    /// Show/hide the diff pane.
    pub fn show_diff(mut self, show: bool) -> Self {
        self.show_diff = show;
        self
    }

    /// Render the app to a frame (the main render function).
    pub fn render(&self, frame: &mut Frame) {
        let area = frame.area();

        // Layout: status bar (3 lines) + content (flex) + key hints (1 line).
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // status bar
                Constraint::Min(1),    // content
                Constraint::Length(1), // key hints
            ])
            .split(area);

        // Status bar.
        frame.render_widget(
            Paragraph::new(self.status.as_str()).style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            chunks[0],
        );

        // Content area: chat (left) + optional browser (right) or diff (fullscreen).
        if self.show_diff {
            // Diff review fullscreen takeover.
            self.diff.render(frame, chunks[1]);
        } else if self.show_browser {
            // Chat + browser side-by-side.
            let content = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                .split(chunks[1]);
            self.chat.render(frame, content[0]);
            self.browser.render(frame, content[1]);
        } else {
            // Chat only.
            self.chat.render(frame, chunks[1]);
        }

        // Key hint bar.
        frame.render_widget(
            Paragraph::new(self.key_hints.as_str()).style(Style::default().fg(Color::DarkGray)),
            chunks[2],
        );
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

/// Render the app on a TestBackend and return the buffer content as a string
/// (for tests — flattens the terminal buffer into a single string).
pub fn render_to_string(app: &App, width: u16, height: u16) -> String {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal.draw(|f| app.render(f)).expect("draw");

    let buffer = terminal.backend().buffer();
    let mut content = String::new();
    for y in 0..height {
        for x in 0..width {
            if let Some(cell) = buffer.cell((x, y)) {
                content.push_str(cell.symbol());
            }
        }
        content.push('\n');
    }
    content
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T49: App renders the status bar with "RAI Code" and "RAI Labs".
    #[test]
    fn app_renders_status_bar() {
        let app = App::new();
        let content = render_to_string(&app, 80, 24);
        assert!(
            content.contains("RAI Code"),
            "status bar should contain 'RAI Code':\n{content}"
        );
        assert!(
            content.contains("RAI Labs"),
            "status bar should contain 'RAI Labs':\n{content}"
        );
    }

    /// T49: the key hint bar renders.
    #[test]
    fn app_renders_key_hints() {
        let app = App::new();
        let content = render_to_string(&app, 80, 24);
        assert!(
            content.contains("[Space]"),
            "key hints should contain '[Space]':\n{content}"
        );
    }

    /// T49: custom status text renders.
    #[test]
    fn app_custom_status() {
        let app = App::new().with_status("custom status here");
        let content = render_to_string(&app, 80, 24);
        assert!(
            content.contains("custom status"),
            "custom status should render:\n{content}"
        );
    }
}
