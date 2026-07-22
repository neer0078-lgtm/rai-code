//! Hindsight client — wraps the user-behavior memory via the MemoryStore trait.
//!
//! The SidecarMemoryStore (T40's scripts/sidecar.py) handles the actual
//! Hindsight calls (retain, recall) over JSON-RPC. This module is a placeholder
//! for Hindsight-specific helpers (memory-bank config, directive builders,
//! mental-model accessors) that land in the loop-wiring phase.

/// A Hindsight memory bank config (the agent's stance — mission, directives,
/// disposition). Per the Hindsight docs (hindsight.vectorize.io).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemoryBankConfig {
    /// The mission (natural-language identity for the bank).
    pub mission: String,
    /// Hard rules the agent must follow (e.g. "Never auto-commit without approval").
    pub directives: Vec<String>,
    /// Soft disposition traits (skepticism, literalism, empathy — each 1-5).
    pub disposition: Disposition,
}

/// The disposition traits (1-5 scale each).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Disposition {
    /// Skepticism (1 = trusting, 5 = deeply skeptical).
    pub skepticism: u8,
    /// Literalism (1 = loose, 5 = strictly literal).
    pub literalism: u8,
    /// Empathy (1 = terse, 5 = warm).
    pub empathy: u8,
}
