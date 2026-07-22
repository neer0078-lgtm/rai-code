//! The MCP server — exposes RAI Code's own tools to external MCP clients via
//! `rmcp`'s `ServerHandler` trait + the `#[tool]` macro. All traffic is routed
//! through `rai-security` (AgentK mediates `tool.invoke`).
//!
//! T22 keeps the live `ServerHandler` impl stubbed (it requires an async
//! transport + a running client); the testable surface is the `McpServer`
//! shape + the `ExposedTool` descriptor. The full `serve_server` wiring lands
//! when the loop exposes RAI Code's tools (a later task).

use serde::{Deserialize, Serialize};

/// A tool RAI Code exposes over MCP (descriptor only — the handler is wired later).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExposedTool {
    /// The tool name (without the `mcp__rai__` prefix — rmcp adds it).
    pub name: String,
    /// The model-facing description.
    pub description: String,
    /// The JSON Schema for the input.
    pub input_schema: serde_json::Value,
}

/// The MCP server. Holds the tools RAI Code exposes. T22 stubs the live
/// `ServerHandler`/`serve_server` wiring; the testable surface is the
/// tool registry.
pub struct McpServer {
    /// The server name (tools are exposed as `mcp__{name}__...`).
    pub name: String,
    /// The tools this server exposes.
    pub tools: Vec<ExposedTool>,
}

impl McpServer {
    /// Construct a new, empty server with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            tools: vec![],
        }
    }

    /// Expose a tool.
    pub fn expose(&mut self, tool: ExposedTool) {
        self.tools.push(tool);
    }

    /// The names of the exposed tools.
    pub fn tool_names(&self) -> Vec<&str> {
        self.tools.iter().map(|t| t.name.as_str()).collect()
    }

    // TODO(loop-wiring): impl rmcp::ServerHandler + serve_server(transport).
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T22: McpServer constructs + exposes tools + lists names.
    #[test]
    fn mcp_server_exposes_tools() {
        let mut s = McpServer::new("rai");
        assert!(s.tool_names().is_empty());

        s.expose(ExposedTool {
            name: "read_file".into(),
            description: "read a file".into(),
            input_schema: serde_json::json!({"type": "object"}),
        });
        s.expose(ExposedTool {
            name: "grep".into(),
            description: "search".into(),
            input_schema: serde_json::json!({"type": "object"}),
        });

        assert_eq!(s.name, "rai");
        assert_eq!(s.tool_names(), vec!["read_file", "grep"]);
    }
}
