//! The BrowserPane — renders the a11y-tree text + console/network panes (T54).
//!
//! Four switchable modes: Screenshot / DOM(a11y) / Console / Network.
//! The a11y-tree text is the default (~300 tokens); screenshots are on-demand.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// The browser pane mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BrowserMode {
    /// The a11y-tree text (default — cheap).
    #[default]
    DomA11y,
    /// A screenshot (ratatui-image — on-demand).
    Screenshot,
    /// The console log (errors/warnings).
    Console,
    /// The network log (requests/responses/failures).
    Network,
}

/// A console event for rendering.
#[derive(Debug, Clone)]
pub struct ConsoleEvent {
    /// The level: "error", "warn", "info".
    pub level: String,
    /// The message.
    pub message: String,
}

/// The browser pane state.
#[derive(Debug, Default)]
pub struct BrowserPaneState {
    /// The current mode.
    pub mode: BrowserMode,
    /// The a11y-tree text (the DOM snapshot).
    pub a11y_text: String,
    /// The console events.
    pub console_events: Vec<ConsoleEvent>,
    /// The network log lines.
    pub network_lines: Vec<String>,
}

impl BrowserPaneState {
    /// Set the a11y text.
    pub fn set_a11y(&mut self, text: impl Into<String>) {
        self.a11y_text = text.into();
    }

    /// Add a console event.
    pub fn add_console(&mut self, level: impl Into<String>, msg: impl Into<String>) {
        self.console_events.push(ConsoleEvent {
            level: level.into(),
            message: msg.into(),
        });
    }

    /// Add a network line.
    pub fn add_network(&mut self, line: impl Into<String>) {
        self.network_lines.push(line.into());
    }

    /// Render the browser pane.
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(8)])
            .split(area);

        // Top: the mode-specific content (a11y text by default).
        let content = match self.mode {
            BrowserMode::DomA11y => self.a11y_text.as_str(),
            BrowserMode::Screenshot => "(screenshot — ratatui-image in full impl)",
            BrowserMode::Console => "(console — see below)",
            BrowserMode::Network => "(network — see below)",
        };
        frame.render_widget(
            Paragraph::new(content).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!("Browser [{:?}]", self.mode)),
            ),
            chunks[0],
        );

        // Bottom: console + network (always visible when browser is shown).
        let bottom = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[1]);

        let console_lines: Vec<Line> = self
            .console_events
            .iter()
            .map(|e| {
                let color = match e.level.as_str() {
                    "error" => Color::Red,
                    "warn" => Color::Yellow,
                    _ => Color::White,
                };
                Line::from(Span::styled(
                    format!("[{}] {}", e.level, e.message),
                    Style::default().fg(color),
                ))
            })
            .collect();
        frame.render_widget(
            Paragraph::new(console_lines)
                .block(Block::default().borders(Borders::ALL).title("Console")),
            bottom[0],
        );

        let network_lines: Vec<Line> = self
            .network_lines
            .iter()
            .map(|l| Line::raw(l.as_str()))
            .collect();
        frame.render_widget(
            Paragraph::new(network_lines)
                .block(Block::default().borders(Borders::ALL).title("Network")),
            bottom[1],
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::app::render_to_string;
    use crate::App;

    /// T54: BrowserPane renders a11y text + console events.
    #[test]
    fn browser_pane_renders_a11y_and_console() {
        let mut app = App::new().show_browser(true);
        app.browser
            .set_a11y("- navigation\n  - link \"Home\" [A0]\n  - link \"Settings\" [A1]\n");
        app.browser.add_console("error", "TypeError at App.tsx:42");
        app.browser.add_console("warn", "fetch 404 /api/users");
        app.browser.add_network("GET / -> 200");
        app.browser.add_network("GET /api/users -> 404 FAIL");
        let content = render_to_string(&app, 100, 30);
        assert!(
            content.contains("[A0]"),
            "should show a11y ref marker:\n{content}"
        );
        assert!(
            content.contains("TypeError"),
            "should show console error:\n{content}"
        );
        assert!(
            content.contains("404"),
            "should show network failure:\n{content}"
        );
    }
}
