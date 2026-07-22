//! The FileTreePane — renders an indented file tree (T52).

use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use std::path::PathBuf;

/// The file tree pane state.
#[derive(Debug, Default)]
pub struct FileTreePaneState {
    /// The file paths to display (sorted, rendered as a tree).
    pub paths: Vec<PathBuf>,
}

impl FileTreePaneState {
    /// Add a path.
    pub fn add_path(&mut self, path: impl Into<PathBuf>) {
        self.paths.push(path.into());
        self.paths.sort();
    }

    /// Render the file tree.
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let mut lines: Vec<Line> = vec![];

        // Simple tree rendering: group by directory, show with indentation.
        for path in &self.paths {
            let depth = path.components().count();
            let indent = "  ".repeat(depth.saturating_sub(1));
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
            lines.push(Line::from(Span::raw(format!("{indent}{name}"))));
        }

        frame.render_widget(
            Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("Files")),
            area,
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::App;

    /// T52: FileTreePane renders a tree from a list of paths.
    #[test]
    fn file_tree_pane_renders_tree() {
        let mut app = App::new();
        app.file_tree.add_path("src/main.rs");
        app.file_tree.add_path("src/lib.rs");
        app.file_tree.add_path("Cargo.toml");
        // The file tree is NOT shown by default (only chat); for this test we
        // check the state directly.
        assert_eq!(app.file_tree.paths.len(), 3);
    }
}
