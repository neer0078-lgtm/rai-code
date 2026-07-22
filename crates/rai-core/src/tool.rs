//! The self-describing Tool trait + StreamingToolExecutor + ToolRegistry.
//!
//! Clean-room pattern (non-copyrightable): each tool carries its own identity,
//! input schema, async handler, permission predicate, concurrency-safety flag.
//! No central orchestrator "knows" about tools — adding tool N+1 changes nothing.
//!
//! T12/T14: Tool trait + StreamingToolExecutor classifier.
//! T42: ToolRegistry (get + schema_for_model).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// A tool call requested by the model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolCall {
    /// Tool identifier (stable across calls).
    pub name: String,
    /// Arguments as a JSON object.
    pub args: Value,
    /// Provider-assigned call id (for streaming correlation).
    pub call_id: String,
}

/// A tool result returned to the loop.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolResult {
    /// The provider-assigned call id this result corresponds to.
    pub call_id: String,
    /// Structured content (string, JSON, image ref, etc.).
    pub content: ToolContent,
    /// Whether the tool errored.
    pub is_error: bool,
}

/// The payload of a tool result. Structured (not free-form text) — the Warp pattern.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "data")]
pub enum ToolContent {
    /// A plain-text result.
    Text(String),
    /// A JSON-structured result.
    Json(Value),
    /// A reference to a captured image (rendered in the BrowserPane).
    ImageRef {
        /// Path/identifier of the image.
        path: String,
    },
    /// A file diff produced by an edit tool.
    FileDiff {
        /// The file path.
        path: String,
        /// The unified diff.
        diff: String,
    },
    /// No content (e.g., a void side-effect).
    Empty,
}

/// Context handed to every tool execution.
pub struct ToolContext<'a> {
    /// The working directory the tool operates in.
    pub workdir: &'a std::path::Path,
    /// The permission resolver for inline permission checks.
    pub permission: &'a crate::perm::PermissionResolver,
    /// A cancellation token the tool should poll on long operations.
    pub cancellation: tokio_util::sync::CancellationToken,
    // TODO(T12+): sandbox handle, graph/hindsight clients, security kernel.
}

/// The self-describing tool contract.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Stable tool name.
    fn name(&self) -> &str;
    /// Human/model-facing description.
    fn description(&self) -> &str;
    /// JSON Schema for the input (serialized for the model).
    fn input_schema(&self) -> serde_json::Value;
    /// Whether this tool is safe to run concurrently with other concurrency-safe tools.
    fn is_concurrency_safe(&self) -> bool;
    /// Execute the tool. Errors become `ToolResult { is_error: true }`.
    async fn execute(&self, call: ToolCall, ctx: ToolContext<'_>) -> ToolResult;
}

/// The streaming tool executor: starts tools during model streaming.
#[derive(Default)]
pub struct StreamingToolExecutor {
    // TODO(T14): JoinSet<Task>, pending queue.
}

impl StreamingToolExecutor {
    /// Construct a new, empty executor.
    pub fn new() -> Self {
        Self::default()
    }

    /// T14: pure concurrency classifier — can `incoming` execute given `pending`?
    pub fn can_execute_tool(pending: &[bool], incoming: bool) -> bool {
        if pending.is_empty() {
            return true;
        }
        pending.iter().all(|&safe| safe) && incoming
    }
}

/// T42: the tool registry — holds `Arc<dyn Tool>`; the loop queries by name
/// and collects schemas for the model.
#[derive(Default)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    /// Construct an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a tool.
    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    /// Get a tool by name.
    pub fn get(&self, name: &str) -> Option<&Arc<dyn Tool>> {
        self.tools.get(name)
    }

    /// The number of registered tools.
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// T42: collect all tool schemas for the model (name + description + input_schema).
    /// This is the payload sent to the LLM as the `tools` parameter.
    pub fn schema_for_model(&self) -> Vec<serde_json::Value> {
        self.tools
            .values()
            .map(|t| {
                serde_json::json!({
                    "name": t.name(),
                    "description": t.description(),
                    "input_schema": t.input_schema(),
                })
            })
            .collect()
    }

    /// The registered tool names.
    pub fn names(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T12: a concrete read-only `EchoTool` implements `Tool` and executes correctly.
    #[tokio::test]
    async fn echo_tool_executes() {
        struct EchoTool;
        #[async_trait]
        impl Tool for EchoTool {
            fn name(&self) -> &str {
                "Echo"
            }
            fn description(&self) -> &str {
                "echoes the args as text"
            }
            fn input_schema(&self) -> serde_json::Value {
                serde_json::json!({"type": "object"})
            }
            fn is_concurrency_safe(&self) -> bool {
                true
            }
            async fn execute(&self, call: ToolCall, _ctx: ToolContext<'_>) -> ToolResult {
                ToolResult {
                    call_id: call.call_id,
                    content: ToolContent::Text(call.args.to_string()),
                    is_error: false,
                }
            }
        }

        let tool = EchoTool;
        assert_eq!(tool.name(), "Echo");
        assert!(tool.is_concurrency_safe());

        let call = ToolCall {
            name: "Echo".into(),
            args: serde_json::json!({"msg": "hello"}),
            call_id: "c1".into(),
        };
        let ctx = ToolContext {
            workdir: std::path::Path::new("."),
            permission: &crate::perm::PermissionResolver::new(
                crate::perm::PermissionMode::Approval,
            ),
            cancellation: tokio_util::sync::CancellationToken::new(),
        };
        let res = tool.execute(call, ctx).await;
        assert_eq!(res.call_id, "c1");
        assert!(!res.is_error);
        match res.content {
            ToolContent::Text(t) => assert!(t.contains("hello")),
            other => panic!("expected Text, got {other:?}"),
        }
    }

    /// T13: all 5 ToolContent variants round-trip through serde.
    #[test]
    fn tool_content_serde_roundtrip() {
        let contents = vec![
            ToolContent::Text("hi".into()),
            ToolContent::Json(serde_json::json!({"k": 1})),
            ToolContent::ImageRef {
                path: "/tmp/x.png".into(),
            },
            ToolContent::FileDiff {
                path: "foo.rs".into(),
                diff: "@@ -1 +1 @@".into(),
            },
            ToolContent::Empty,
        ];
        for c in contents {
            let json = serde_json::to_string(&c).expect("serialize");
            let back: ToolContent = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(c, back);
        }
        let r = ToolResult {
            call_id: "c1".into(),
            content: ToolContent::Text("boom".into()),
            is_error: true,
        };
        let json = serde_json::to_string(&r).expect("serialize");
        let back: ToolResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(r, back);
        assert!(back.is_error);
    }

    /// T14: can_execute_tool classifier.
    #[test]
    fn can_execute_tool_classifier() {
        assert!(StreamingToolExecutor::can_execute_tool(&[], true));
        assert!(StreamingToolExecutor::can_execute_tool(&[], false));
        assert!(StreamingToolExecutor::can_execute_tool(&[true, true], true));
        assert!(!StreamingToolExecutor::can_execute_tool(
            &[true, false],
            true
        ));
        assert!(!StreamingToolExecutor::can_execute_tool(&[false], true));
        assert!(!StreamingToolExecutor::can_execute_tool(
            &[true, true],
            false
        ));
    }

    /// T42: ToolRegistry register + get + schema_for_model + names.
    #[tokio::test]
    async fn tool_registry_register_and_schema() {
        struct EchoTool;
        #[async_trait]
        impl Tool for EchoTool {
            fn name(&self) -> &str {
                "Echo"
            }
            fn description(&self) -> &str {
                "echo"
            }
            fn input_schema(&self) -> serde_json::Value {
                serde_json::json!({"type": "object"})
            }
            fn is_concurrency_safe(&self) -> bool {
                true
            }
            async fn execute(&self, call: ToolCall, _ctx: ToolContext<'_>) -> ToolResult {
                ToolResult {
                    call_id: call.call_id,
                    content: ToolContent::Text("ok".into()),
                    is_error: false,
                }
            }
        }

        let mut reg = ToolRegistry::new();
        assert!(reg.is_empty());
        reg.register(Arc::new(EchoTool));
        assert_eq!(reg.len(), 1);

        // get by name.
        let t = reg.get("Echo").expect("registered");
        assert_eq!(t.name(), "Echo");

        // get unknown -> None.
        assert!(reg.get("Bogus").is_none());

        // schema_for_model -> 1 entry with name + description + input_schema.
        let schemas = reg.schema_for_model();
        assert_eq!(schemas.len(), 1);
        assert_eq!(schemas[0]["name"], "Echo");
        assert_eq!(schemas[0]["description"], "echo");

        // names.
        assert_eq!(reg.names(), vec!["Echo"]);
    }
}
