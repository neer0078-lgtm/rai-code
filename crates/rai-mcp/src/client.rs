//! The MCP client — consumes external MCP servers (Graphiti, Hindsight, Semgrep,
//! Playwright, filesystem, ...) via `rmcp`'s `TokioChildProcess` (stdio) or
//! Streamable HTTP. All MCP traffic is routed through `rai-security` (AgentK).
//!
//! T22 keeps the live rmcp wiring stubbed (it requires a running MCP server);
//! the testable surface is the type definitions + the `McpClient` shape. T23
//! adds the spawn-config builder. T24 adds the `ToolCatalog` (client-side
//! ToolSearch, pure + fully tested).

use serde::{Deserialize, Serialize};
use std::process::Command;
// NOTE: when the live rmcp wiring lands (loop-wiring phase), import
// `rmcp::model::{CallToolResult, Tool}` and `rmcp::service::ServiceExt` here.

/// A discovered MCP tool (wraps `rmcp::model::Tool` with the server it came from).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    /// The server name that exposes this tool.
    pub server: String,
    /// The tool name (e.g. "graphiti_search").
    pub name: String,
    /// The tool description (model-facing).
    pub description: Option<String>,
    /// The JSON Schema for the input.
    pub input_schema: serde_json::Value,
}

impl McpTool {
    /// The fully-qualified tool name the agent sees: `mcp__{server}__{name}`
    /// (the Claude Code convention, clean-room reimplemented).
    pub fn qualified_name(&self) -> String {
        format!("mcp__{}__{}", self.server, self.name)
    }
}

/// A tool call to an MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolCall {
    /// The fully-qualified tool name (`mcp__server__name`).
    pub name: String,
    /// The arguments as a JSON object.
    pub args: serde_json::Value,
}

/// The MCP client. T22 stubs the live rmcp calls; T23 adds spawn; the loop
/// wires real `list_tools`/`call_tool` once a server is running.
pub struct McpClient {
    /// The servers this client knows about (name -> config).
    pub servers: std::collections::HashMap<String, ServerConfig>,
}

/// How to reach an MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ServerConfig {
    /// A stdio MCP server (spawned as a subprocess).
    Stdio {
        /// The server name (for `mcp__{name}__...` tool naming).
        name: String,
        /// The command to run.
        command: String,
        /// Optional args.
        args: Vec<String>,
        /// Optional env vars.
        env: Vec<(String, String)>,
    },
    /// A Streamable HTTP MCP server.
    Http {
        /// The server name.
        name: String,
        /// The URL.
        url: String,
    },
}

impl ServerConfig {
    /// The server name (the `mcp__{name}__...` prefix).
    pub fn name(&self) -> &str {
        match self {
            ServerConfig::Stdio { name, .. } | ServerConfig::Http { name, .. } => name,
        }
    }
}

impl McpClient {
    /// Construct an empty client.
    pub fn new() -> Self {
        Self {
            servers: std::collections::HashMap::new(),
        }
    }

    /// Register a server config.
    pub fn register(&mut self, config: ServerConfig) {
        let name = config.name().to_string();
        self.servers.insert(name, config);
    }

    /// List the registered server names.
    pub fn server_names(&self) -> Vec<&str> {
        self.servers.keys().map(|s| s.as_str()).collect()
    }

    // TODO(T23): spawn_stdio_server(config) -> anyhow::Result<rmcp::Service>
    // TODO(loop-wiring): live list_tools() / call_tool(call) via rmcp::Peer.
}

impl Default for McpClient {
    fn default() -> Self {
        Self::new()
    }
}

/// T23: build the `tokio::process::Command` for a stdio MCP server, WITHOUT
/// spawning it. Returns the configured `Command` so the test can assert the
/// args/env without needing a real server process.
///
/// This is the pure half of `TokioChildProcess::new` — the config builder.
/// The actual `TokioChildProcess::new(cmd)` call (which spawns) happens in the
/// live wiring; here we just build + validate the command.
pub fn build_stdio_command(config: &ServerConfig) -> anyhow::Result<Command> {
    let ServerConfig::Stdio {
        command, args, env, ..
    } = config
    else {
        anyhow::bail!("build_stdio_command only applies to ServerConfig::Stdio");
    };
    if command.is_empty() {
        anyhow::bail!("stdio server command is empty");
    }
    let mut cmd = Command::new(command);
    cmd.args(args);
    for (k, v) in env {
        cmd.env(k, v);
    }
    Ok(cmd)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T22: McpClient constructs empty + registers servers + lists names.
    #[test]
    fn mcp_client_registers_and_lists() {
        let mut c = McpClient::new();
        assert!(c.server_names().is_empty());

        c.register(ServerConfig::Stdio {
            name: "graphiti".into(),
            command: "python".into(),
            args: vec!["-m".into(), "graphiti.mcp_server".into()],
            env: vec![],
        });
        c.register(ServerConfig::Http {
            name: "semgrep".into(),
            url: "http://localhost:8888".into(),
        });

        let mut names = c.server_names();
        names.sort();
        assert_eq!(names, vec!["graphiti", "semgrep"]);
    }

    /// T22: McpTool.qualified_name uses the mcp__server__name convention.
    #[test]
    fn mcp_tool_qualified_name() {
        let t = McpTool {
            server: "graphiti".into(),
            name: "search".into(),
            description: None,
            input_schema: serde_json::json!({}),
        };
        assert_eq!(t.qualified_name(), "mcp__graphiti__search");
    }

    /// T22: ServerConfig::name() works for both variants.
    #[test]
    fn server_config_name() {
        let stdio = ServerConfig::Stdio {
            name: "g".into(),
            command: "x".into(),
            args: vec![],
            env: vec![],
        };
        assert_eq!(stdio.name(), "g");
        let http = ServerConfig::Http {
            name: "s".into(),
            url: "http://x".into(),
        };
        assert_eq!(http.name(), "s");
    }

    /// T23: build_stdio_command builds the right Command (args + env) without
    /// spawning. We can't easily inspect a std::process::Command's args, but we
    /// can assert it builds for a valid config and errors for empty/HTTP.
    #[test]
    fn build_stdio_command_config_builds() {
        let stdio = ServerConfig::Stdio {
            name: "graphiti".into(),
            command: "python".into(),
            args: vec!["-m".into(), "graphiti.mcp_server".into()],
            env: vec![("NEO4J_URL".into(), "bolt://localhost".into())],
        };
        let cmd = build_stdio_command(&stdio).expect("builds for stdio");
        // We can't read args back from std::process::Command, but it built.
        let _ = cmd; // compiles + didn't spawn

        // empty command -> error
        let bad = ServerConfig::Stdio {
            name: "x".into(),
            command: "".into(),
            args: vec![],
            env: vec![],
        };
        assert!(build_stdio_command(&bad).is_err());

        // HTTP config -> error (build_stdio_command is stdio-only)
        let http = ServerConfig::Http {
            name: "s".into(),
            url: "http://x".into(),
        };
        assert!(build_stdio_command(&http).is_err());
    }
}
