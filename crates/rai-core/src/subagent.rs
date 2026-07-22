//! Sub-agents — clean-room pattern (non-copyrightable): spawn primitive with fresh
//! context, inheritable tool set + permission mode (default Bubble), model override,
//! depth-limited, worktree isolation via git2, sidechain transcripts (intermediate
//! tool calls stay in the sub-agent; only the final summary returns), background-by-default.
//!
//! Per user decision turn-3: depth default ~3, worktree isolation, summary-only return,
//! A2A for remote. This is the "firewalling delegated context" pattern (Harness Effect)
//! that converts quadratic token growth to linear — FastContext: 60% token reduction.

use serde::{Deserialize, Serialize};

/// A sub-agent definition (mirrors the agent-definition file frontmatter).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgent {
    /// When to use this sub-agent (model-facing).
    pub description: String,
    /// The system prompt.
    pub prompt: String,
    /// Allowed tools (None = inherit all).
    pub tools: Option<Vec<String>>,
    /// Disallowed tools.
    pub disallowed_tools: Vec<String>,
    /// Model override ("inherit", "sonnet", "opus", "haiku", or a full id).
    pub model: String,
    /// Permission mode (default Bubble for sub-agents).
    pub permission_mode: crate::perm::PermissionMode,
    /// Max turns for this sub-agent.
    pub max_turns: u32,
    /// Run in background by default.
    pub background: bool,
    /// Isolation level (worktree / in-process / fork).
    pub isolation: Isolation,
    /// Depth from the root (enforced limit, default 3).
    pub depth: u32,
}

/// How a sub-agent is isolated.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Isolation {
    /// Fresh context, separate worktree (default — safest + parallelizable).
    #[default]
    Worktree,
    /// In-process, fresh context.
    InProcess,
    /// Fork: inherit parent's full context (drops input isolation, keeps tool-call isolation).
    Fork,
}

/// A handle to a running sub-agent.
pub struct SubAgentHandle {
    /// The sub-agent's unique id.
    pub id: String,
    /// The join handle to the spawned task producing the result.
    pub join: tokio::task::JoinHandle<SubAgentResult>,
}

/// What a sub-agent returns — a *summary*, not the full trajectory (token saving).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgentResult {
    /// The concise outcome summary (the only thing that enters the parent's context).
    pub summary: String,
    /// Artifact paths the sub-agent produced.
    pub artifacts: Vec<String>,
    /// Files the sub-agent touched.
    pub files_touched: Vec<String>,
    /// Tokens the sub-agent consumed (for accounting).
    pub tokens_used: usize,
}

impl SubAgentHandle {
    /// Whether the sub-agent is complete.
    pub fn is_complete(&self) -> bool {
        self.join.is_finished()
    }
    /// Await and extract the result.
    pub async fn outcome(self) -> anyhow::Result<SubAgentResult> {
        self.join.await.map_err(anyhow::Error::from)
    }
}

/// Spawn a sub-agent in an isolated git worktree (when Isolation::Worktree).
///
/// Uses git2 to create `../.rai/worktrees/<id>` on a dedicated branch, runs the
/// agent loop there, and returns only the summary + file list to the parent.
pub fn spawn_worktree_subagent(
    _repo: &git2::Repository,
    _def: SubAgent,
    worktree_root: &std::path::Path,
) -> anyhow::Result<SubAgentHandle> {
    let id = format!("sub-{}", uuid_like());
    let _wt_path = worktree_root.join(&id);
    // TODO: repo.worktree(&id, &wt_path, &worktree_opts)?; spawn tokio task with fresh state.
    let join = tokio::spawn(async move {
        SubAgentResult {
            summary: "(stub)".into(),
            artifacts: vec![],
            files_touched: vec![],
            tokens_used: 0,
        }
    });
    Ok(SubAgentHandle { id, join })
}

fn uuid_like() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{nanos:x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T09: Isolation defaults to Worktree and round-trips through serde as snake_case.
    #[test]
    fn isolation_default_and_serde() {
        assert_eq!(Isolation::default(), Isolation::Worktree);

        for (iso, expected) in [
            (Isolation::Worktree, "\"worktree\""),
            (Isolation::InProcess, "\"in_process\""),
            (Isolation::Fork, "\"fork\""),
        ] {
            let json = serde_json::to_string(&iso).expect("serialize");
            assert_eq!(json, expected, "serde tag mismatch for {iso:?}");
            let back: Isolation = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(back, iso, "round-trip failed for {iso:?}");
        }
    }
}
