//! The LLM client — a thin wrapper over `genai::Client` with RAI Code's
//! multi-provider config (custom endpoints, local models, LiteLLM proxy).
//!
//! T20's testable surface is the pure `endpoint_for(provider)` resolver and
//! the `Config` builder — no network. The actual genai streaming wiring lands
//! in a later task (the loop wiring, T43+).

use crate::provider::{ChatRequest, ChatResponse, ChatStream, Provider};
use serde::{Deserialize, Serialize};

/// The provider key (used by `endpoint_for` + config).
pub type ProviderKey = String;

/// RAI Code's LLM client config.
///
/// Per user decisions (turn-3): single default + user-configured routing;
/// model-native endpoints + custom endpoints + local models; optional LiteLLM
/// proxy. In `--local` mode, `provider = "ollama-local"` + no api key.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Config {
    /// The provider key: "anthropic", "openai", "google", "xai", "deepseek",
    /// "openrouter", "ollama-local", "vllm-local", "litellm", or a custom key.
    pub provider: ProviderKey,
    /// The default model id (user can override per-request).
    pub model: String,
    /// An optional custom endpoint base URL (for gateways / local servers).
    pub base_url: Option<String>,
    /// An optional API key (None for local-only providers / env vars).
    pub api_key: Option<String>,
}

impl Config {
    /// Construct a config for a provider + model, no key, no custom endpoint.
    pub fn new(provider: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            model: model.into(),
            base_url: None,
            api_key: None,
        }
    }

    /// Set a custom endpoint base URL.
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Set the API key.
    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Whether this config is for a local (offline) provider.
    pub fn is_local(&self) -> bool {
        matches!(
            self.provider.as_str(),
            "ollama-local" | "vllm-local" | "llamacpp-local" | "lm-studio-local"
        )
    }
}

/// The default base URL for a known provider's model.
///
/// Returns `None` for unknown/custom providers (the caller should use
/// `Config.base_url` instead). This is a pure function — no network — so it's
/// directly testable.
///
/// Known mappings:
/// - `anthropic` → `https://api.anthropic.com`
/// - `openai` → `https://api.openai.com`
/// - `google` → `https://generativelanguage.googleapis.com`
/// - `xai` → `https://api.x.ai`
/// - `deepseek` → `https://api.deepseek.com`
/// - `openrouter` → `https://openrouter.ai`
/// - `ollama-local` → `http://localhost:11434`
/// - `vllm-local` → `http://localhost:8000`
/// - `llamacpp-local` → `http://localhost:8080`
/// - `lm-studio-local` → `http://localhost:1234`
/// - `litellm` → `http://localhost:4000` (the LiteLLM proxy default)
pub fn endpoint_for(provider: &str) -> Option<&'static str> {
    match provider {
        "anthropic" => Some("https://api.anthropic.com"),
        "openai" => Some("https://api.openai.com"),
        "google" => Some("https://generativelanguage.googleapis.com"),
        "xai" => Some("https://api.x.ai"),
        "deepseek" => Some("https://api.deepseek.com"),
        "openrouter" => Some("https://openrouter.ai"),
        "ollama-local" => Some("http://localhost:11434"),
        "vllm-local" => Some("http://localhost:8000"),
        "llamacpp-local" => Some("http://localhost:8080"),
        "lm-studio-local" => Some("http://localhost:1234"),
        "litellm" => Some("http://localhost:4000"),
        _ => None,
    }
}

/// The RAI Code LLM client.
///
/// Wraps a boxed `Provider` (so the caller can inject `AnthropicProvider`,
/// `OpenAiProvider`, `MockProvider`, etc.). The actual genai-backed streaming
/// wiring lands in a later task; this struct holds the config + the provider.
pub struct Client {
    /// The client config.
    pub config: Config,
    /// The underlying provider (boxed for polymorphism).
    pub provider: Box<dyn Provider>,
}

impl Client {
    /// Construct a client from a config + a provider.
    pub fn new(config: Config, provider: Box<dyn Provider>) -> Self {
        Self { config, provider }
    }

    /// The resolved endpoint URL for this client (custom base_url overrides the
    /// provider default; pure — no network).
    pub fn endpoint(&self) -> Option<String> {
        if let Some(ref url) = self.config.base_url {
            return Some(url.clone());
        }
        endpoint_for(&self.config.provider).map(|s| s.to_string())
    }

    /// Stream a completion (delegates to the provider).
    pub async fn stream(&self, req: ChatRequest) -> anyhow::Result<ChatStream> {
        self.provider.stream(req).await
    }

    /// One-shot completion (delegates to the provider).
    pub async fn complete(&self, req: ChatRequest) -> anyhow::Result<ChatResponse> {
        self.provider.complete(req).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T20: endpoint_for returns the right URL for known providers, None for unknown.
    #[test]
    fn endpoint_for_known_providers() {
        assert_eq!(endpoint_for("anthropic"), Some("https://api.anthropic.com"));
        assert_eq!(endpoint_for("openai"), Some("https://api.openai.com"));
        assert_eq!(
            endpoint_for("google"),
            Some("https://generativelanguage.googleapis.com")
        );
        assert_eq!(endpoint_for("xai"), Some("https://api.x.ai"));
        assert_eq!(endpoint_for("deepseek"), Some("https://api.deepseek.com"));
        assert_eq!(endpoint_for("openrouter"), Some("https://openrouter.ai"));
        // local providers
        assert_eq!(endpoint_for("ollama-local"), Some("http://localhost:11434"));
        assert_eq!(endpoint_for("vllm-local"), Some("http://localhost:8000"));
        assert_eq!(
            endpoint_for("llamacpp-local"),
            Some("http://localhost:8080")
        );
        assert_eq!(
            endpoint_for("lm-studio-local"),
            Some("http://localhost:1234")
        );
        assert_eq!(endpoint_for("litellm"), Some("http://localhost:4000"));
        // unknown -> None
        assert_eq!(endpoint_for("some-custom-provider"), None);
    }

    /// T20: Config builders + is_local().
    #[test]
    fn config_builders_and_is_local() {
        let local = Config::new("ollama-local", "qwen3-coder-32b");
        assert!(local.is_local());
        assert_eq!(local.model, "qwen3-coder-32b");
        assert!(local.api_key.is_none());

        let cloud = Config::new("anthropic", "claude-sonnet-5")
            .with_base_url("https://my-gw.example.com")
            .with_api_key("sk-x");
        assert!(!cloud.is_local());
        assert_eq!(cloud.base_url.as_deref(), Some("https://my-gw.example.com"));
        assert_eq!(cloud.api_key.as_deref(), Some("sk-x"));
    }

    /// T20: Client::endpoint() — custom base_url overrides the provider default.
    #[cfg(feature = "mock")]
    #[test]
    fn client_endpoint_custom_overrides_default() {
        // No custom base_url -> provider default.
        let cfg = Config::new("ollama-local", "qwen3-coder-32b");
        // Use a mock provider (behind the mock feature) so we don't need network.
        let provider: Box<dyn Provider> = Box::new(crate::mock::MockProvider::abc());
        let c = Client::new(cfg, provider);
        assert_eq!(c.endpoint().as_deref(), Some("http://localhost:11434"));

        // Custom base_url overrides.
        let cfg2 =
            Config::new("ollama-local", "qwen3-coder-32b").with_base_url("http://gpu-box:11434");
        let provider2: Box<dyn Provider> = Box::new(crate::mock::MockProvider::abc());
        let c2 = Client::new(cfg2, provider2);
        assert_eq!(c2.endpoint().as_deref(), Some("http://gpu-box:11434"));

        // Unknown provider + no custom url -> None.
        let cfg3 = Config::new("custom", "m");
        let provider3: Box<dyn Provider> = Box::new(crate::mock::MockProvider::abc());
        let c3 = Client::new(cfg3, provider3);
        assert_eq!(c3.endpoint(), None);
    }
}
