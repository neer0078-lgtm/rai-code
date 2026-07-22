//! A `MockProvider` for tests (behind the `mock` feature).
//!
//! Returns a fixed `ChatResponse` or a fixed sequence of `ChatChunk`s, with no
//! network. Used by rai-core's loop tests (T43+) to verify the loop wiring
//! without hitting a real LLM.

use crate::provider::{
    ChatChunk, ChatRequest, ChatResponse, ChatStream, Provider, ResolvedToolCall, StopReason, Usage,
};
use async_trait::async_trait;
use futures::stream;

/// A mock provider that returns a fixed response or a fixed chunk sequence.
pub struct MockProvider {
    /// The provider name (default "mock").
    pub name: String,
    /// The chunks to emit on `stream()` (in order). If empty, `stream()` emits
    /// a single `Finish(EndTurn)`.
    pub chunks: Vec<ChatChunk>,
    /// The response to return on `complete()`.
    pub complete_response: ChatResponse,
}

impl MockProvider {
    /// Construct a mock that emits the given chunks on stream + the given
    /// response on complete.
    pub fn new(name: impl Into<String>, chunks: Vec<ChatChunk>, complete: ChatResponse) -> Self {
        Self {
            name: name.into(),
            chunks,
            complete_response: complete,
        }
    }

    /// A simple mock that streams three text tokens then finishes with EndTurn,
    /// and completes with "abc".
    pub fn abc() -> Self {
        Self::new(
            "mock",
            vec![
                ChatChunk::Delta("a".into()),
                ChatChunk::Delta("b".into()),
                ChatChunk::Delta("c".into()),
                ChatChunk::Finish(StopReason::EndTurn),
            ],
            ChatResponse {
                content: "abc".into(),
                tool_calls: vec![],
                stop_reason: StopReason::EndTurn,
                usage: Usage::default(),
            },
        )
    }

    /// A mock that requests a single tool call (for testing tool execution).
    pub fn with_tool_call(name: &str, args: serde_json::Value) -> Self {
        let call = ResolvedToolCall {
            call_id: "call-1".into(),
            name: name.into(),
            args,
        };
        Self::new(
            "mock",
            vec![
                ChatChunk::ToolCall {
                    call_id: call.call_id.clone(),
                    name: call.name.clone(),
                    args_delta: call.args.to_string(),
                },
                ChatChunk::Finish(StopReason::ToolCalls),
            ],
            ChatResponse {
                content: String::new(),
                tool_calls: vec![call],
                stop_reason: StopReason::ToolCalls,
                usage: Usage::default(),
            },
        )
    }
}

#[async_trait]
impl Provider for MockProvider {
    fn name(&self) -> &str {
        &self.name
    }

    async fn stream(&self, _req: ChatRequest) -> anyhow::Result<ChatStream> {
        let chunks: Vec<ChatChunk> = if self.chunks.is_empty() {
            vec![ChatChunk::Finish(StopReason::EndTurn)]
        } else {
            self.chunks.clone()
        };
        Ok(Box::pin(stream::iter(chunks)))
    }

    async fn complete(&self, _req: ChatRequest) -> anyhow::Result<ChatResponse> {
        Ok(self.complete_response.clone())
    }
}
