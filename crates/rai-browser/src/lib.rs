//! rai-browser — the TUI-embedded browser for testing/debugging the apps RAI Code builds.
//!
//! chromiumoxide (pure-Rust CDP) drives headless Chromium; ratatui-image renders
//! screenshots in the BrowserPane via Kitty/iTerm2/Sixel/half-blocks (auto-detect).
//!
//! DEFAULT observation = accessibility-tree text (~200-500 tokens/page) — the cheap mode.
//! FALLBACK = screenshot (~1-2K image tokens) — on-demand only (visual layout / unresolvable target).
//! Stream only diffs after the first snapshot; capture only failures (console errors, network 4xx/5xx).
//!
//! Agent browser tools (MCP-style, backed by chromiumoxide):
//!   browser_navigate, browser_snapshot (a11y), browser_click(ref), browser_type(ref,text),
//!   browser_screenshot, browser_get_console_errors, browser_get_network_failures,
//!   browser_assert_text, browser_run_playwright_test, browser_get_source_map_stack,
//!   browser_evaluate, browser_set_viewport.
//!
//! E2B sandbox runs the app + headless Chromium together (CDP :9222, public URL).
#![warn(missing_docs)]

pub mod cdp;
pub mod pane;
pub mod tools;

pub use tools::{serialize_a11y, BrowserAction};
