//! The PlanPane — renders the task plan graph (placeholder for Phase 10+).

use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// The plan pane state.
#[derive(Debug, Default)]
pub struct PlanPaneState {
    /// The plan text (a simple tree for now; the full graph viz is later).
    pub text: String,
}

impl PlanPaneState {
    /// Set the plan text.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    /// Render the plan pane.
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        frame.render_widget(
            Paragraph::new(self.text.as_str())
                .block(Block::default().borders(Borders::ALL).title("Plan")),
            area,
        );
    }
}
