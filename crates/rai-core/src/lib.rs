//! rai-core — the agent loop.
//!
//! Clean-room reimplementation of Claude Code's architectural patterns
//! (non-copyrightable: methods of operation, interfaces, data flows — 17 USC §102(b))
//! in fresh Rust. See docs/architecture/ for the spec this implements.
//!
//! Crate contents (to be implemented):
//! - `loop_`: the streaming agent loop (Stream<AgentEvent>), typed StopReason enum,
//!   backpressure, cancellation (CancellationToken).
//! - `tool`: the self-describing Tool trait (schema, is_concurrency_safe, execute),
//!   StreamingToolExecutor (tokio::JoinSet, parallel-safe / sequential-unsafe).
//! - `perm`: the permission resolution chain (hooks>deny>ask>mode>allow>callback),
//!   modes: Plan, Bypass(YOLO), Approval(HITL), Auto(classifier), Bubble(subagent).
//! - `hook`: ~20 lifecycle hooks (PreToolUse, PostToolUse, UserPromptSubmit, Stop,
//!   SubagentStart/Stop, PreCompact, SessionStart, ...), parallel, most-restrictive-wins.
//! - `subagent`: spawn primitive (fresh context, depth-limited, worktree isolation via git2,
//!   sidechain transcripts, summary-only return, background-by-default).
//! - `compact`: the compaction cascade (tool-result budget > snip > microcompact dual-path
//!   > context collapse > autocompact > reactive), cache-aware (CacheControl).
//! - `escalation`: adaptive loop selection (Agentless / PlanExecute / TreeSearch /
//!   DynamicWorkflow / Explore / ContextFolding) based on task classification.
//! - `state`: two-tier (bootstrap OnceLock + AppState rebuilt per frame for Ratatui).
//! - `workflow`: dynamic workflows via rustyscript (deno_core/V8), agent() as a custom op.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod compact;
pub mod diff_gate;
pub mod escalation;
pub mod harness;
pub mod harness_evolution;
pub mod hook;
pub mod itvf;
pub mod loop_;
pub mod perm;
pub mod state;
pub mod subagent;
pub mod test_hack_detector;
pub mod tool;
pub mod workflow;

// Re-exports
pub use harness::{
    adapt_harness_for_task, GlobalPattern, HarnessConfig, HarnessDimension, TaskDiagnostic,
};
pub use hook::{Hook, HookContext, HookEvent, HookOutcome};
pub use itvf::{next_state, run_itvf, ItvfConfig, ItvfEvent, ItvfResult, ItvfState};
pub use loop_::{AgentEvent, AgentLoop, StopReason};
pub use perm::{Permission, PermissionMode, PermissionResolver};
pub use state::{AppState, BootstrapState};
pub use subagent::{SubAgent, SubAgentHandle, SubAgentResult};
pub use tool::{StreamingToolExecutor, Tool, ToolContext, ToolResult};
