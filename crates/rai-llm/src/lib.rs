//! rai-llm — multi-provider LLM client.
//!
//! Built on `genai` (26+ providers) with native protocols, Anthropic
//! `CacheControl` prompt caching, custom endpoints (OpenAI/Anthropic/Ollama-
//! compatible), and an optional LiteLLM HTTP proxy. The `Provider` trait
//! pattern is inspired by Goose (github.com/block/goose, Apache-2.0) — RAI
//! Code's own naming, no literal code.
//!
//! Per user decisions (turn-3): LiteLLM multi-provider with model-native
//! endpoint support + custom endpoints + local models; single default +
//! user-configured routing.
//!
//! Features:
//! - `mock` — exposes a `MockProvider` for tests (used by rai-core's loop tests).
#![warn(missing_docs)]

pub mod client;
#[cfg(feature = "mock")]
pub mod mock;
pub mod provider;

// Re-export the common types for convenience.
pub use provider::{
    with_cache_breakpoint, with_default_prefix_cache, CacheControl, CacheTarget, CacheTtl,
    ChatChunk, ChatMessage, ChatRequest, ChatResponse, ChatStream, Provider, ResolvedToolCall,
    StopReason, Usage,
};
