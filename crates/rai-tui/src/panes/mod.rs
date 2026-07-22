//! The TUI panes: Chat, Diff, FileTree, Browser, Plan.
//!
//! Each pane has a State struct (the data) + a render method (immediate-mode —
//! the pane is rebuilt each frame from its state). The UX follows docs/UX-DESIGN.md.

pub mod browser;
pub mod chat;
pub mod diff;
pub mod file_tree;
pub mod plan;

pub use browser::BrowserPaneState;
pub use chat::ChatPaneState;
pub use diff::DiffPaneState;
pub use file_tree::FileTreePaneState;
pub use plan::PlanPaneState;
