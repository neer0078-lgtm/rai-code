//! Two-tier state — clean-room pattern (non-copyrightable).
//!
//! Bootstrap state: OnceLock for one-time init (config, API keys, Python interpreter).
//! App state: rebuilt each Ratatui frame (immediate-mode — render is a pure fn of state).
//! For reactive cross-task updates, tokio::sync::watch channels or Arc<RwLock<AppState>>.

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Bootstrap state — initialized once at startup.
pub struct BootstrapState {
    /// The runtime config.
    pub config: Config,
    /// The provider API keys (local-only mode uses none).
    pub api_keys: ApiKeys,
    // TODO(T41+): python interpreter handle (when full feature), graph/hindsight clients.
}

impl BootstrapState {
    /// Initialize bootstrap state with config + API keys.
    pub fn init(config: Config, api_keys: ApiKeys) -> Self {
        Self { config, api_keys }
    }
}

/// The reactive app state, shared across the TUI + agent loop.
#[derive(Default)]
pub struct AppState {
    /// The conversation messages.
    pub messages: Vec<Message>,
    /// The token currently being streamed (for live rendering), if any.
    pub streaming_token: Option<String>,
    /// Which panes are visible.
    pub active_panes: PaneLayout,
    /// The active permission mode.
    pub permission_mode: crate::perm::PermissionMode,
    /// The active escalation mode (default None = single-loop).
    pub escalation: crate::escalation::EscalationMode,
    /// The active context-management strategy, if any.
    pub context_strategy: Option<crate::escalation::ContextStrategy>,
    // TODO(T10+): permission queue, file tree cache, plan graph, browser state.
}

/// Shared, thread-safe app state.
pub type SharedAppState = Arc<RwLock<AppState>>;

/// Which panes the TUI is currently showing.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PaneLayout {
    /// Whether the browser pane is visible.
    pub show_browser: bool,
    /// Whether the diff pane is visible.
    pub show_diff: bool,
    /// Whether the plan graph is visible.
    pub show_plan: bool,
}

/// The RAI Code runtime config.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    /// The model id to use.
    pub model: String,
    /// The provider key (anthropic, openai, ollama-local, ...).
    pub provider: String,
    /// An optional custom endpoint base URL.
    pub base_url: Option<String>,
    /// Whether to run in local-only (offline) mode.
    pub local_mode: bool,
}

/// API keys for each provider (local-only mode uses none).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ApiKeys {
    /// Anthropic API key.
    pub anthropic: Option<String>,
    /// OpenAI API key.
    pub openai: Option<String>,
    /// Google (Gemini) API key.
    pub google: Option<String>,
    /// xAI (Grok) API key.
    pub xai: Option<String>,
    /// OpenRouter API key.
    pub openrouter: Option<String>,
    /// LiteLLM proxy key (if using a LiteLLM gateway).
    pub litellm: Option<String>,
}

/// A single conversation message.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Message {
    /// The role: "user", "assistant", "system", "tool".
    pub role: String,
    /// The message content.
    pub content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T10: AppState::default() is empty + SharedAppState constructs.
    #[test]
    fn app_state_default_empty() {
        let s = AppState::default();
        assert!(s.messages.is_empty());
        assert!(s.streaming_token.is_none());
        assert_eq!(s.permission_mode, crate::perm::PermissionMode::Approval);
        assert_eq!(s.escalation, crate::escalation::EscalationMode::None);
        assert!(s.context_strategy.is_none());

        // SharedAppState (Arc<RwLock<AppState>>) constructs.
        let shared: SharedAppState =
            std::sync::Arc::new(parking_lot::RwLock::new(AppState::default()));
        let _ = shared; // compiles + (no deadlock from a read)
    }
}
