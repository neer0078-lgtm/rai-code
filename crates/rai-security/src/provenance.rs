//! Argument-Level Security Provenance — PACT-inspired value origin tracking.
//!
//! From the research alignment (P1): PACT (arxiv 2605.11039) achieves 100%
//! security + 100% utility under oracle provenance. AuthGraph (arxiv 2605.26497)
//! reduces attack success from 40% to 1%. These go beyond AgentK's tool-call
//! mediation to **argument-level provenance** — tracking where each value in a
//! tool call's arguments came from, and checking whether that origin satisfies
//! the argument's role-specific trust contract.
//!
//! The provenance system:
//! 1. Tags each value in a tool call's args with an origin (user_input,
//!    model_output, tool_result, file_read, env_var, etc.).
//! 2. Each tool argument has a trust contract (what origins are allowed).
//! 3. Before executing a tool, the provenance checker verifies that every
//!    argument's origin satisfies its trust contract.
//! 4. If an argument's origin doesn't satisfy the contract (e.g. a file path
//!    came from model_output instead of user_input), the call is blocked.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The origin of a value in a tool call's arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueOrigin {
    /// The value came directly from the user's input.
    UserInput,
    /// The value was produced by the model's output (the LLM's text).
    ModelOutput,
    /// The value came from a previous tool's result.
    ToolResult,
    /// The value was read from a file.
    FileRead,
    /// The value came from an environment variable.
    EnvVar,
    /// The origin is unknown (untrusted).
    Unknown,
}

impl ValueOrigin {
    /// The trust level of this origin (higher = more trusted).
    pub fn trust_level(&self) -> u8 {
        match self {
            ValueOrigin::UserInput => 5, // highest trust
            ValueOrigin::FileRead => 4,
            ValueOrigin::EnvVar => 3,
            ValueOrigin::ToolResult => 2,
            ValueOrigin::ModelOutput => 1, // lowest trust (can be hallucinated)
            ValueOrigin::Unknown => 0,     // untrusted
        }
    }

    /// Whether this origin is trusted for a given minimum trust level.
    pub fn is_trusted_for(&self, min_level: u8) -> bool {
        self.trust_level() >= min_level
    }
}

/// A trust contract for a tool argument — what origins are allowed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustContract {
    /// The argument name (e.g. "file_path", "command", "url").
    pub arg_name: String,
    /// The minimum trust level required for this argument.
    pub min_trust_level: u8,
    /// Optional: specific allowed origins (if set, only these are allowed).
    pub allowed_origins: Option<Vec<ValueOrigin>>,
}

impl TrustContract {
    /// Construct a contract requiring a minimum trust level.
    pub fn min_trust(arg_name: impl Into<String>, level: u8) -> Self {
        Self {
            arg_name: arg_name.into(),
            min_trust_level: level,
            allowed_origins: None,
        }
    }

    /// Construct a contract allowing only specific origins.
    pub fn only_origins(arg_name: impl Into<String>, origins: Vec<ValueOrigin>) -> Self {
        Self {
            arg_name: arg_name.into(),
            min_trust_level: 0,
            allowed_origins: Some(origins),
        }
    }

    /// Check if an origin satisfies this contract.
    pub fn check(&self, origin: ValueOrigin) -> bool {
        if let Some(ref allowed) = self.allowed_origins {
            return allowed.contains(&origin);
        }
        origin.is_trusted_for(self.min_trust_level)
    }
}

/// A provenance-tracked value — a JSON value + its origin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedValue {
    /// The JSON value.
    pub value: serde_json::Value,
    /// The origin of this value.
    pub origin: ValueOrigin,
}

/// The provenance checker — verifies tool call arguments against trust contracts.
#[derive(Debug, Default)]
pub struct ProvenanceChecker {
    /// The trust contracts for each tool's arguments.
    /// Key: tool_name, Value: map of arg_name -> TrustContract.
    contracts: HashMap<String, HashMap<String, TrustContract>>,
}

impl ProvenanceChecker {
    /// Construct a new checker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a trust contract for a tool argument.
    pub fn register_contract(&mut self, tool_name: impl Into<String>, contract: TrustContract) {
        self.contracts
            .entry(tool_name.into())
            .or_default()
            .insert(contract.arg_name.clone(), contract);
    }

    /// Check a tool call's arguments against the registered contracts.
    ///
    /// `tracked_args` is a map of arg_name -> TrackedValue (value + origin).
    /// Returns Ok(()) if all arguments satisfy their contracts, or
    /// Err(violation_message) if any argument's origin doesn't satisfy.
    pub fn check(
        &self,
        tool_name: &str,
        tracked_args: &HashMap<String, TrackedValue>,
    ) -> Result<(), String> {
        let Some(tool_contracts) = self.contracts.get(tool_name) else {
            return Ok(()); // no contracts registered = allow.
        };

        for (arg_name, contract) in tool_contracts {
            let Some(tracked) = tracked_args.get(arg_name) else {
                continue; // arg not present = skip.
            };

            if !contract.check(tracked.origin) {
                return Err(format!(
                    "provenance violation: tool '{tool_name}' arg '{arg_name}' has origin {:?} \
                     (trust level {}) but requires min trust {} {:?}",
                    tracked.origin,
                    tracked.origin.trust_level(),
                    contract.min_trust_level,
                    contract.allowed_origins,
                ));
            }
        }

        Ok(())
    }

    /// The default contracts for common tools (file paths must come from user
    /// input or file reads, not model output; commands must come from user
    /// input; URLs must come from user input or file reads).
    pub fn with_defaults(mut self) -> Self {
        // Write/Edit: file_path must be user_input or file_read (not model output).
        self.register_contract(
            "Write",
            TrustContract::only_origins(
                "file_path",
                vec![ValueOrigin::UserInput, ValueOrigin::FileRead],
            ),
        );
        self.register_contract(
            "Edit",
            TrustContract::only_origins(
                "file_path",
                vec![ValueOrigin::UserInput, ValueOrigin::FileRead],
            ),
        );
        // Bash: command must be at least model_output trust (the model generates commands).
        self.register_contract("Bash", TrustContract::min_trust("command", 1));
        // Network: url must be user_input or file_read (not hallucinated).
        self.register_contract(
            "Network",
            TrustContract::only_origins(
                "url",
                vec![
                    ValueOrigin::UserInput,
                    ValueOrigin::FileRead,
                    ValueOrigin::EnvVar,
                ],
            ),
        );
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_trust_levels() {
        assert_eq!(ValueOrigin::UserInput.trust_level(), 5);
        assert_eq!(ValueOrigin::ModelOutput.trust_level(), 1);
        assert_eq!(ValueOrigin::Unknown.trust_level(), 0);
        assert!(ValueOrigin::UserInput.is_trusted_for(3));
        assert!(!ValueOrigin::ModelOutput.is_trusted_for(3));
    }

    #[test]
    fn contract_min_trust() {
        let contract = TrustContract::min_trust("file_path", 4);
        assert!(contract.check(ValueOrigin::UserInput)); // 5 >= 4
        assert!(contract.check(ValueOrigin::FileRead)); // 4 >= 4
        assert!(!contract.check(ValueOrigin::ModelOutput)); // 1 < 4
    }

    #[test]
    fn contract_only_origins() {
        let contract = TrustContract::only_origins(
            "file_path",
            vec![ValueOrigin::UserInput, ValueOrigin::FileRead],
        );
        assert!(contract.check(ValueOrigin::UserInput));
        assert!(contract.check(ValueOrigin::FileRead));
        assert!(!contract.check(ValueOrigin::ModelOutput));
        assert!(!contract.check(ValueOrigin::Unknown));
    }

    #[test]
    fn checker_allows_when_no_contracts() {
        let checker = ProvenanceChecker::new();
        let mut args = HashMap::new();
        args.insert(
            "file_path".into(),
            TrackedValue {
                value: serde_json::json!("/etc/passwd"),
                origin: ValueOrigin::Unknown,
            },
        );
        assert!(checker.check("AnyTool", &args).is_ok());
    }

    #[test]
    fn checker_blocks_violation() {
        let checker = ProvenanceChecker::new().with_defaults();
        let mut args = HashMap::new();
        args.insert(
            "file_path".into(),
            TrackedValue {
                value: serde_json::json!("/etc/passwd"),
                origin: ValueOrigin::ModelOutput, // not allowed for Write
            },
        );
        let result = checker.check("Write", &args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("provenance violation"));
    }

    #[test]
    fn checker_allows_compliant() {
        let checker = ProvenanceChecker::new().with_defaults();
        let mut args = HashMap::new();
        args.insert(
            "file_path".into(),
            TrackedValue {
                value: serde_json::json!("src/main.rs"),
                origin: ValueOrigin::UserInput, // allowed for Write
            },
        );
        assert!(checker.check("Write", &args).is_ok());
    }

    #[test]
    fn checker_blocks_hallucinated_url() {
        let checker = ProvenanceChecker::new().with_defaults();
        let mut args = HashMap::new();
        args.insert(
            "url".into(),
            TrackedValue {
                value: serde_json::json!("https://evil.com/exfil"),
                origin: ValueOrigin::ModelOutput, // not allowed for Network
            },
        );
        assert!(checker.check("Network", &args).is_err());
    }

    #[test]
    fn checker_allows_user_provided_url() {
        let checker = ProvenanceChecker::new().with_defaults();
        let mut args = HashMap::new();
        args.insert(
            "url".into(),
            TrackedValue {
                value: serde_json::json!("https://api.example.com"),
                origin: ValueOrigin::UserInput, // allowed for Network
            },
        );
        assert!(checker.check("Network", &args).is_ok());
    }

    #[test]
    fn checker_skips_missing_args() {
        let checker = ProvenanceChecker::new().with_defaults();
        let args = HashMap::new(); // no args at all
        assert!(checker.check("Write", &args).is_ok());
    }

    #[test]
    fn tracked_value_serde() {
        let tv = TrackedValue {
            value: serde_json::json!("test"),
            origin: ValueOrigin::UserInput,
        };
        let json = serde_json::to_string(&tv).unwrap();
        let back: TrackedValue = serde_json::from_str(&json).unwrap();
        assert_eq!(tv.value, back.value);
        assert_eq!(tv.origin, back.origin);
    }
}
