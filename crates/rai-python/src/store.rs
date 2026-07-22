//! The MemoryStore trait + MockMemoryStore + JSON-RPC wire types + sidecar spawn config.
//!
//! T37: MemoryStore trait + MockMemoryStore (in-memory HashMap) for tests.
//! T38: JsonRpcRequest/Response/Error wire types (serde, parse_request handles malformed).
//! T39: SidecarSpawnConfig (the command + args + env for spawning the Python sidecar).
//! T40: the Python sidecar script itself (scripts/sidecar.py, --self-test).

use async_trait::async_trait;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;

/// The async MemoryStore contract — the interface for Graphiti (temporal code
/// KG) + Hindsight (user model). Two impls: MockMemoryStore (tests) +
/// SidecarMemoryStore (Phase 1, JSON-RPC over stdin/stdout to a Python process).
#[async_trait]
pub trait MemoryStore: Send + Sync {
    /// Search the Graphiti temporal code KG.
    async fn graphiti_search(&self, q: &str) -> anyhow::Result<serde_json::Value>;
    /// Add an episode to Graphiti (a commit, a significant edit, a PR).
    async fn graphiti_add_episode(&self, ep: &str) -> anyhow::Result<()>;
    /// Recall from Hindsight (the user model — who is this user?).
    async fn hindsight_recall(&self, q: &str) -> anyhow::Result<serde_json::Value>;
    /// Retain to Hindsight (a user behavior, a preference, a delegated action).
    async fn hindsight_retain(&self, content: &str) -> anyhow::Result<()>;
}

/// A mock MemoryStore backed by in-memory HashMaps (for tests — no Python, no network).
/// Uses Mutex for interior mutability so `retain` works via `&self`.
#[derive(Default)]
pub struct MockMemoryStore {
    episodes: Mutex<HashMap<String, String>>,
    memories: Mutex<HashMap<String, String>>,
}

impl MockMemoryStore {
    /// Construct an empty mock store.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl MemoryStore for MockMemoryStore {
    async fn graphiti_search(&self, q: &str) -> anyhow::Result<serde_json::Value> {
        let eps = self.episodes.lock();
        let matches: Vec<String> = eps.values().filter(|ep| ep.contains(q)).cloned().collect();
        Ok(serde_json::json!({"results": matches}))
    }

    async fn graphiti_add_episode(&self, ep: &str) -> anyhow::Result<()> {
        let mut eps = self.episodes.lock();
        let id = format!("ep-{}", eps.len());
        eps.insert(id, ep.to_string());
        Ok(())
    }

    async fn hindsight_recall(&self, q: &str) -> anyhow::Result<serde_json::Value> {
        let mems = self.memories.lock();
        let matches: Vec<String> = mems.values().filter(|m| m.contains(q)).cloned().collect();
        Ok(serde_json::json!({"results": matches}))
    }

    async fn hindsight_retain(&self, content: &str) -> anyhow::Result<()> {
        let mut mems = self.memories.lock();
        let id = format!("mem-{}", mems.len());
        mems.insert(id, content.to_string());
        Ok(())
    }
}

/// T38: a JSON-RPC 2.0 request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcRequest {
    /// The request id (a number or string; here a string for simplicity).
    pub id: serde_json::Value,
    /// The method name (e.g. "graphiti_search").
    pub method: String,
    /// The params (a JSON object).
    pub params: serde_json::Value,
}

/// T38: a JSON-RPC 2.0 response (success).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcResponse {
    /// The request id this response corresponds to.
    pub id: serde_json::Value,
    /// The result (on success).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// The error (on failure).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// T38: a JSON-RPC 2.0 error.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcError {
    /// The error code (e.g. -32600 for invalid request).
    pub code: i32,
    /// The error message.
    pub message: String,
    /// Optional additional data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcError {
    /// Construct a "method not found" error (-32601).
    pub fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: format!("method not found: {method}"),
            data: None,
        }
    }

    /// Construct a "parse error" (-32700).
    pub fn parse_error(detail: &str) -> Self {
        Self {
            code: -32700,
            message: format!("parse error: {detail}"),
            data: None,
        }
    }
}

impl JsonRpcResponse {
    /// Construct a success response.
    pub fn ok(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self {
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Construct an error response.
    pub fn err(id: serde_json::Value, error: JsonRpcError) -> Self {
        Self {
            id,
            result: None,
            error: Some(error),
        }
    }
}

/// T38: parse a JSON-RPC request from a string. Returns an error for malformed
/// JSON or missing required fields.
pub fn parse_request(buf: &str) -> anyhow::Result<JsonRpcRequest> {
    let req: JsonRpcRequest = serde_json::from_str(buf)?;
    if req.method.is_empty() {
        anyhow::bail!("JSON-RPC request missing method");
    }
    Ok(req)
}

/// T39: the config for spawning the Python sidecar process.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SidecarSpawnConfig {
    /// The Python command (e.g. "python3").
    pub command: String,
    /// The script path (e.g. "scripts/sidecar.py").
    pub script: String,
    /// Additional args (e.g. ["--self-test"]).
    pub args: Vec<String>,
    /// Optional env vars.
    pub env: Vec<(String, String)>,
}

impl SidecarSpawnConfig {
    /// Construct a sidecar config for the given Python command + script.
    pub fn new(command: impl Into<String>, script: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            script: script.into(),
            args: vec![],
            env: vec![],
        }
    }

    /// Add an arg.
    pub fn with_arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Add an env var.
    pub fn with_env(mut self, key: impl Into<String>, val: impl Into<String>) -> Self {
        self.env.push((key.into(), val.into()));
        self
    }

    /// T39: build the `tokio::process::Command` for spawning the sidecar
    /// (WITHOUT spawning — the config is testable without a real Python).
    /// Sets up stdin=Stdio::piped(), stdout=Stdio::piped(), stderr=Stdio::inherit().
    pub fn build_command(&self) -> anyhow::Result<Command> {
        if self.command.is_empty() {
            anyhow::bail!("sidecar command is empty");
        }
        let mut cmd = Command::new(&self.command);
        cmd.arg(&self.script);
        for arg in &self.args {
            cmd.arg(arg);
        }
        for (k, v) in &self.env {
            cmd.env(k, v);
        }
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::inherit());
        Ok(cmd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T37: MockMemoryStore constructs + the trait methods work.
    #[tokio::test]
    async fn mock_memory_store_works() {
        let store = MockMemoryStore::new();
        // graphiti_search on empty -> empty results.
        let r = store.graphiti_search("test").await.unwrap();
        assert!(r["results"].as_array().unwrap().is_empty());
        // graphiti_add_episode -> Ok.
        store
            .graphiti_add_episode("commit abc: changed foo")
            .await
            .unwrap();
        // hindsight_recall on empty -> empty results.
        let r = store.hindsight_recall("user").await.unwrap();
        assert!(r["results"].as_array().unwrap().is_empty());
        // hindsight_retain -> Ok.
        store
            .hindsight_retain("user prefers functional patterns")
            .await
            .unwrap();
    }

    /// T38: JsonRpcRequest/Response/Error round-trip through serde.
    #[test]
    fn jsonrpc_serde_roundtrip() {
        let req = JsonRpcRequest {
            id: serde_json::json!(1),
            method: "graphiti_search".into(),
            params: serde_json::json!({"q": "auth middleware"}),
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: JsonRpcRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, back);

        let resp = JsonRpcResponse::ok(
            serde_json::json!(1),
            serde_json::json!({"results": [{"entity": "authMiddleware"}]}),
        );
        let json = serde_json::to_string(&resp).unwrap();
        let back: JsonRpcResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp, back);
        assert!(back.error.is_none());

        let err_resp = JsonRpcResponse::err(
            serde_json::json!(2),
            JsonRpcError::method_not_found("bogus"),
        );
        let json = serde_json::to_string(&err_resp).unwrap();
        let back: JsonRpcResponse = serde_json::from_str(&json).unwrap();
        assert!(back.result.is_none());
        assert_eq!(back.error.unwrap().code, -32601);
    }

    /// T38: parse_request handles malformed JSON + missing method.
    #[test]
    fn parse_request_handles_errors() {
        // Valid request.
        let req = parse_request(r#"{"id":1,"method":"graphiti_search","params":{"q":"x"}}"#);
        assert!(req.is_ok());
        assert_eq!(req.unwrap().method, "graphiti_search");

        // Malformed JSON.
        assert!(parse_request("not json").is_err());

        // Missing method.
        assert!(parse_request(r#"{"id":1,"params":{}}"#).is_err());

        // Empty method.
        assert!(parse_request(r#"{"id":1,"method":"","params":{}}"#).is_err());
    }

    /// T39: SidecarSpawnConfig builds a Command (without spawning).
    #[test]
    fn sidecar_spawn_config_builds() {
        let cfg = SidecarSpawnConfig::new("python3", "scripts/sidecar.py")
            .with_arg("--self-test")
            .with_env("NEO4J_URL", "bolt://localhost:7687");
        assert_eq!(cfg.command, "python3");
        assert_eq!(cfg.script, "scripts/sidecar.py");
        assert_eq!(cfg.args, vec!["--self-test"]);
        assert_eq!(
            cfg.env,
            vec![("NEO4J_URL".to_string(), "bolt://localhost:7687".to_string())]
        );

        // build_command builds (doesn't spawn).
        let cmd = cfg.build_command().expect("builds");
        let _ = cmd; // compiles + didn't spawn

        // Empty command -> error.
        let bad = SidecarSpawnConfig::new("", "x.py");
        assert!(bad.build_command().is_err());
    }
}
