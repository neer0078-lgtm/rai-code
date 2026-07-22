//! The DiffPane — renders syntax-highlighted file diffs (T51).
//!
//! Shows added lines (green +) and removed lines (red -) with the path header.
//! Hunk-level accept/reject is wired in the event loop (Phase 10 event handling).

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// A single diff hunk.
#[derive(Debug, Clone)]
pub struct DiffHunk {
    /// The hunk content (unified diff lines).
    pub lines: Vec<String>,
}

/// A file diff.
#[derive(Debug, Clone)]
pub struct FileDiff {
    /// The file path.
    pub path: String,
    /// The hunks.
    pub hunks: Vec<DiffHunk>,
}

/// The diff pane state.
#[derive(Debug, Default)]
pub struct DiffPaneState {
    /// The diffs to display.
    pub diffs: Vec<FileDiff>,
}

impl DiffPaneState {
    /// Add a file diff.
    pub fn add_diff(&mut self, path: impl Into<String>, lines: Vec<String>) {
        self.diffs.push(FileDiff {
            path: path.into(),
            hunks: vec![DiffHunk { lines }],
        });
    }

    /// Render the diff pane.
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let mut lines: Vec<Line> = vec![];

        for diff in &self.diffs {
            lines.push(Line::from(Span::styled(
                format!("--- {} ---", diff.path),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )));
            for hunk in &diff.hunks {
                for line in &hunk.lines {
                    let (color, prefix) = if line.starts_with('+') {
                        (Color::Green, "")
                    } else if line.starts_with('-') {
                        (Color::Red, "")
                    } else {
                        (Color::White, "")
                    };
                    lines.push(Line::from(Span::styled(
                        format!("{prefix}{line}"),
                        Style::default().fg(color),
                    )));
                }
            }
            lines.push(Line::raw(""));
        }

        frame.render_widget(
            Paragraph::new(lines)
                .block(Block::default().borders(Borders::ALL).title("Diff Review")),
            area,
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::app::render_to_string;
    use crate::App;

    /// T51: DiffPane renders file diffs with + and - markers.
    #[test]
    fn diff_pane_renders_hunks() {
        let mut app = App::new().show_diff(true);
        app.diff.add_diff(
            "crates/rai-core/src/tool.rs",
            vec![
                "+use serde_json::Value;".into(),
                "+".into(),
                "+pub trait Tool: Send + Sync {".into(),
                "-// TODO: implement".into(),
            ],
        );
        let content = render_to_string(&app, 80, 24);
        assert!(
            content.contains("tool.rs"),
            "should show file path:\n{content}"
        );
        assert!(
            content.contains("+pub trait Tool"),
            "should show added line:\n{content}"
        );
        assert!(
            content.contains("-// TODO"),
            "should show removed line:\n{content}"
        );
    }
}
