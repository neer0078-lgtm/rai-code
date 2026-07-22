//! rai-mcp — Model Context Protocol client + server via `rmcp` (official Rust SDK, MIT).
//!
//! Two-phase lazy schema loading (Tool Attention, arxiv 2604.21816):
//! - Phase 1: compact summary pool (~50 tokens/tool) always resident.
//! - Phase 2: full schemas (~500-1000 tokens/tool) promoted on-demand.
//! - 95% per-turn token reduction when K << N tools are used per turn.
#![warn(missing_docs)]

pub mod client;
pub mod server;
pub mod tool_tax;
pub mod toolsearch;

pub use client::{build_stdio_command, McpClient, McpTool, McpToolCall, ServerConfig};
pub use server::{ExposedTool, McpServer};
pub use tool_tax::{FullSchema, ToolSummary, TwoPhaseToolRegistry};
pub use toolsearch::ToolCatalog;
