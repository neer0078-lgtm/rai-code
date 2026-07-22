//! Client-side ToolSearch — the lazy-loaded tool catalog (clean-room pattern
//! from Claude Code: don't send all tool schemas to the model; send a
//! name-only candidate catalog + a `ToolSearch` tool, load full schemas on
//! demand, max N per search, loaded tools stay for subsequent turns).
//!
//! T24 is a pure, fully-tested implementation — no network, no MCP server.

use std::collections::HashSet;

/// A client-side catalog of candidate MCP tools (name-only until loaded).
///
/// The model sees the candidate names (cheap); it calls `ToolSearch` to load
/// the full schema for the ones it actually wants (max `max_per_search` at a
/// time). Loaded tools stay in the working set for subsequent turns (until
/// compaction evicts them).
#[derive(Debug, Clone, Default)]
pub struct ToolCatalog {
    /// All known tool names (the candidate catalog).
    pub names: Vec<String>,
    /// Names already loaded into the working set (have their full schema).
    pub loaded: HashSet<String>,
    /// Max tools loaded per search (Claude Code uses ~5).
    pub max_per_search: usize,
}

impl ToolCatalog {
    /// Construct an empty catalog with a max-per-search of 5.
    pub fn new() -> Self {
        Self {
            names: vec![],
            loaded: HashSet::new(),
            max_per_search: 5,
        }
    }

    /// Construct a catalog from a list of candidate tool names.
    pub fn from_names(names: Vec<String>) -> Self {
        Self {
            names,
            loaded: HashSet::new(),
            max_per_search: 5,
        }
    }

    /// Set the max-per-search.
    pub fn with_max_per_search(mut self, n: usize) -> Self {
        self.max_per_search = n;
        self
    }

    /// Search the candidate catalog by keyword (case-insensitive substring).
    /// Returns up to `max_per_search` names that match AND are not yet loaded.
    /// An empty query returns the first `max_per_search` unloaded names.
    pub fn search(&self, query: &str) -> Vec<&str> {
        let q = query.trim().to_lowercase();
        self.names
            .iter()
            .filter(|n| !self.loaded.contains(*n))
            .filter(|n| q.is_empty() || n.to_lowercase().contains(&q))
            .take(self.max_per_search)
            .map(|n| n.as_str())
            .collect()
    }

    /// Mark a tool as loaded (it now has its full schema in the working set).
    /// Returns true if it was newly loaded, false if it was already loaded or
    /// not in the catalog.
    pub fn mark_loaded(&mut self, name: &str) -> bool {
        if !self.names.iter().any(|n| n == name) {
            return false;
        }
        self.loaded.insert(name.to_string())
    }

    /// The names currently in the working set (loaded).
    pub fn loaded_names(&self) -> Vec<&str> {
        self.names
            .iter()
            .filter(|n| self.loaded.contains(*n))
            .map(|n| n.as_str())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T24: empty query returns the first max_per_search unloaded names.
    #[test]
    fn search_empty_returns_first_unloaded() {
        let cat = ToolCatalog::from_names(vec![
            "mcp__graphiti__search".into(),
            "mcp__graphiti__add_episode".into(),
            "mcp__hindsight__recall".into(),
        ]);
        let got = cat.search("");
        assert_eq!(got.len(), 3); // all 3, under the max of 5
        assert_eq!(got[0], "mcp__graphiti__search");
    }

    /// T24: a query matching N candidates returns those N (up to max).
    #[test]
    fn search_query_matches_substring() {
        let cat = ToolCatalog::from_names(vec![
            "mcp__graphiti__search".into(),
            "mcp__graphiti__add_episode".into(),
            "mcp__hindsight__recall".into(),
            "mcp__hindsight__retain".into(),
        ]);
        let got = cat.search("graphiti");
        assert_eq!(got.len(), 2);
        assert!(got.iter().all(|n| n.contains("graphiti")));
    }

    /// T24: max caps the result count.
    #[test]
    fn search_caps_at_max_per_search() {
        let cat = ToolCatalog::from_names(vec![
            "a_search".into(),
            "b_search".into(),
            "c_search".into(),
        ])
        .with_max_per_search(2);
        let got = cat.search("search");
        assert_eq!(got.len(), 2); // capped at 2
    }

    /// T24: loaded tools are excluded from search results.
    #[test]
    fn search_excludes_loaded() {
        let mut cat = ToolCatalog::from_names(vec![
            "graphiti_search".into(),
            "graphiti_add".into(),
            "hindsight_recall".into(),
        ]);
        assert!(cat.mark_loaded("graphiti_search"));
        let got = cat.search("graphiti");
        // graphiti_search is loaded -> excluded; graphiti_add still matches.
        assert_eq!(got, vec!["graphiti_add"]);
    }

    /// T24: mark_loaded returns false for unknown names + idempotent for known.
    #[test]
    fn mark_loaded_semantics() {
        let mut cat = ToolCatalog::from_names(vec!["a".into(), "b".into()]);
        assert!(cat.mark_loaded("a")); // newly loaded
        assert!(!cat.mark_loaded("a")); // already loaded -> false
        assert!(!cat.mark_loaded("zzz")); // not in catalog -> false
        assert_eq!(cat.loaded_names(), vec!["a"]);
    }

    /// T24: search is case-insensitive.
    #[test]
    fn search_is_case_insensitive() {
        let cat = ToolCatalog::from_names(vec!["Graphiti_Search".into()]);
        let got = cat.search("GRAPHITI");
        assert_eq!(got, vec!["Graphiti_Search"]);
    }
}
