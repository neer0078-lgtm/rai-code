//! Graphiti client — wraps the temporal code KG via the MemoryStore trait.
//!
//! The SidecarMemoryStore (T40's scripts/sidecar.py) handles the actual
//! Graphiti calls (add_episode, search) over JSON-RPC. This module is a
//! placeholder for Graphiti-specific helpers (entity/edge type builders,
//! temporal query helpers) that land in the loop-wiring phase.

/// A Graphiti episode description (what changed + why), ready for ingestion.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Episode {
    /// The episode text (e.g. "Commit abc changed function foo in bar.rs, adding parameter baz").
    pub text: String,
    /// The source (e.g. "commit:abc123", "edit:bar.rs", "pr:42").
    pub source: String,
    /// The timestamp (ISO 8601 or epoch nanos).
    pub timestamp: String,
}
