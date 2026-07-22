//! The Provider trait + the request/response/chunk types + per-provider impls.
//!
//! Clean-room pattern (non-copyrightable): an async `Provider` trait with a
//! streaming `stream()` and a one-shot `complete()`, plus per-provider impls.
//! The trait shape is inspired by Goose's `Provider` trait
//! (github.com/block/goose, Apache-2.0) — RAI Code's own naming, no literal code.
//!
//! Per user decisions (turn-3): multi-provider with model-native endpoint support
//! + custom endpoints + local models; single default + user-configured routing.

use async_trait::async_trait;
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::pin::Pin;

/// A pinned, boxed stream of chat chunks (the streaming response).
pub type ChatStream = Pin<Box<dyn Stream<Item = ChatChunk> + Send>>;

/// A request to a chat completion endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatRequest {
    /// The model id (e.g. "claude-sonnet-5", "gpt-5.5", "qwen3-coder-32b").
    pub model: String,
    /// The conversation messages (role + content).
    pub messages: Vec<ChatMessage>,
    /// An optional system prompt.
    pub system: Option<String>,
    /// Optional tool definitions (JSON Schema), for function calling.
    pub tools: Vec<Value>,
    /// The maximum output tokens (None = provider default).
    pub max_tokens: Option<u32>,
    /// The temperature (None = provider default).
    pub temperature: Option<f32>,
    /// Prompt-caching breakpoints (Anthropic-style `cache_control`).
    pub cache_control: Vec<CacheControl>,
}

impl ChatRequest {
    /// Construct a minimal request with a model + a single user message.
    pub fn new(model: impl Into<String>, user: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            messages: vec![ChatMessage::user(user)],
            system: None,
            tools: vec![],
            max_tokens: None,
            temperature: None,
            cache_control: vec![],
        }
    }
}

/// A single chat message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatMessage {
    /// The role: "user", "assistant", "system", "tool".
    pub role: String,
    /// The content (text; structured content is a future extension).
    pub content: String,
}

impl ChatMessage {
    /// Construct a user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
        }
    }
    /// Construct an assistant message.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".into(),
            content: content.into(),
        }
    }
    /// Construct a system message.
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: content.into(),
        }
    }
}

/// A streaming chunk (T19: serde round-trip).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", content = "data")]
pub enum ChatChunk {
    /// A partial text delta (a few tokens).
    Delta(String),
    /// A tool-call chunk (function name + partial args).
    ToolCall {
        /// The tool call id (for streaming correlation).
        call_id: String,
        /// The tool/function name.
        name: String,
        /// A partial arguments JSON string.
        args_delta: String,
    },
    /// The stream finished with a stop reason.
    Finish(StopReason),
}

/// Why the stream stopped (mirrors rai-core's StopReason but owned here to keep
/// rai-llm independent of rai-core — the loop maps between them).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, thiserror::Error)]
#[serde(tag = "kind", content = "detail")]
pub enum StopReason {
    /// The model ended its turn with no pending tool calls.
    #[error("turn completed")]
    EndTurn,
    /// The model requested tool calls (stream paused pending execution).
    #[error("tool calls requested")]
    ToolCalls,
    /// Output-token budget exhausted.
    #[error("token budget exhausted")]
    BudgetExhausted,
    /// The provider returned a model-level error.
    #[error("model error: {0}")]
    ModelError(String),
}

/// A non-streaming completion response (T19: serde round-trip).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatResponse {
    /// The full text content.
    pub content: String,
    /// The tool calls the model requested (if any).
    pub tool_calls: Vec<ResolvedToolCall>,
    /// Why the response stopped.
    pub stop_reason: StopReason,
    /// Token usage accounting.
    pub usage: Usage,
}

/// A fully-resolved tool call (the args are complete, not partial).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResolvedToolCall {
    /// The tool call id.
    pub call_id: String,
    /// The tool/function name.
    pub name: String,
    /// The complete arguments JSON.
    pub args: Value,
}

/// Token-usage accounting.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Usage {
    /// Input (prompt) tokens.
    pub input_tokens: u64,
    /// Output (completion) tokens.
    pub output_tokens: u64,
    /// Cache-read input tokens (Anthropic prompt caching).
    pub cache_read_tokens: u64,
    /// Cache-write input tokens (Anthropic prompt caching).
    pub cache_write_tokens: u64,
}

/// A prompt-caching breakpoint (Anthropic `cache_control`).
///
/// Mark stable content with one of these so the provider can cache the prefix
/// and discount cache-hit input tokens (Anthropic: ~90% off on hits).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CacheControl {
    /// Where the breakpoint applies (an index into the messages or tools).
    pub at: CacheTarget,
    /// The cache time-to-live.
    pub ttl: CacheTtl,
}

/// Where a cache breakpoint applies.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CacheTarget {
    /// Cache through the end of the tools list.
    Tools,
    /// Cache through the end of the system prompt.
    System,
    /// Cache through message at the given index.
    Message(usize),
}

/// The cache time-to-live.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CacheTtl {
    /// 5-minute cache (the default; refreshed on each hit; best for active sessions).
    #[default]
    Ephemeral5m,
    /// 1-hour cache (better for long-gap/background; higher write premium).
    Ephemeral1h,
}

impl CacheControl {
    /// Construct a cache breakpoint at the tools list (5-min TTL by default).
    pub fn at_tools() -> Self {
        Self {
            at: CacheTarget::Tools,
            ttl: CacheTtl::default(),
        }
    }
    /// Construct a cache breakpoint at the system prompt (5-min TTL by default).
    pub fn at_system() -> Self {
        Self {
            at: CacheTarget::System,
            ttl: CacheTtl::default(),
        }
    }
    /// Construct a cache breakpoint at message `idx` (5-min TTL by default).
    pub fn at_message(idx: usize) -> Self {
        Self {
            at: CacheTarget::Message(idx),
            ttl: CacheTtl::default(),
        }
    }
    /// Set the TTL (builder style).
    pub fn with_ttl(mut self, ttl: CacheTtl) -> Self {
        self.ttl = ttl;
        self
    }
}

/// T21: insert a cache breakpoint into a `ChatRequest` at a given target.
///
/// Pure — no network. This is the helper the loop calls to mark the stable
/// prefix (tools → system → AGENTS.md → ... ) so the provider can cache it and
/// discount cache-hit input tokens (Anthropic: ~90% off on hits).
///
/// The golden rule (per the token-saving research): place the breakpoint at the
/// end of the stable prefix; all volatile content (user message, graph-recall,
/// code hunks, timestamps) goes AFTER the breakpoint.
pub fn with_cache_breakpoint(req: &mut ChatRequest, cc: CacheControl) {
    req.cache_control.push(cc);
}

/// T21: insert a cache breakpoint at the end of the stable prefix — the
/// conventional "cache through tools + system" breakpoint used on every turn.
///
/// This is the high-ROI default: tools + system prompt + AGENTS.md are the same
/// across turns, so caching them gives ~78-81% input-cost reduction (per the
/// prompt-caching research, arxiv 2601.06007).
pub fn with_default_prefix_cache(req: &mut ChatRequest) {
    with_cache_breakpoint(req, CacheControl::at_tools());
    if req.system.is_some() {
        with_cache_breakpoint(req, CacheControl::at_system());
    }
}

/// The async Provider contract.
///
/// Each provider implements this. `stream()` returns a `Stream<ChatChunk>` for
/// pull-based backpressure; `complete()` is the one-shot convenience. `name()`
/// is the provider key (e.g. "anthropic", "openai", "ollama-local").
#[async_trait]
pub trait Provider: Send + Sync {
    /// The provider key (e.g. "anthropic", "openai", "ollama-local").
    fn name(&self) -> &str;
    /// Stream a chat completion as a sequence of chunks.
    async fn stream(&self, req: ChatRequest) -> anyhow::Result<ChatStream>;
    /// One-shot chat completion (collects the stream internally).
    async fn complete(&self, req: ChatRequest) -> anyhow::Result<ChatResponse>;
}

/// An Anthropic Messages-API provider (stub until T20 wires genai).
///
/// Anthropic's native format: Messages API with `cache_control` breakpoints,
/// token-efficient tool use beta header, streaming SSE.
pub struct AnthropicProvider {
    /// The API key (or None for ANTHROPIC_API_KEY env).
    pub api_key: Option<String>,
    /// An optional custom base URL (for LiteLLM/gateway redirects).
    pub base_url: Option<String>,
}

impl AnthropicProvider {
    /// Construct an Anthropic provider with optional key + base URL.
    pub fn new(api_key: Option<String>, base_url: Option<String>) -> Self {
        Self { api_key, base_url }
    }
}

#[async_trait]
impl Provider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    async fn stream(&self, _req: ChatRequest) -> anyhow::Result<ChatStream> {
        // TODO(T20): wire genai::Client with cache_control + token-efficient tool use.
        Err(anyhow::anyhow!(
            "AnthropicProvider::stream not yet implemented (T20)"
        ))
    }

    async fn complete(&self, _req: ChatRequest) -> anyhow::Result<ChatResponse> {
        // TODO(T20): collect the stream into a ChatResponse.
        Err(anyhow::anyhow!(
            "AnthropicProvider::complete not yet implemented (T20)"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T18: the Provider trait compiles with a concrete provider; the
    /// AnthropicProvider stub constructs and reports its name.
    #[test]
    fn provider_trait_compiles_with_anthropic_stub() {
        let p = AnthropicProvider::new(None, None);
        assert_eq!(p.name(), "anthropic");
        assert_eq!(Provider::name(&p), "anthropic"); // via the trait
                                                     // Config fields are settable.
        let p2 = AnthropicProvider::new(Some("k".into()), Some("https://gw".into()));
        assert_eq!(p2.api_key.as_deref(), Some("k"));
        assert_eq!(p2.base_url.as_deref(), Some("https://gw"));
    }

    /// T18: ChatRequest::new builds a minimal request with a user message.
    #[test]
    fn chat_request_new_is_minimal() {
        let r = ChatRequest::new("claude-sonnet-5", "hello");
        assert_eq!(r.model, "claude-sonnet-5");
        assert_eq!(r.messages.len(), 1);
        assert_eq!(r.messages[0].role, "user");
        assert_eq!(r.messages[0].content, "hello");
        assert!(r.system.is_none());
        assert!(r.tools.is_empty());
        assert!(r.cache_control.is_empty());
    }

    /// T19: ChatChunk round-trips through serde for all variants.
    #[test]
    fn chat_chunk_serde_roundtrip() {
        let chunks = vec![
            ChatChunk::Delta("hi".into()),
            ChatChunk::ToolCall {
                call_id: "c1".into(),
                name: "Read".into(),
                args_delta: "{\"path\":".into(),
            },
            ChatChunk::Finish(StopReason::EndTurn),
            ChatChunk::Finish(StopReason::ToolCalls),
            ChatChunk::Finish(StopReason::ModelError("500".into())),
        ];
        for c in chunks {
            let json = serde_json::to_string(&c).expect("serialize");
            let back: ChatChunk = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(c, back, "round-trip failed for {c:?} (json: {json})");
        }
    }

    /// T19: ChatResponse round-trips through serde, including tool calls + usage.
    #[test]
    fn chat_response_serde_roundtrip() {
        let r = ChatResponse {
            content: "done".into(),
            tool_calls: vec![ResolvedToolCall {
                call_id: "c1".into(),
                name: "Edit".into(),
                args: serde_json::json!({"path": "foo.rs", "content": "x"}),
            }],
            stop_reason: StopReason::ToolCalls,
            usage: Usage {
                input_tokens: 100,
                output_tokens: 50,
                cache_read_tokens: 80,
                cache_write_tokens: 20,
            },
        };
        let json = serde_json::to_string(&r).expect("serialize");
        let back: ChatResponse = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(r, back);
        assert_eq!(back.usage.cache_read_tokens, 80);
        assert_eq!(back.tool_calls.len(), 1);
        assert_eq!(back.tool_calls[0].name, "Edit");
    }

    /// T19: CacheControl + CacheTarget + CacheTtl round-trip through serde.
    #[test]
    fn cache_control_serde_roundtrip() {
        let ccs = vec![
            CacheControl {
                at: CacheTarget::Tools,
                ttl: CacheTtl::Ephemeral5m,
            },
            CacheControl {
                at: CacheTarget::System,
                ttl: CacheTtl::Ephemeral1h,
            },
            CacheControl {
                at: CacheTarget::Message(3),
                ttl: CacheTtl::Ephemeral5m,
            },
        ];
        for c in ccs {
            let json = serde_json::to_string(&c).expect("serialize");
            let back: CacheControl = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(c, back, "round-trip failed for {c:?} (json: {json})");
        }
        // CacheTtl default is the 5-min ephemeral.
        assert_eq!(CacheTtl::default(), CacheTtl::Ephemeral5m);
    }

    /// T21: with_cache_breakpoint inserts a marker at the right target;
    /// with_default_prefix_cache adds tools + system breakpoints; pure (no network).
    #[test]
    fn cache_breakpoint_helpers() {
        // with_cache_breakpoint adds to req.cache_control in order.
        let mut req = ChatRequest::new("claude-sonnet-5", "hi");
        assert!(req.cache_control.is_empty());
        with_cache_breakpoint(&mut req, CacheControl::at_message(2));
        assert_eq!(req.cache_control.len(), 1);
        assert_eq!(req.cache_control[0].at, CacheTarget::Message(2));
        assert_eq!(req.cache_control[0].ttl, CacheTtl::Ephemeral5m);

        // with_ttl builder.
        let cc = CacheControl::at_tools().with_ttl(CacheTtl::Ephemeral1h);
        assert_eq!(cc.ttl, CacheTtl::Ephemeral1h);
        assert_eq!(cc.at, CacheTarget::Tools);

        // with_default_prefix_cache: tools breakpoint always; system breakpoint
        // only if a system prompt is set.
        let mut req_no_sys = ChatRequest::new("claude-sonnet-5", "hi");
        with_default_prefix_cache(&mut req_no_sys);
        assert_eq!(req_no_sys.cache_control.len(), 1); // tools only
        assert_eq!(req_no_sys.cache_control[0].at, CacheTarget::Tools);

        let mut req_with_sys = ChatRequest::new("claude-sonnet-5", "hi");
        req_with_sys.system = Some("you are RAI Code".into());
        with_default_prefix_cache(&mut req_with_sys);
        assert_eq!(req_with_sys.cache_control.len(), 2); // tools + system
        assert_eq!(req_with_sys.cache_control[0].at, CacheTarget::Tools);
        assert_eq!(req_with_sys.cache_control[1].at, CacheTarget::System);
    }
}
