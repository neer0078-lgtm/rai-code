//! The ChatPane — renders conversation messages + streaming tokens + ITVF cards.
//!
//! T50: ChatPane renders messages + streaming token.
//! T57: ITVF iteration card.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// A single chat message for rendering.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    /// The role: "user", "assistant", "tool".
    pub role: String,
    /// The content text.
    pub content: String,
}

/// An ITVF iteration card for rendering (T57).
#[derive(Debug, Clone)]
pub struct ItvfCard {
    /// The task id.
    pub task_id: String,
    /// The current iteration (N).
    pub iteration: u32,
    /// The max iterations (M).
    pub max_iter: u32,
    /// The current phase.
    pub phase: String,
    /// The last verify result ("pass" or the failure message).
    pub last_verify: String,
}

/// The chat pane state.
#[derive(Debug, Default)]
pub struct ChatPaneState {
    /// The messages to display.
    pub messages: Vec<ChatMessage>,
    /// The token currently being streamed (live rendering).
    pub streaming_token: Option<String>,
    /// The ITVF cards to display.
    pub itvf_cards: Vec<ItvfCard>,
}

impl ChatPaneState {
    /// Add a message.
    pub fn add_message(&mut self, role: impl Into<String>, content: impl Into<String>) {
        self.messages.push(ChatMessage {
            role: role.into(),
            content: content.into(),
        });
    }

    /// Set the streaming token.
    pub fn set_streaming(&mut self, token: impl Into<String>) {
        self.streaming_token = Some(token.into());
    }

    /// Add an ITVF card.
    pub fn add_itvf_card(&mut self, card: ItvfCard) {
        self.itvf_cards.push(card);
    }

    /// Render the chat pane.
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let mut lines: Vec<Line> = vec![];

        for msg in &self.messages {
            let (prefix, color) = match msg.role.as_str() {
                "user" => ("▸ user: ", Color::Green),
                "assistant" | "rai" => ("▸ rai: ", Color::Cyan),
                "tool" => ("  ⋯ ", Color::DarkGray),
                _ => ("▸ ", Color::White),
            };
            lines.push(Line::from(vec![
                Span::styled(
                    prefix,
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::raw(&msg.content),
            ]));
        }

        // Streaming token (if any).
        if let Some(ref token) = self.streaming_token {
            lines.push(Line::from(vec![
                Span::styled(
                    "▸ rai: ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(token),
            ]));
        }

        // ITVF cards (T57).
        for card in &self.itvf_cards {
            let verify_span = if card.last_verify == "pass" {
                Span::styled("✓", Style::default().fg(Color::Green))
            } else {
                Span::styled("✗", Style::default().fg(Color::Red))
            };
            lines.push(Line::from(vec![
                Span::styled(
                    format!(
                        "  ⋯ T{} ITVF {}/{} ",
                        card.task_id, card.iteration, card.max_iter
                    ),
                    Style::default().fg(Color::Yellow),
                ),
                Span::raw(format!("[{}] ", card.phase)),
                verify_span,
                Span::raw(format!(" last: {}", card.last_verify)),
            ]));
        }

        frame.render_widget(
            Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("Chat")),
            area,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::render_to_string;
    use crate::App;

    /// T50: ChatPane renders messages + streaming token.
    #[test]
    fn chat_pane_renders_messages_and_streaming() {
        let mut app = App::new();
        app.chat.add_message("user", "implement the Tool trait");
        app.chat.add_message("rai", "on it. reading tool.rs...");
        app.chat.set_streaming("writing");
        let content = render_to_string(&app, 80, 24);
        assert!(
            content.contains("implement the Tool trait"),
            "should show user message:\n{content}"
        );
        assert!(
            content.contains("on it"),
            "should show rai message:\n{content}"
        );
        assert!(
            content.contains("writing"),
            "should show streaming token:\n{content}"
        );
    }

    /// T57: ITVF card renders with iteration count + verify symbol.
    #[test]
    fn itvf_card_renders() {
        let mut app = App::new();
        app.chat.add_itvf_card(ItvfCard {
            task_id: "07".into(),
            iteration: 2,
            max_iter: 8,
            phase: "verify".into(),
            last_verify: "pass".into(),
        });
        let content = render_to_string(&app, 80, 24);
        assert!(content.contains("T07"), "should show task id:\n{content}");
        assert!(content.contains("ITVF"), "should show ITVF:\n{content}");
        assert!(
            content.contains("2/8"),
            "should show iteration 2/8:\n{content}"
        );
    }
}
