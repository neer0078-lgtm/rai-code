//! Dynamic workflows — clean-room pattern (non-copyrightable): the model writes
//! JavaScript as ordinary tokens mid-turn; a sandboxed runtime (rustyscript / deno_core)
//! executes it in isolation with an `agent()` primitive, `parallel()`, `pipeline()`,
//! `phase()`, `log()`, `workflow()` globals. Journaling for resume; schema enforcement
//! at the tool boundary. Constraints: max concurrent agents, max total agents, no
//! mid-run user input, no direct fs/shell from script (agents do I/O), determinism
//! (Math.random/Date.now throw) for journaling.
//!
//! The `agent()` primitive is a custom Deno op: JS -> Rust -> tokio::spawn subagent.
//! This is the escalation mode for PARALLELIZABLE tasks (research: proven for that case).

// Implementation deferred to Phase 3. Stub for now.
