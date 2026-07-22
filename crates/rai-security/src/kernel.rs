//! The SecurityKernel trait + taint labels.
//!
//! Clean-room port of the AgentK pattern (github.com/Atomics-hub/agentk, MIT,
//! native Rust — no port needed): prompt-based safety ("don't run destructive
//! commands") is a SUGGESTION, not enforcement. RAI Code enforces at the
//! execution layer.
//!
//! Non-copyrightable patterns ported (studied from AgentK's intent, not its
//! literal code — RAI Code's own naming + structure):
//! - Typed syscalls with provenance + taint labels; default-deny for unknown.
//! - Taint-aware egress control — block exfiltration of tainted values.
//! - (Ed25519 capability receipts + opaque secret FD handles + MCP proxy
//!   mediation land in later hardening tasks; T25/T27 deliver the trait +
//!   taint propagation.)

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// A typed syscall the agent wants to make (the execution-layer unit of
/// permission). Unknown variants are default-denied.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "data")]
pub enum Syscall {
    /// Run a shell command.
    RunShell {
        /// The command string.
        command: String,
    },
    /// Read a file.
    ReadFile {
        /// The path.
        path: String,
    },
    /// Write a file.
    WriteFile {
        /// The path.
        path: String,
    },
    /// Make an outbound network request.
    Network {
        /// The URL.
        url: String,
    },
    /// Call an MCP tool.
    McpToolCall {
        /// The fully-qualified tool name.
        tool: String,
        /// The args.
        args: serde_json::Value,
    },
}

/// A taint label tracking the sensitivity of a value through the agent.
///
/// Taint-aware egress control uses this: a `Network` syscall whose URL/body
/// contains a `UserSecret`-tainted value is blocked (exfiltration prevention).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaintLabel {
    /// No sensitive data — safe to emit/log.
    #[default]
    Clean,
    /// A user secret (API key, password, token, ...) — must never be exfiltrated
    /// or materialized as a string the model can echo.
    UserSecret,
    /// User data (PII, private content) — must not be exfiltrated.
    UserData,
}

impl TaintLabel {
    /// The most-sensitive of two labels (UserSecret > UserData > Clean).
    pub fn most_sensitive(self, other: TaintLabel) -> TaintLabel {
        fn rank(t: TaintLabel) -> u8 {
            match t {
                TaintLabel::Clean => 0,
                TaintLabel::UserData => 1,
                TaintLabel::UserSecret => 2,
            }
        }
        if rank(self) >= rank(other) {
            self
        } else {
            other
        }
    }
}

/// The keys whose presence (case-insensitive, recursive) marks a value as
/// `UserSecret`. Used by `taint_of`.
const SECRET_KEYS: &[&str] = &[
    "secret", "password", "passwd", "token", "api_key", "apikey", "apikey",
];

/// T27: compute the taint of a JSON value.
///
/// Marks any value containing a key named `secret`/`password`/`passwd`/`token`/
/// `api_key`/`apikey` (case-insensitive) as `UserSecret`, recursively through
/// objects + arrays. Strings that look like long hex/base64 secrets could also
/// be flagged in a future pass; T27 is key-based.
pub fn taint_of(value: &serde_json::Value) -> TaintLabel {
    fn walk(v: &serde_json::Value, label: &mut TaintLabel) {
        match v {
            serde_json::Value::Object(map) => {
                for (k, child) in map {
                    if is_secret_key(k) {
                        *label = label.most_sensitive(TaintLabel::UserSecret);
                    }
                    walk(child, label);
                }
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    walk(item, label);
                }
            }
            _ => {}
        }
    }
    let mut label = TaintLabel::Clean;
    walk(value, &mut label);
    label
}

/// Whether a key name (case-insensitive) is a secret-bearing key.
fn is_secret_key(key: &str) -> bool {
    let lower = key.to_lowercase();
    SECRET_KEYS
        .iter()
        .any(|s| lower == **s || lower.contains(s))
}

/// A permission decision from the kernel.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "data")]
pub enum SecurityDecision {
    /// The syscall is allowed.
    Allow,
    /// The syscall is denied, with a reason.
    Deny(String),
    /// The syscall needs the user to decide (escalate to HITL).
    Ask(String),
}

/// The async SecurityKernel contract.
///
/// Every agent action goes through this. `mediate_tool_call` is the MCP-proxy
/// mediation point (AgentK pattern: `tool.invoke` mediated). `check_syscall`
/// is the general permission check. `flight_log_hash` returns the current
/// hash-chain tip (for tamper-evidence).
#[async_trait]
pub trait SecurityKernel: Send + Sync {
    /// The kernel's name (e.g. "default", "strict").
    fn name(&self) -> &str;
    /// Mediate an MCP tool call — the proxy point between the agent + the tool.
    async fn mediate_tool_call(&self, tool: &str, args: &serde_json::Value) -> SecurityDecision;
    /// Check a general syscall (shell/file/network).
    async fn check_syscall(&self, syscall: &Syscall) -> SecurityDecision;
    /// The current flight-log hash-chain tip (tamper-evidence).
    fn flight_log_hash(&self) -> String;
}

/// A default kernel that allows everything (the stub — hardened later).
///
/// **This is intentionally permissive for the PoC.** The real kernel (T25's
/// follow-up) enforces: default-deny unknown syscalls, taint-aware egress
/// (block Network with UserSecret-tainted args), Ed25519 capability receipts,
/// opaque secret FD handles. For now, the DefaultKernel is the testbed for the
/// trait shape + the FlightRecorder wiring.
pub struct DefaultKernel {
    /// The kernel name.
    pub name: String,
    /// The flight recorder (hash-chained log).
    pub recorder: crate::flight::FlightRecorder,
}

impl DefaultKernel {
    /// Construct a default (allow-all) kernel with an empty flight recorder.
    pub fn new() -> Self {
        Self {
            name: "default".into(),
            recorder: crate::flight::FlightRecorder::new(),
        }
    }
}

impl Default for DefaultKernel {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SecurityKernel for DefaultKernel {
    fn name(&self) -> &str {
        &self.name
    }

    async fn mediate_tool_call(&self, tool: &str, args: &serde_json::Value) -> SecurityDecision {
        // T27 in action: taint-aware egress. If the tool args carry a
        // UserSecret, the default kernel still allows (PoC) but logs the taint
        // to the flight recorder. The strict kernel (later) would block.
        let taint = taint_of(args);
        self.recorder
            .append_tool_call(tool, args, taint)
            .unwrap_or_default();
        SecurityDecision::Allow
    }

    async fn check_syscall(&self, syscall: &Syscall) -> SecurityDecision {
        // Default-deny is the AgentK pattern, but the PoC default-ALLOWS so the
        // agent can actually run. The strict kernel (later) flips this.
        // Taint-aware egress for Network: log the taint (block in strict mode).
        if let Syscall::Network { url } = syscall {
            let taint = if is_secret_bearing_url(url) {
                TaintLabel::UserSecret
            } else {
                TaintLabel::Clean
            };
            self.recorder
                .append_syscall(syscall, taint)
                .unwrap_or_default();
        } else {
            self.recorder
                .append_syscall(syscall, TaintLabel::Clean)
                .unwrap_or_default();
        }
        SecurityDecision::Allow
    }

    fn flight_log_hash(&self) -> String {
        self.recorder.tip_hex()
    }
}

/// Whether a URL looks like it carries a secret (e.g. `?token=...`, `:password@`).
fn is_secret_bearing_url(url: &str) -> bool {
    let lower = url.to_lowercase();
    lower.contains("token=")
        || lower.contains("password=")
        || lower.contains("api_key=")
        || lower.contains("secret=")
        || lower.contains(":password@")
        || lower.contains(":secret@")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T25: the SecurityKernel trait compiles + DefaultKernel constructs +
    /// reports its name + the flight log hash is the zero hash when empty.
    #[tokio::test]
    async fn security_kernel_trait_compiles() {
        let k = DefaultKernel::new();
        assert_eq!(k.name(), "default");
        assert_eq!(SecurityKernel::name(&k), "default"); // via the trait
                                                         // Empty flight recorder -> zero-hash tip.
        assert_eq!(k.flight_log_hash(), crate::flight::ZERO_HASH);
    }

    /// T25: DefaultKernel mediate_tool_call allows (PoC) + records to the flight log.
    #[tokio::test]
    async fn default_kernel_mediate_allows_and_records() {
        let k = DefaultKernel::new();
        let d = k
            .mediate_tool_call("Read", &serde_json::json!({"path": "foo.rs"}))
            .await;
        assert_eq!(d, SecurityDecision::Allow);
        // After one record, the tip is no longer the zero hash.
        assert_ne!(k.flight_log_hash(), crate::flight::ZERO_HASH);
    }

    /// T25: DefaultKernel check_syscall allows (PoC) + records.
    #[tokio::test]
    async fn default_kernel_check_syscall_allows_and_records() {
        let k = DefaultKernel::new();
        let d = k
            .check_syscall(&Syscall::RunShell {
                command: "echo hi".into(),
            })
            .await;
        assert_eq!(d, SecurityDecision::Allow);
        assert_ne!(k.flight_log_hash(), crate::flight::ZERO_HASH);
    }

    /// T27: taint_of marks secret-bearing keys as UserSecret (recursive).
    #[test]
    fn taint_of_marks_secrets() {
        // clean args -> Clean
        assert_eq!(
            taint_of(&serde_json::json!({"path": "foo.rs"})),
            TaintLabel::Clean
        );
        // top-level secret key -> UserSecret
        assert_eq!(
            taint_of(&serde_json::json!({"secret": "x"})),
            TaintLabel::UserSecret
        );
        // case-insensitive
        assert_eq!(
            taint_of(&serde_json::json!({"API_KEY": "x"})),
            TaintLabel::UserSecret
        );
        // nested -> UserSecret
        assert_eq!(
            taint_of(&serde_json::json!({"db": {"password": 123}})),
            TaintLabel::UserSecret
        );
        // token in an array of objects -> UserSecret
        assert_eq!(
            taint_of(&serde_json::json!({"items": [{"token": "abc"}]})),
            TaintLabel::UserSecret
        );
        // nested clean -> Clean
        assert_eq!(
            taint_of(&serde_json::json!({"db": {"host": "localhost", "port": 5432}})),
            TaintLabel::Clean
        );
    }

    /// T27: TaintLabel::most_sensitive ranks UserSecret > UserData > Clean.
    #[test]
    fn taint_label_most_sensitive() {
        assert_eq!(
            TaintLabel::Clean.most_sensitive(TaintLabel::UserSecret),
            TaintLabel::UserSecret
        );
        assert_eq!(
            TaintLabel::UserData.most_sensitive(TaintLabel::Clean),
            TaintLabel::UserData
        );
        assert_eq!(
            TaintLabel::UserSecret.most_sensitive(TaintLabel::UserData),
            TaintLabel::UserSecret
        );
        assert_eq!(
            TaintLabel::Clean.most_sensitive(TaintLabel::Clean),
            TaintLabel::Clean
        );
    }

    /// T27: is_secret_bearing_url flags URLs with token=/password=/etc.
    #[test]
    fn secret_bearing_url_detection() {
        assert!(is_secret_bearing_url("https://x?token=abc"));
        assert!(is_secret_bearing_url("https://u:password@h/path"));
        assert!(is_secret_bearing_url("https://x?API_KEY=abc"));
        assert!(!is_secret_bearing_url("https://x/path"));
        assert!(!is_secret_bearing_url("https://x?q=search"));
    }
}
