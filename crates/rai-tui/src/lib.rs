//! rai-tui — Ratatui application for RAI Code.
#![warn(missing_docs)]

pub mod app;
pub mod palette;
pub mod panes;

pub use app::{render_to_string, App};
pub use palette::{Command, CommandPalette};
pub use panes::browser::{BrowserMode, BrowserPaneState, ConsoleEvent};
pub use panes::chat::{ChatMessage, ChatPaneState, ItvfCard};
pub use panes::diff::{DiffHunk, DiffPaneState, FileDiff};
pub use panes::file_tree::FileTreePaneState;
pub use panes::plan::PlanPaneState;
