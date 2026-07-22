//! rai-codegen — the deterministic code-graph layer (DCG).
//!
//! Layer 1 of the hybrid two-layer code context (the other is Graphiti via
//! rai-python). tree-sitter: incremental parsing (<1ms), 40+ langs.
//! async-lsp: types, precise cross-file refs, call/type hierarchy, diagnostics.
//! SCIP consumer: compiler-accurate nav when available. tree-house: Helix-grade
//! syntax highlighting.
//!
//! DCG handles STRUCTURAL facts (calls, imports, types, refs) — fast, exact,
//! free (no LLM). Graphiti (rai-python) handles SEMANTIC/TEMPORAL facts.
#![warn(missing_docs)]

pub mod dcg;
pub mod highlight;
pub mod lsp;

pub use dcg::{parse_rust, Dcg, FileNode, Position, Symbol, SymbolKind};
