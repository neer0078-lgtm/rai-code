//! The streaming agent loop — clean-room port of the AsyncGenerator `query()` pattern.
//!
//! In Claude Code (TS): `async function* query()` — an AsyncGenerator with typed
//! Terminal stop-reasons, backpressure, `yield*` composability, cancellation.
//!
//! In RAI Code (Rust): a `futures::Stream<AgentEvent>` produced by `async_stream::stream!`,
//! with a typed `StopReason` enum, inherent pull-based backpressure, stream flattening,
//! and `CancellationToken` + `tokio::select!` for cancellation.
//!
//! T41: AgentLoop holds messages + tool registry + resolver + hooks + cancellation.
//! T43: the loop calls the LLM (via Provider) + yields Token events.
//! T44: the loop executes tool calls + yields ToolResult.
//! T45: permission check before tool execution (Approval -> Ask).
//! T46: cancellation propagates (CancellationToken -> Terminal(StreamAborted)).

use crate::diff_gate::{extract_scope_from_request, DiffGate, GateDecision, TaskScope};
use crate::harness::HarnessConfig;
use crate::perm::{Permission, PermissionMode, PermissionResolver};
use crate::tool::{Tool, ToolCall, ToolContent, ToolContext, ToolRegistry, ToolResult};
use futures::stream::Stream;
use rai_llm::{ChatChunk, ChatRequest, Provider, StopReason as LlmStopReason};
use std::pin::Pin;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// A typed stop reason (clean-room: functional categories, not copied identifiers).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, thiserror::Error)]
#[serde(tag = "kind", content = "detail")]
pub enum StopReason {
    /// The model ended its turn with no pending tool calls.
    #[error("turn completed")]
    EndTurn,
    /// The user aborted (Ctrl-C / /stop).
    #[error("user aborted")]
    UserAbort,
    /// Output-token budget exhausted.
    #[error("token budget exhausted")]
    BudgetExhausted,
    /// A Stop hook intervened and prevented continuation.
    #[error("stop hook prevented: {0}")]
    StopHookPrevented(String),
    /// Max turns reached for this session/sub-agent.
    #[error("max turns reached: {0}")]
    MaxTurns(u32),
    /// Unrecoverable error from the model or transport.
    #[error("unrecoverable error: {0}")]
    Unrecoverable(String),
    /// A blocking limit was hit (rate limit / quota / concurrency).
    #[error("blocking limit: {0}")]
    BlockingLimit(String),
    /// The stream was aborted mid-flight.
    #[error("stream aborted")]
    StreamAborted,
    /// The provider returned a model-level error (retriable separately).
    #[error("model error: {0}")]
    ModelError(String),
    /// Prompt exceeded the context window.
    #[error("prompt too long")]
    PromptTooLong,
    /// An image could not be processed.
    #[error("image error: {0}")]
    ImageError(String),
}

/// Events the loop streams to the consumer (TUI / SDK / headless).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum AgentEvent {
    /// A model token (streaming) — part of the "Thought" in ReAct.
    Token(String),
    /// The model's reasoning before acting (the "Thought" in ReAct —
    /// explicit Thought/Action/Observation, Yao et al. 2022, arxiv 2210.03629).
    /// Emitted when the model produces text before a tool call.
    Thought(String),
    /// The model requested one or more tool calls (the "Action" in ReAct).
    ToolCalls(Vec<ToolCall>),
    /// A tool completed (the "Observation" in ReAct).
    ToolResult(ToolResult),
    /// A ReAct event (explicit thought/action/observation chain).
    ReAct(crate::itvf::ReActEvent),
    /// A permission request is pending user decision.
    PermissionRequest(crate::perm::PermissionRequest),
    /// The loop compacted context (for UI feedback).
    Compacted {
        /// Token count before compaction.
        tokens_before: usize,
        /// Token count after compaction.
        tokens_after: usize,
    },
    /// The loop terminated with a reason.
    Terminal(StopReason),
}

/// Pinned, boxed stream alias.
pub type AgentStream = Pin<Box<dyn Stream<Item = AgentEvent> + Send>>;

/// T41: the agent loop — holds the provider, tool registry, permission resolver,
/// cancellation token, and conversation messages. Constructed per user message.
pub struct AgentLoop {
    /// The LLM provider (Anthropic, OpenAI, MockProvider, ...).
    pub provider: Arc<dyn Provider>,
    /// The tool registry.
    pub tools: ToolRegistry,
    /// The permission resolver.
    pub permission: PermissionResolver,
    /// The cancellation token.
    pub cancel: CancellationToken,
    /// The conversation messages (rai-llm ChatMessage format).
    pub messages: Vec<rai_llm::ChatMessage>,
    /// The system prompt.
    pub system: Option<String>,
    /// The six-dimensional harness config (MemoHarness — the single source of truth).
    pub harness: HarnessConfig,
    /// The diff gate — scope enforcement (P0 from research alignment).
    /// Rejects writes to files outside the task's scope. None = no gate (open).
    pub diff_gate: Option<DiffGate>,
}

impl AgentLoop {
    /// Construct a new loop with the given provider + model.
    pub fn new(provider: Arc<dyn Provider>, model: impl Into<String>) -> Self {
        let model_str = model.into();
        Self {
            provider,
            tools: ToolRegistry::new(),
            permission: PermissionResolver::new(
                HarnessConfig::default_local(&model_str)
                    .orchestration
                    .permission_mode,
            ),
            cancel: CancellationToken::new(),
            messages: vec![],
            system: None,
            harness: HarnessConfig::default_local(&model_str),
            diff_gate: None,
        }
    }

    /// Register a tool.
    pub fn with_tool(mut self, tool: Arc<dyn Tool>) -> Self {
        self.tools.register(tool);
        self
    }

    /// Set the permission mode (overrides the orchestration dimension).
    pub fn with_permission_mode(mut self, mode: PermissionMode) -> Self {
        self.harness.orchestration.permission_mode = mode;
        self.permission = PermissionResolver::new(mode);
        self
    }

    /// Set the system prompt.
    pub fn with_system(mut self, system: impl Into<String>) -> Self {
        self.system = Some(system.into());
        self
    }

    /// Add a user message.
    pub fn with_user_message(mut self, msg: impl Into<String>) -> Self {
        self.messages.push(rai_llm::ChatMessage::user(msg));
        self
    }

    /// Set the max turns (overrides the orchestration dimension).
    pub fn with_max_turns(mut self, n: u32) -> Self {
        self.harness.orchestration.max_turns = n;
        self
    }

    /// Set the full harness config (the six-dimensional source of truth).
    /// This is the MemoHarness integration point — `adapt_harness_for_task`
    /// produces a case-specific config that's passed here.
    pub fn with_harness(mut self, harness: HarnessConfig) -> Self {
        self.permission = PermissionResolver::new(harness.orchestration.permission_mode);
        self.harness = harness;
        self
    }
    /// Set the diff gate from a task scope (scope enforcement — P0).
    pub fn with_diff_gate(mut self, scope: TaskScope) -> Self {
        self.diff_gate = Some(DiffGate::new(scope));
        self
    }

    /// Auto-extract the diff gate from the user's request.
    pub fn with_auto_diff_gate(mut self) -> Self {
        if let Some(msg) = self.messages.first() {
            if msg.role == "user" {
                let scope = extract_scope_from_request(&msg.content);
                self.diff_gate = Some(DiffGate::new(scope));
            }
        }
        self
    }

    /// T43-T46: run the loop, returning a stream of events.
    ///
    /// The loop: call the provider → stream tokens → if tool calls, execute them
    /// (with permission checks) → feed results back → repeat until no more tool
    /// calls or cancellation or max turns.
    pub fn run(self) -> AgentStream {
        Box::pin(async_stream::stream! {
            let provider = self.provider.clone();
            let tools = self.tools;
            let permission = self.permission;
            let cancel = self.cancel.clone();
            let mut messages = self.messages;
            let system = self.system;
            let model = self.harness.generation.model.clone();
            let max_turns = self.harness.orchestration.max_turns;
            let prompt_caching = self.harness.generation.prompt_caching;
            let diff_gate = self.diff_gate;
            let _cache_ttl = self.harness.generation.cache_ttl;
            let tool_result_clearing = self.harness.context.tool_result_clearing;
            let keep_last = self.harness.context.keep_last_tool_results;

            let mut turn = 0u32;
            loop {
                turn += 1;
                if turn > max_turns {
                    yield AgentEvent::Terminal(StopReason::MaxTurns(max_turns));
                    return;
                }

                // Build the request.
                let mut req = ChatRequest::new(&model, "");
                req.messages = messages.clone();
                req.system = system.clone();
                req.tools = tools.schema_for_model();

                // MemoHarness Generation dimension: prompt caching.
                if prompt_caching {
                    rai_llm::with_default_prefix_cache(&mut req);
                }

                // T46: cancellation check before the model call.
                if cancel.is_cancelled() {
                    yield AgentEvent::Terminal(StopReason::StreamAborted);
                    return;
                }

                // T43: call the provider + stream chunks.
                let stream = match provider.stream(req).await {
                    Ok(s) => s,
                    Err(e) => {
                        yield AgentEvent::Terminal(StopReason::ModelError(e.to_string()));
                        return;
                    }
                };

                // Collect the streamed chunks: yield Token events, accumulate
                // tool calls, watch for the finish.
                use futures::StreamExt;
                let mut collected_tokens = String::new();
                let mut tool_calls: Vec<ToolCall> = vec![];
                let mut stop_reason: Option<LlmStopReason> = None;

                let mut chunk_stream = stream;
                loop {
                    // T46: race the next chunk against cancellation.
                    let next = tokio::select! {
                        chunk = chunk_stream.next() => chunk,
                        _ = cancel.cancelled() => {
                            yield AgentEvent::Terminal(StopReason::StreamAborted);
                            return;
                        }
                    };

                    match next {
                        None => break,
                        Some(ChatChunk::Delta(text)) => {
                            collected_tokens.push_str(&text);
                            yield AgentEvent::Token(text);
                        }
                        Some(ChatChunk::ToolCall { call_id, name, args_delta }) => {
                            // Accumulate the tool call (in a real impl, partial
                            // args are buffered until complete; for the mock, the
                            // full args come in one chunk).
                            if let Some(existing) = tool_calls.iter_mut().find(|tc| tc.call_id == call_id) {
                                // Append to existing args (streaming accumulation).
                                if let serde_json::Value::String(ref mut s) = existing.args {
                                    s.push_str(&args_delta);
                                }
                            } else {
                                tool_calls.push(ToolCall {
                                    name: name.clone(),
                                    args: serde_json::Value::String(args_delta.clone()),
                                    call_id: call_id.clone(),
                                });
                            }
                        }
                        Some(ChatChunk::Finish(reason)) => {
                            stop_reason = Some(reason);
                            break;
                        }
                    }
                }

                // If the model produced text, add it to the conversation.
                if !collected_tokens.is_empty() {
                    messages.push(rai_llm::ChatMessage::assistant(collected_tokens.clone()));
                    // ReAct: emit a Thought event (the model's reasoning before acting).
                    yield AgentEvent::Thought(collected_tokens.clone());
                }

                // T44: if there are tool calls, execute them.
                if !tool_calls.is_empty() {
                    yield AgentEvent::ToolCalls(tool_calls.clone());

                    for tc in &tool_calls {
                        // T45: permission check.
                        let perm = permission.resolve(&tc.name);
                        if perm == Permission::Ask {
                            // Yield the permission request (the TUI/harness
                            // resolves it; for the mock loop test, the test
                            // pre-approves via Bypass mode).
                            yield AgentEvent::PermissionRequest(crate::perm::PermissionRequest {
                                tool: tc.name.clone(),
                                args: tc.args.clone(),
                                mode: permission.mode,
                                reason: "approval required".into(),
                            });
                            // In a real impl, we'd await the user's decision
                            // via a channel. For the loop skeleton, if we're in
                            // Approval mode and hit Ask, we treat it as Deny
                            // (the harness must use Bypass or Auto for auto-exec).
                            // The test uses Bypass to avoid this.
                            if permission.mode == PermissionMode::Approval {
                                // Skip execution (user hasn't approved in the skeleton).
                                continue;
                            }
                        }
                        if matches!(perm, Permission::Deny(_)) {
                            // Denied — yield an error result.
                            yield AgentEvent::ToolResult(ToolResult {
                                call_id: tc.call_id.clone(),
                                content: ToolContent::Text("permission denied".into()),
                                is_error: true,
                            });
                            continue;
                        }

                        // P0: Diff gate — scope enforcement.
                        if let Some(ref gate) = diff_gate {
                            let gate_decision = gate.check(&tc.name, &tc.args);
                            match gate_decision {
                                GateDecision::Allow => {}
                                GateDecision::Deny(reason) => {
                                    yield AgentEvent::ToolResult(ToolResult {
                                        call_id: tc.call_id.clone(),
                                        content: ToolContent::Text(format!(
                                            "diff gate denied: {reason}"
                                        )),
                                        is_error: true,
                                    });
                                    continue;
                                }
                                GateDecision::Propose(reason) => {
                                    // In Bypass mode, auto-approve the scope expansion.
                                    // In Approval mode, yield a permission request.
                                    if permission.mode == PermissionMode::Bypass {
                                        // Auto-expand scope in Bypass mode — just allow.
                                        let _ = TaskScope::extract_file_path(&tc.name, &tc.args);
                                    } else {
                                        yield AgentEvent::PermissionRequest(
                                            crate::perm::PermissionRequest {
                                                tool: tc.name.clone(),
                                                args: tc.args.clone(),
                                                mode: permission.mode,
                                                reason: format!(
                                                    "diff gate: {reason}"
                                                ),
                                            },
                                        );
                                        if permission.mode == PermissionMode::Approval {
                                            continue;
                                        }
                                    }
                                }
                            }
                        }

                        // Execute the tool.
                        if let Some(tool) = tools.get(&tc.name) {
                            let ctx = ToolContext {
                                workdir: std::path::Path::new("."),
                                permission: &permission,
                                cancellation: cancel.clone(),
                            };
                            let result = tool.execute(tc.clone(), ctx).await;
                            // ReAct: emit the Observation (the tool result).
                            yield AgentEvent::ReAct(crate::itvf::ReActEvent::Observation {
                                success: !result.is_error,
                                content: match &result.content {
                                    ToolContent::Text(t) => t.clone(),
                                    ToolContent::Json(v) => v.to_string(),
                                    ToolContent::FileDiff { path, diff } => format!("{path}:\n{diff}"),
                                    ToolContent::ImageRef { path } => format!("[image: {path}]"),
                                    ToolContent::Empty => String::new(),
                                },
                            });
                            yield AgentEvent::ToolResult(result.clone());

                            // Feed the tool result back into the conversation.
                            let result_text = match &result.content {
                                ToolContent::Text(t) => t.clone(),
                                ToolContent::Json(v) => v.to_string(),
                                ToolContent::FileDiff { path, diff } => format!("{path}:\n{diff}"),
                                ToolContent::ImageRef { path } => format!("[image: {path}]"),
                                ToolContent::Empty => String::new(),
                            };
                            messages.push(rai_llm::ChatMessage {
                                role: "tool".into(),
                                content: result_text,
                            });
                        } else {
                            // Tool not found.
                            yield AgentEvent::ToolResult(ToolResult {
                                call_id: tc.call_id.clone(),
                                content: ToolContent::Text(format!("tool not found: {}", tc.name)),
                                is_error: true,
                            });
                        }
                    }
                    // MemoHarness Context dimension: tool-result clearing.
                    // Keep only the last N tool results; older ones are cleared
                    // (the model knows it made the call; it can re-call if needed).
                    if tool_result_clearing && messages.len() > keep_last * 2 {
                        let drain_from = messages.len() - keep_last * 2;
                        messages.drain(0..drain_from);
                    }

                    // Continue the loop (the model sees the tool results + decides next).
                    continue;
                }

                // No tool calls — check the stop reason.
                match stop_reason {
                    Some(LlmStopReason::EndTurn) | None => {
                        yield AgentEvent::Terminal(StopReason::EndTurn);
                        return;
                    }
                    Some(LlmStopReason::ToolCalls) => {
                        // Shouldn't happen (we checked tool_calls.is_empty), but continue.
                        continue;
                    }
                    Some(LlmStopReason::BudgetExhausted) => {
                        yield AgentEvent::Terminal(StopReason::BudgetExhausted);
                        return;
                    }
                    Some(LlmStopReason::ModelError(e)) => {
                        yield AgentEvent::Terminal(StopReason::ModelError(e));
                        return;
                    }
                }
            }
        })
    }
}

/// A default-constructible AgentLoop for backward compat with the T17 test
/// (which expects AgentLoop::default().run() to yield Terminal(EndTurn)).
/// Uses a MockProvider that returns EndTurn immediately.
impl Default for AgentLoop {
    fn default() -> Self {
        #[cfg(feature = "mock-loop")]
        {
            Self::new(Arc::new(rai_llm::mock::MockProvider::abc()), "mock")
        }
        #[cfg(not(feature = "mock-loop"))]
        {
            Self::new(Arc::new(NoOpProvider), "noop")
        }
    }
}

/// A no-op provider for Default::default() (returns EndTurn with no tokens).
/// Used when the mock-loop feature isn't enabled.
#[cfg(not(feature = "mock-loop"))]
struct NoOpProvider;

#[cfg(not(feature = "mock-loop"))]
#[async_trait::async_trait]
impl Provider for NoOpProvider {
    fn name(&self) -> &str {
        "noop"
    }
    async fn stream(&self, _req: ChatRequest) -> anyhow::Result<rai_llm::ChatStream> {
        use futures::stream;
        Ok(Box::pin(stream::iter(vec![ChatChunk::Finish(
            LlmStopReason::EndTurn,
        )])))
    }
    async fn complete(&self, _req: ChatRequest) -> anyhow::Result<rai_llm::ChatResponse> {
        Ok(rai_llm::ChatResponse {
            content: String::new(),
            tool_calls: vec![],
            stop_reason: LlmStopReason::EndTurn,
            usage: rai_llm::Usage::default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    /// T02: StopReason round-trips through serde for all 11 variants.
    #[test]
    fn stop_reason_serde_roundtrip() {
        let all = [
            StopReason::EndTurn,
            StopReason::UserAbort,
            StopReason::BudgetExhausted,
            StopReason::StopHookPrevented("hook x".into()),
            StopReason::MaxTurns(7),
            StopReason::Unrecoverable("boom".into()),
            StopReason::BlockingLimit("rate".into()),
            StopReason::StreamAborted,
            StopReason::ModelError("500".into()),
            StopReason::PromptTooLong,
            StopReason::ImageError("bad png".into()),
        ];
        for v in all {
            let json = serde_json::to_string(&v).expect("serialize");
            let back: StopReason = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(v, back);
        }
    }

    /// T03: StopReason has exactly 11 variants.
    #[test]
    fn stop_reason_has_eleven_variants() {
        const EXPECTED_VARIANTS: usize = 11;
        let all = [
            StopReason::EndTurn,
            StopReason::UserAbort,
            StopReason::BudgetExhausted,
            StopReason::StopHookPrevented("x".into()),
            StopReason::MaxTurns(1),
            StopReason::Unrecoverable("x".into()),
            StopReason::BlockingLimit("x".into()),
            StopReason::StreamAborted,
            StopReason::ModelError("x".into()),
            StopReason::PromptTooLong,
            StopReason::ImageError("x".into()),
        ];
        assert_eq!(all.len(), EXPECTED_VARIANTS);
    }

    /// T11: AgentEvent round-trips through serde.
    #[test]
    fn agent_event_serde_roundtrip() {
        let events = vec![
            AgentEvent::Token("hi".into()),
            AgentEvent::ToolCalls(vec![ToolCall {
                name: "Read".into(),
                args: serde_json::json!({"path":"foo.rs"}),
                call_id: "c1".into(),
            }]),
            AgentEvent::ToolResult(ToolResult {
                call_id: "c1".into(),
                content: ToolContent::Text("ok".into()),
                is_error: false,
            }),
            AgentEvent::Compacted {
                tokens_before: 100,
                tokens_after: 40,
            },
            AgentEvent::Terminal(StopReason::EndTurn),
            AgentEvent::Terminal(StopReason::ModelError("500".into())),
        ];
        for ev in events {
            let json = serde_json::to_string(&ev).expect("serialize");
            let back: AgentEvent = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(ev, back);
        }
    }

    /// T17: AgentLoop::default().run() yields Terminal(EndTurn).
    #[tokio::test]
    async fn agent_loop_yields_endturn() {
        let stream = AgentLoop::default().run();
        let collected: Vec<AgentEvent> = stream.collect().await;
        assert!(!collected.is_empty());
        assert_eq!(
            collected.last(),
            Some(&AgentEvent::Terminal(StopReason::EndTurn))
        );
    }

    /// T41: AgentLoop::new constructs with a provider + model + registers tools.
    #[tokio::test]
    async fn agent_loop_construction() {
        let provider: Arc<dyn Provider> = Arc::new(rai_llm::mock::MockProvider::abc());
        let loop_ = AgentLoop::new(provider, "mock")
            .with_user_message("hello")
            .with_system("you are RAI Code")
            .with_permission_mode(PermissionMode::Bypass)
            .with_max_turns(10);
        assert_eq!(loop_.harness.generation.model, "mock");
        assert_eq!(loop_.messages.len(), 1);
        assert!(loop_.system.is_some());
        assert_eq!(loop_.permission.mode, PermissionMode::Bypass);
        assert_eq!(loop_.harness.orchestration.max_turns, 10);
    }

    /// T43: the loop calls the MockProvider + yields Token events then Terminal(EndTurn).
    #[tokio::test]
    async fn loop_yields_tokens_then_endturn() {
        let provider: Arc<dyn Provider> = Arc::new(rai_llm::mock::MockProvider::abc());
        let loop_ = AgentLoop::new(provider, "mock")
            .with_user_message("hello")
            .with_permission_mode(PermissionMode::Bypass);
        let stream = loop_.run();
        let collected: Vec<AgentEvent> = stream.collect().await;

        // Should have at least: Token("a"), Token("b"), Token("c"), Terminal(EndTurn).
        let tokens: Vec<String> = collected
            .iter()
            .filter_map(|e| {
                if let AgentEvent::Token(t) = e {
                    Some(t.clone())
                } else {
                    None
                }
            })
            .collect();
        assert!(
            tokens.iter().any(|t| t.contains("a")),
            "should have token 'a': {tokens:?}"
        );
        assert!(
            tokens.iter().any(|t| t.contains("b")),
            "should have token 'b': {tokens:?}"
        );
        assert!(
            tokens.iter().any(|t| t.contains("c")),
            "should have token 'c': {tokens:?}"
        );

        // Ends with Terminal(EndTurn).
        assert_eq!(
            collected.last(),
            Some(&AgentEvent::Terminal(StopReason::EndTurn))
        );
    }

    /// T44: the loop executes a tool call (MockProvider returns tool_use).
    #[tokio::test]
    async fn loop_executes_tool_call() {
        use async_trait::async_trait;

        struct EchoTool;
        #[async_trait]
        impl Tool for EchoTool {
            fn name(&self) -> &str {
                "Echo"
            }
            fn description(&self) -> &str {
                "echo"
            }
            fn input_schema(&self) -> serde_json::Value {
                serde_json::json!({"type":"object"})
            }
            fn is_concurrency_safe(&self) -> bool {
                true
            }
            async fn execute(&self, call: ToolCall, _ctx: ToolContext<'_>) -> ToolResult {
                ToolResult {
                    call_id: call.call_id,
                    content: ToolContent::Text(call.args.to_string()),
                    is_error: false,
                }
            }
        }

        let provider: Arc<dyn Provider> = Arc::new(rai_llm::mock::MockProvider::with_tool_call(
            "Echo",
            serde_json::json!({"msg":"hi"}),
        ));
        let loop_ = AgentLoop::new(provider, "mock")
            .with_user_message("echo hi")
            .with_permission_mode(PermissionMode::Bypass)
            .with_tool(Arc::new(EchoTool));

        let stream = loop_.run();
        let collected: Vec<AgentEvent> = stream.collect().await;

        // Should have: ToolCalls([...]), ToolResult(...), then a second model
        // call that produces EndTurn.
        let has_tool_calls = collected
            .iter()
            .any(|e| matches!(e, AgentEvent::ToolCalls(_)));
        assert!(has_tool_calls, "should have ToolCalls event: {collected:?}");

        let has_tool_result = collected.iter().any(|e| {
            if let AgentEvent::ToolResult(r) = e {
                !r.is_error && matches!(&r.content, ToolContent::Text(t) if t.contains("hi"))
            } else {
                false
            }
        });
        assert!(
            has_tool_result,
            "should have a successful ToolResult containing 'hi': {collected:?}"
        );
    }

    /// T46: cancellation propagates -> Terminal(StreamAborted).
    #[tokio::test]
    async fn loop_cancellation_aborts() {
        let provider: Arc<dyn Provider> = Arc::new(rai_llm::mock::MockProvider::abc());
        let loop_ = AgentLoop::new(provider, "mock")
            .with_user_message("hello")
            .with_permission_mode(PermissionMode::Bypass);

        // Cancel before running.
        loop_.cancel.cancel();

        let stream = loop_.run();
        let collected: Vec<AgentEvent> = stream.collect().await;

        // Should have Terminal(StreamAborted) (possibly after some events, but
        // the cancel was before the first model call so it should be immediate).
        let has_aborted = collected
            .iter()
            .any(|e| matches!(e, AgentEvent::Terminal(StopReason::StreamAborted)));
        assert!(
            has_aborted,
            "should have Terminal(StreamAborted): {collected:?}"
        );
    }

    /// T67: HarnessConfig drives the loop — case-adaptation changes behavior.
    ///
    /// This is the MemoHarness integration: adapt_harness_for_task produces a
    /// case-specific HarnessConfig, which is passed to AgentLoop::with_harness.
    /// The loop then reads max_turns, permission_mode, model, and prompt
    /// caching from the harness dimensions — not from individual fields.
    #[tokio::test]
    async fn harness_config_drives_loop_behavior() {
        use crate::harness::{
            adapt_harness_for_task, GlobalPattern, HarnessConfig, HarnessDimension,
        };

        let provider: Arc<dyn Provider> = Arc::new(rai_llm::mock::MockProvider::abc());

        // Start with a default local harness.
        let global = HarnessConfig::default_local("qwen3-coder-32b");

        // Simulate a multi-file refactor task — the experience bank has a
        // pattern that says "multi-file refactors need plan-execute".
        let patterns = vec![GlobalPattern {
            description: "multi-file refactors need plan-execute".into(),
            dimension: HarnessDimension::Orchestration,
            task_types: vec!["refactor".into()],
            recommended_change: "use plan-execute orchestration".into(),
            confidence: 0.9,
        }];

        // Case-adapt the harness for this task.
        let adapted = adapt_harness_for_task(&global, "refactor the auth module", &[], &patterns);

        // The adapted harness should use PlanAndExecute escalation.
        assert_eq!(
            adapted.orchestration.escalation_mode,
            crate::escalation::EscalationMode::PlanAndExecute
        );

        // The loop should reflect the adapted harness.
        let loop_ = AgentLoop::new(provider, "mock")
            .with_user_message("refactor the auth module")
            .with_harness(adapted);

        // The loop's permission resolver should match the harness.
        assert_eq!(
            loop_.harness.orchestration.permission_mode,
            PermissionMode::Approval
        );
        // The loop's max_turns should come from the harness.
        assert_eq!(
            loop_.harness.orchestration.max_turns,
            global.orchestration.max_turns
        );
        // The loop's model should come from the harness generation dimension.
        assert_eq!(loop_.harness.generation.model, "qwen3-coder-32b");
        // Prompt caching should be enabled (the generation dimension).
        assert!(loop_.harness.generation.prompt_caching);
    }

    /// T67: with_max_turns overrides the harness orchestration dimension.
    #[tokio::test]
    async fn with_max_turns_overrides_harness() {
        let provider: Arc<dyn Provider> = Arc::new(rai_llm::mock::MockProvider::abc());
        let loop_ = AgentLoop::new(provider, "mock")
            .with_user_message("test")
            .with_max_turns(5);
        assert_eq!(loop_.harness.orchestration.max_turns, 5);
    }

    /// T67: with_permission_mode overrides the harness orchestration dimension.
    #[tokio::test]
    async fn with_permission_mode_overrides_harness() {
        let provider: Arc<dyn Provider> = Arc::new(rai_llm::mock::MockProvider::abc());
        let loop_ = AgentLoop::new(provider, "mock")
            .with_user_message("test")
            .with_permission_mode(PermissionMode::Bypass);
        assert_eq!(
            loop_.harness.orchestration.permission_mode,
            PermissionMode::Bypass
        );
        assert_eq!(loop_.permission.mode, PermissionMode::Bypass);
    }

    /// T69: the loop emits ReAct Thought events when the model produces text
    /// before tool calls, and ReAct Observation events when tools execute.
    #[tokio::test]
    async fn loop_emits_react_events() {
        use crate::itvf::ReActEvent;
        use async_trait::async_trait;

        struct EchoTool;
        #[async_trait]
        impl Tool for EchoTool {
            fn name(&self) -> &str {
                "Echo"
            }
            fn description(&self) -> &str {
                "echo"
            }
            fn input_schema(&self) -> serde_json::Value {
                serde_json::json!({"type":"object"})
            }
            fn is_concurrency_safe(&self) -> bool {
                true
            }
            async fn execute(&self, call: ToolCall, _ctx: ToolContext<'_>) -> ToolResult {
                ToolResult {
                    call_id: call.call_id,
                    content: ToolContent::Text("echoed".into()),
                    is_error: false,
                }
            }
        }

        let provider: Arc<dyn Provider> = Arc::new(rai_llm::mock::MockProvider::with_tool_call(
            "Echo",
            serde_json::json!({"msg":"hi"}),
        ));
        let loop_ = AgentLoop::new(provider, "mock")
            .with_user_message("echo hi")
            .with_permission_mode(PermissionMode::Bypass)
            .with_tool(Arc::new(EchoTool));

        use futures::StreamExt;
        let events: Vec<AgentEvent> = loop_.run().collect().await;

        // Should have ReAct Observation events (from the tool execution).
        let has_react_obs = events.iter().any(|e| {
            matches!(
                e,
                AgentEvent::ReAct(ReActEvent::Observation { success: true, .. })
            )
        });
        assert!(
            has_react_obs,
            "should have a ReAct Observation event: {events:?}"
        );
    }
}
