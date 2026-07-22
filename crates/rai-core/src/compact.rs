//! The compaction cascade — clean-room pattern (non-copyrightable): a progressive
//! compression pipeline, cheapest first, heaviest last, cache-aware.
//!
//! Levels:
//!  1. Tool-result budget — persist full output to disk, keep a 2KB preview.
//!  2. History snip — remove "zombie" messages (stale tool results, orphaned markers).
//!  3. Microcompact — clear old tool results. Dual-path:
//!       - Cold cache (cache expired): edit message content directly.
//!       - Warm cache: use the Anthropic cache_edits API to delete tool-result refs
//!         server-side without touching local messages (preserves the 100K+ cached prefix).
//!  4. Context collapse — projection-based folding (like a DB view).
//!  5. Autocompact — fork a child agent for a full LLM summary (irreversible, cache-invalidate).
//!  6. Reactive compact — emergency compaction on a 413 / prompt-too-long error.
//!
//! Per expert-improvement: compaction co-designed with caching — the checkpoint
//! becomes the new cacheable prefix (preserves cache stability; Harness Effect paper).
//! Governance directives (Hindsight) are NEVER summarized away — they live in the
//! cached prefix or as persistent state, solving the 30-59% governance-decay violation rate.

use serde::{Deserialize, Serialize};

/// Compaction level (ordered by cost).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactLevel {
    /// L1: persist full tool output to disk, keep a 2KB preview.
    ToolResultBudget,
    /// L2: remove "zombie" messages (stale tool results, orphaned markers).
    HistorySnip,
    /// L3: clear old tool results (dual-path: cold-cache direct / warm-cache cache_edits).
    Microcompact,
    /// L4: projection-based folding (like a DB view).
    ContextCollapse,
    /// L5: fork a child agent for a full LLM summary (irreversible, cache-invalidate).
    Autocompact,
    /// L6: emergency compaction on a 413 / prompt-too-long error.
    Reactive,
}

/// Triggers and thresholds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactConfig {
    /// Start tool-result budget at this many chars per result.
    pub tool_result_budget_chars: usize,
    /// Utilization fraction that triggers ContextCollapse (e.g. 0.90).
    pub collapse_threshold: f64,
    /// Utilization fraction that triggers Autocompact (e.g. 0.87).
    pub autocompact_threshold: f64,
    /// Circuit breaker: stop autocompacting after N consecutive failures.
    pub autocompact_circuit_breaker: u32,
    /// Keep this many recently-read files post-compact (≤5K tokens each, 50K budget).
    pub post_compact_keep_files: usize,
    /// Keep the last N user messages verbatim (intent tracking).
    pub post_compact_keep_user_msgs: usize,
}

impl Default for CompactConfig {
    fn default() -> Self {
        Self {
            tool_result_budget_chars: 50_000,
            collapse_threshold: 0.90,
            autocompact_threshold: 0.87,
            autocompact_circuit_breaker: 3,
            post_compact_keep_files: 5,
            post_compact_keep_user_msgs: 10,
        }
    }
}

/// The autocompact summary structure (9 conceptual sections — functional, not copied).
/// Section 6 (user messages) is preserved VERBATIM — critical for intent tracking.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AutoCompactSummary {
    /// Section 1: the primary request and intent.
    pub primary_request_and_intent: String,
    /// Section 2: key technical concepts.
    pub key_technical_concepts: Vec<String>,
    /// Section 3: files and code sections (with snippets).
    pub files_and_code_sections: Vec<String>,
    /// Section 4: errors and fixes.
    pub errors_and_fixes: Vec<String>,
    /// Section 5: problem-solving narrative.
    pub problem_solving: String,
    /// Section 6: VERBATIM user messages (critical for intent tracking).
    pub all_user_messages_verbatim: Vec<String>,
    /// Section 7: pending tasks.
    pub pending_tasks: Vec<String>,
    /// Section 8: current work.
    pub current_work: String,
    /// Section 9: optional next step.
    pub optional_next_step: String,
}

/// Recursion guard: a compaction sub-agent's source is tagged so the trigger
/// suppresses itself (prevents infinite compaction loop). Circuit breaker after
/// N consecutive autocompact failures.
#[derive(Default)]
pub struct Compactor {
    /// The compaction configuration (thresholds, budgets).
    pub config: CompactConfig,
    /// Consecutive autocompact failures (circuit breaker counter).
    pub consecutive_failures: u32,
    /// Whether a compaction is currently in flight (recursion guard).
    pub is_compacting: bool,
}

impl Compactor {
    /// Construct a compactor with default config.
    pub fn new() -> Self {
        Self::default()
    }
    // TODO(T08+): maybe_compact(utilization, messages, cache_state) -> Vec<AgentEvent>.
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T07: AutoCompactSummary::default() is empty and round-trips through serde.
    #[test]
    fn autocompact_summary_default_and_serde() {
        let d = AutoCompactSummary::default();
        assert!(d.primary_request_and_intent.is_empty());
        assert!(d.key_technical_concepts.is_empty());
        assert!(d.all_user_messages_verbatim.is_empty());

        let s = AutoCompactSummary {
            primary_request_and_intent: "refactor auth".into(),
            key_technical_concepts: vec!["JWT".into()],
            files_and_code_sections: vec!["auth.py:42".into()],
            errors_and_fixes: vec!["null deref -> added check".into()],
            problem_solving: "traced via stack".into(),
            all_user_messages_verbatim: vec!["fix login".into()],
            pending_tasks: vec!["add tests".into()],
            current_work: "middleware".into(),
            optional_next_step: "wire routes".into(),
        };
        let json = serde_json::to_string(&s).expect("serialize");
        let back: AutoCompactSummary = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            back.primary_request_and_intent,
            s.primary_request_and_intent
        );
        assert_eq!(
            back.all_user_messages_verbatim,
            s.all_user_messages_verbatim
        );
    }

    /// T08: CompactConfig::default() thresholds are sane.
    #[test]
    fn compact_config_default_sane() {
        let c = CompactConfig::default();
        assert!(
            c.collapse_threshold > c.autocompact_threshold,
            "collapse ({}) should be > autocompact ({})",
            c.collapse_threshold,
            c.autocompact_threshold
        );
        assert!(c.post_compact_keep_files <= 5);
        assert!(c.post_compact_keep_user_msgs >= 5);
    }
}
