//! Lifecycle hooks — clean-room pattern (non-copyrightable): ~20 events, parallel
//! execution, most-restrictive-wins. Hooks can block, modify input, inject context,
//! short-circuit the loop, or replace tool output before the model sees it.

use async_trait::async_trait;
use serde_json::Value;

/// The set of lifecycle events (functional triggers, not copied identifiers).
#[derive(Debug, Clone)]
pub enum HookEvent {
    /// Before a tool call is executed (can block/deny).
    BeforeToolUse,
    /// After a tool call succeeds.
    AfterToolUse,
    /// After a tool call fails.
    AfterToolFailure,
    /// After a whole batch of tool calls resolves.
    AfterToolBatch,
    /// When the user submits a prompt.
    OnUserPromptSubmit,
    /// When a slash command expands to a prompt.
    OnUserPromptExpansion,
    /// When an assistant message completes for display.
    OnMessageDisplay,
    /// When the agent loop stops.
    OnStop,
    /// When a sub-agent starts.
    OnSubagentStart,
    /// When a sub-agent stops.
    OnSubagentStop,
    /// Before a compaction runs.
    BeforeCompact,
    /// When a permission request is surfaced.
    OnPermissionRequest,
    /// When a session starts.
    OnSessionStart,
    /// When a session ends.
    OnSessionEnd,
    /// When the agent emits a notification.
    OnNotification,
    /// On session maintenance/setup.
    OnSetup,
    /// When a teammate (sub-agent) goes idle.
    OnTeammateIdle,
    /// When a background task completes.
    OnTaskCompleted,
    /// When config changes.
    OnConfigChange,
    /// When a git worktree is created (for sub-agent isolation).
    OnWorktreeCreate,
    /// When a git worktree is removed.
    OnWorktreeRemove,
}

/// Context handed to a hook.
pub struct HookContext<'a> {
    /// The event being dispatched.
    pub event: &'a HookEvent,
    /// The tool name, if the event is tool-related.
    pub tool: Option<&'a str>,
    /// The tool args, if the event is tool-related.
    pub args: Option<&'a Value>,
    /// The conversation transcript so far.
    pub transcript: &'a [Value],
}

/// What a hook returns.
#[derive(Debug, Clone)]
pub enum HookOutcome {
    /// Allow the action to proceed.
    Allow,
    /// Block the action with a reason (most-restrictive-wins).
    Deny(String),
    /// Modify the tool input before execution.
    ModifyInput(Value),
    /// Inject additional context into the prompt.
    InjectContext(String),
    /// Short-circuit the loop.
    ShortCircuit,
    /// Replace the tool output before the model sees it.
    ReplaceOutput(Value),
}

/// A hook implementation (programmatic callback or filesystem shell command).
#[async_trait]
pub trait Hook: Send + Sync {
    /// Which events this hook cares about.
    fn events(&self) -> &[HookEvent];
    /// Run the hook. Runs in parallel with other hooks; most-restrictive-wins.
    async fn run(&self, ctx: HookContext<'_>) -> HookOutcome;
}

/// The hook registry + runner.
#[derive(Default)]
pub struct HookRegistry {
    // TODO(T16): Vec<Arc<dyn Hook>>, parallel execution via JoinSet, aggregate.
}

impl HookRegistry {
    /// Construct a new, empty registry.
    pub fn new() -> Self {
        Self::default()
    }
    // TODO(T16): register, dispatch(event, ctx) -> aggregated HookOutcome (most-restrictive-wins).

    /// T16: aggregate a set of hook outcomes (most-restrictive-wins).
    ///
    /// Priority: `Deny` (any) dominates immediately, then `ShortCircuit`, then
    /// `ModifyInput` (last wins), then `ReplaceOutput` (last wins), then
    /// `InjectContext` (concatenated), else `Allow`.
    pub fn aggregate_hooks(outcomes: Vec<HookOutcome>) -> HookOutcome {
        let mut last_modify: Option<HookOutcome> = None;
        let mut last_replace: Option<HookOutcome> = None;
        let mut injected: Vec<String> = Vec::new();
        for o in outcomes {
            match o {
                HookOutcome::Deny(_) => return o, // Deny dominates immediately
                HookOutcome::ShortCircuit => return HookOutcome::ShortCircuit,
                HookOutcome::ModifyInput(_) => last_modify = Some(o),
                HookOutcome::ReplaceOutput(_) => last_replace = Some(o),
                HookOutcome::InjectContext(s) => injected.push(s),
                HookOutcome::Allow => {}
            }
        }
        if let Some(m) = last_modify {
            return m;
        }
        if let Some(r) = last_replace {
            return r;
        }
        if !injected.is_empty() {
            return HookOutcome::InjectContext(injected.join("\n"));
        }
        HookOutcome::Allow
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T16: aggregate_hooks — most-restrictive-wins across mixed outcomes.
    #[test]
    fn aggregate_hooks_most_restrictive_wins() {
        // empty -> Allow
        assert!(matches!(
            HookRegistry::aggregate_hooks(vec![]),
            HookOutcome::Allow
        ));

        // one Deny -> Deny (dominates)
        assert!(matches!(
            HookRegistry::aggregate_hooks(vec![HookOutcome::Allow, HookOutcome::Deny("no".into())]),
            HookOutcome::Deny(_)
        ));

        // Deny + Allow -> Deny
        assert!(matches!(
            HookRegistry::aggregate_hooks(vec![HookOutcome::Deny("x".into()), HookOutcome::Allow]),
            HookOutcome::Deny(_)
        ));

        // ModifyInput + Allow -> ModifyInput (last wins)
        let m = HookRegistry::aggregate_hooks(vec![
            HookOutcome::Allow,
            HookOutcome::ModifyInput(serde_json::json!({"x": 1})),
        ]);
        assert!(matches!(m, HookOutcome::ModifyInput(_)));

        // ShortCircuit + Allow -> ShortCircuit
        assert!(matches!(
            HookRegistry::aggregate_hooks(vec![HookOutcome::Allow, HookOutcome::ShortCircuit]),
            HookOutcome::ShortCircuit
        ));

        // InjectContext (two) -> InjectContext (concatenated)
        let inj = HookRegistry::aggregate_hooks(vec![
            HookOutcome::InjectContext("a".into()),
            HookOutcome::InjectContext("b".into()),
        ]);
        match inj {
            HookOutcome::InjectContext(s) => assert_eq!(s, "a\nb"),
            other => panic!("expected InjectContext, got {other:?}"),
        }
    }
}
