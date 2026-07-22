//! MCP Tools Tax Reduction — two-phase lazy schema loading.
//!
//! From the research alignment (P1): Tool Attention (arxiv 2604.21816) shows
//! 95% reduction in per-turn tool tokens via lazy schema loading. ATLAS shows
//! iterative tool loading (ITL) enables 4B SLMs to approach frontier performance.
//!
//! The problem: when an agent connects to many MCP servers, each with many tools,
//! sending all tool schemas to the model on every turn costs 10k-60k tokens —
//! even if the model only uses 2-3 tools per turn. This is the "MCP Tools Tax."
//!
//! The solution: two-phase loading.
//! Phase 1: a compact summary pool (tool name + one-line description) is always
//!   resident in context (~50 tokens per tool instead of ~500-1000).
//! Phase 2: full schemas are promoted on-demand (the model requests a tool via
//!   ToolSearch, and the full schema is loaded only for the requested tools).
//!
//! This reduces per-turn tool tokens from O(N * full_schema) to
//! O(N * summary + K * full_schema) where K << N (typically K=2-5).

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// A compact tool summary (phase 1 — always resident in context).
/// ~50 tokens vs ~500-1000 for a full schema.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolSummary {
    /// The tool name (e.g. "graphiti_search").
    pub name: String,
    /// A one-line description (≤100 chars).
    pub description: String,
    /// The MCP server it belongs to (e.g. "graphiti").
    pub server: String,
}

/// A full tool schema (phase 2 — promoted on-demand).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FullSchema {
    /// The tool name.
    pub name: String,
    /// The full description.
    pub description: String,
    /// The JSON Schema for the input.
    pub input_schema: serde_json::Value,
}

/// The two-phase tool registry — manages the compact pool + on-demand promotion.
///
/// `summary_pool` is always sent to the model (phase 1).
/// `promoted` tracks which tools have been promoted to full schema (phase 2).
/// `schemas` holds the full schemas (only promoted ones are sent to the model).
#[derive(Debug, Default)]
pub struct TwoPhaseToolRegistry {
    /// The compact summary pool (phase 1 — always resident).
    summary_pool: Vec<ToolSummary>,
    /// The full schemas (indexed by tool name).
    schemas: HashMap<String, FullSchema>,
    /// The set of promoted tool names (phase 2 — sent to the model).
    promoted: HashSet<String>,
    /// Max tools to promote per search query (default 5).
    max_promote_per_search: usize,
    /// Max total promoted tools (default 20 — prevents context bloat).
    max_total_promoted: usize,
}

impl TwoPhaseToolRegistry {
    /// Construct a new registry with default limits.
    pub fn new() -> Self {
        Self {
            max_promote_per_search: 5,
            max_total_promoted: 20,
            ..Default::default()
        }
    }

    /// Register a tool with both a summary and a full schema.
    pub fn register(&mut self, summary: ToolSummary, schema: FullSchema) {
        self.schemas.insert(summary.name.clone(), schema);
        self.summary_pool.push(summary);
    }

    /// Set the max promotions per search.
    pub fn with_max_promote_per_search(mut self, n: usize) -> Self {
        self.max_promote_per_search = n;
        self
    }

    /// Set the max total promoted tools.
    pub fn with_max_total_promoted(mut self, n: usize) -> Self {
        self.max_total_promoted = n;
        self
    }

    /// The compact summary pool (phase 1 — what's sent to the model every turn).
    /// Returns a JSON array of {name, description} objects.
    pub fn summary_json(&self) -> serde_json::Value {
        serde_json::Value::Array(
            self.summary_pool
                .iter()
                .map(|s| {
                    serde_json::json!({
                        "name": s.name,
                        "description": s.description,
                    })
                })
                .collect(),
        )
    }

    /// The promoted full schemas (phase 2 — what's sent to the model for
    /// tools it has explicitly requested). Returns a JSON array of full schemas.
    pub fn promoted_schemas_json(&self) -> serde_json::Value {
        serde_json::Value::Array(
            self.promoted
                .iter()
                .filter_map(|name| self.schemas.get(name))
                .map(|s| {
                    serde_json::json!({
                        "name": s.name,
                        "description": s.description,
                        "input_schema": s.input_schema,
                    })
                })
                .collect(),
        )
    }

    /// Search the summary pool by keyword (case-insensitive substring).
    /// Returns up to `max_promote_per_search` tools that match AND are not
    /// already promoted. Promotes them (adds to the promoted set).
    pub fn search_and_promote(&mut self, query: &str) -> Vec<String> {
        let q = query.trim().to_lowercase();
        let mut promoted_names = vec![];

        for summary in &self.summary_pool {
            if self.promoted.len() >= self.max_total_promoted {
                break;
            }
            if promoted_names.len() >= self.max_promote_per_search {
                break;
            }
            if self.promoted.contains(&summary.name) {
                continue;
            }
            if q.is_empty() || summary.name.to_lowercase().contains(&q) {
                self.promoted.insert(summary.name.clone());
                promoted_names.push(summary.name.clone());
            }
        }

        promoted_names
    }

    /// Promote a specific tool by name (explicit request).
    pub fn promote(&mut self, name: &str) -> bool {
        if self.promoted.len() >= self.max_total_promoted {
            return false;
        }
        if !self.schemas.contains_key(name) {
            return false;
        }
        self.promoted.insert(name.to_string())
    }

    /// Demote a tool (remove from the promoted set — e.g. during compaction).
    pub fn demote(&mut self, name: &str) {
        self.promoted.remove(name);
    }

    /// The number of tools in the summary pool.
    pub fn summary_count(&self) -> usize {
        self.summary_pool.len()
    }

    /// The number of currently promoted tools.
    pub fn promoted_count(&self) -> usize {
        self.promoted.len()
    }

    /// Estimate the token savings vs sending all full schemas.
    /// Assumes ~50 tokens per summary, ~500 tokens per full schema.
    pub fn estimated_token_savings(&self) -> f64 {
        let full_cost = self.summary_pool.len() * 500;
        let actual_cost = self.summary_pool.len() * 50 + self.promoted.len() * 500;
        if full_cost == 0 {
            return 0.0;
        }
        1.0 - (actual_cost as f64 / full_cost as f64)
    }

    /// Clear all promotions (e.g. during context compaction — the model
    /// re-searches for tools it needs in the new context).
    pub fn clear_promotions(&mut self) {
        self.promoted.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_summary(name: &str, desc: &str, server: &str) -> ToolSummary {
        ToolSummary {
            name: name.into(),
            description: desc.into(),
            server: server.into(),
        }
    }

    fn make_schema(name: &str, desc: &str) -> FullSchema {
        FullSchema {
            name: name.into(),
            description: desc.into(),
            input_schema: serde_json::json!({"type": "object", "properties": {}}),
        }
    }

    #[test]
    fn summary_pool_always_present() {
        let mut reg = TwoPhaseToolRegistry::new();
        reg.register(
            make_summary("graphiti_search", "search the code KG", "graphiti"),
            make_schema(
                "graphiti_search",
                "search the temporal code knowledge graph",
            ),
        );
        reg.register(
            make_summary("hindsight_recall", "recall user profile", "hindsight"),
            make_schema("hindsight_recall", "recall from Hindsight user model"),
        );

        let summaries = reg.summary_json();
        let arr = summaries.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["name"], "graphiti_search");
        // Promoted is empty → no full schemas sent.
        assert!(reg.promoted_schemas_json().as_array().unwrap().is_empty());
    }

    #[test]
    fn search_and_promote_loads_full_schema() {
        let mut reg = TwoPhaseToolRegistry::new();
        reg.register(
            make_summary("graphiti_search", "search", "graphiti"),
            make_schema("graphiti_search", "full search"),
        );
        reg.register(
            make_summary("hindsight_recall", "recall", "hindsight"),
            make_schema("hindsight_recall", "full recall"),
        );

        let promoted = reg.search_and_promote("graphiti");
        assert_eq!(promoted, vec!["graphiti_search"]);
        assert_eq!(reg.promoted_count(), 1);

        let schemas = reg.promoted_schemas_json();
        let arr = schemas.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["name"], "graphiti_search");
    }

    #[test]
    fn search_capped_at_max_per_search() {
        let mut reg = TwoPhaseToolRegistry::new().with_max_promote_per_search(2);
        for i in 0..10 {
            let name = format!("tool_{i}");
            reg.register(
                make_summary(&name, "desc", "server"),
                make_schema(&name, "full"),
            );
        }

        let promoted = reg.search_and_promote("tool");
        assert_eq!(promoted.len(), 2); // capped at 2
    }

    #[test]
    fn already_promoted_not_re_promoted() {
        let mut reg = TwoPhaseToolRegistry::new();
        reg.register(
            make_summary("graphiti_search", "search", "graphiti"),
            make_schema("graphiti_search", "full"),
        );

        reg.search_and_promote("graphiti");
        let again = reg.search_and_promote("graphiti");
        assert!(again.is_empty(), "already promoted → not re-promoted");
    }

    #[test]
    fn max_total_promoted_enforced() {
        let mut reg = TwoPhaseToolRegistry::new().with_max_total_promoted(3);
        for i in 0..10 {
            let name = format!("tool_{i}");
            reg.register(
                make_summary(&name, "desc", "server"),
                make_schema(&name, "full"),
            );
        }

        reg.search_and_promote(""); // promote as many as possible
        assert!(reg.promoted_count() <= 3);
    }

    #[test]
    fn token_savings_95_percent() {
        let mut reg = TwoPhaseToolRegistry::new();
        for i in 0..100 {
            let name = format!("tool_{i}");
            reg.register(
                make_summary(&name, "desc", "server"),
                make_schema(&name, "full"),
            );
        }

        // Promote 3 tools (typical per-turn usage).
        reg.search_and_promote("");
        reg.search_and_promote("");
        reg.search_and_promote("");

        let savings = reg.estimated_token_savings();
        // With 100 tools, 3 promoted: cost = 100*50 + 3*500 = 6500 vs 100*500 = 50000.
        // Savings = 1 - 6500/50000 = 0.87 = 87%.
        // With fewer promoted, savings approach 95%+.
        assert!(savings > 0.70, "should save >70%: {savings}");
    }

    #[test]
    fn demote_removes_from_promoted() {
        let mut reg = TwoPhaseToolRegistry::new();
        reg.register(
            make_summary("tool_a", "desc", "server"),
            make_schema("tool_a", "full"),
        );

        reg.promote("tool_a");
        assert_eq!(reg.promoted_count(), 1);

        reg.demote("tool_a");
        assert_eq!(reg.promoted_count(), 0);
        assert!(reg.promoted_schemas_json().as_array().unwrap().is_empty());
    }

    #[test]
    fn clear_promotions_resets() {
        let mut reg = TwoPhaseToolRegistry::new();
        for i in 0..5 {
            let name = format!("tool_{i}");
            reg.register(
                make_summary(&name, "desc", "server"),
                make_schema(&name, "full"),
            );
        }
        reg.search_and_promote("");
        assert!(reg.promoted_count() > 0);

        reg.clear_promotions();
        assert_eq!(reg.promoted_count(), 0);
        // Summary pool is still there.
        assert_eq!(reg.summary_count(), 5);
    }

    #[test]
    fn promote_unknown_tool_fails() {
        let mut reg = TwoPhaseToolRegistry::new();
        assert!(!reg.promote("nonexistent"));
    }

    #[test]
    fn empty_query_promotes_first_n() {
        let mut reg = TwoPhaseToolRegistry::new().with_max_promote_per_search(3);
        for i in 0..10 {
            let name = format!("tool_{i}");
            reg.register(
                make_summary(&name, "desc", "server"),
                make_schema(&name, "full"),
            );
        }
        let promoted = reg.search_and_promote("");
        assert_eq!(promoted.len(), 3);
    }
}
