//! The permission resolution chain + modes.
//!
//! Clean-room pattern (non-copyrightable): a strict resolution order
//!   hooks -> deny -> ask -> mode -> allow -> callback
//! with `deny > defer > ask > allow` priority (a single deny from any hook blocks).
//!
//! Modes (the user-facing HITL controls, per user decision turn-3):
//!   - Plan       : read-only; file edits + shell writes route to user approval.
//!   - Bypass     : "YOLO" — approve everything reaching the mode step (hooks/deny still apply).
//!   - Approval   : standard HITL — prompt on each consequential action.
//!   - Auto       : an LLM classifier evaluates each call against the transcript.
//!   - Bubble     : sub-agent escalates to parent or user.
//!
//! Sub-agents inherit the parent's mode and cannot widen it.

use serde::{Deserialize, Serialize};

/// The permission resolution result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", content = "reason")]
pub enum Permission {
    /// The action is allowed.
    Allow,
    /// The action is denied, with a reason.
    Deny(String),
    /// The action requires the user to decide.
    Ask,
}

impl Permission {
    /// Most-restrictive-wins: Deny > Ask > Allow.
    pub fn resolve(permissions: Vec<Permission>) -> Permission {
        permissions
            .into_iter()
            .max_by_key(|p| match p {
                Permission::Allow => 0,
                Permission::Ask => 1,
                Permission::Deny(_) => 2,
            })
            .unwrap_or(Permission::Deny("no permission granted".into()))
    }
}

/// The user-facing permission mode.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    /// Read-only; edits + shell writes route to approval.
    Plan,
    /// YOLO — approve everything reaching the mode step (hooks/deny still apply).
    Bypass,
    /// Standard HITL — prompt on each consequential action.
    #[default]
    Approval,
    /// LLM classifier evaluates each call against the transcript.
    Auto,
    /// Sub-agent escalates to parent or user.
    Bubble,
}

/// A pending permission request surfaced to the UI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PermissionRequest {
    /// The tool that wants to act.
    pub tool: String,
    /// The tool's arguments.
    pub args: serde_json::Value,
    /// The permission mode in effect.
    pub mode: PermissionMode,
    /// A human-readable reason for the request.
    pub reason: String,
}

/// The resolver that walks the chain for a given tool call.
pub struct PermissionResolver {
    // TODO(T15): deny rules, ask rules, allow rules, can_use_tool callback.
    /// The active permission mode.
    pub mode: PermissionMode,
}

impl PermissionResolver {
    /// Construct a resolver in the given mode.
    pub fn new(mode: PermissionMode) -> Self {
        Self { mode }
    }

    /// T15: a stub `resolve` that respects the mode for read-only vs write tools.
    ///
    /// Read-only tools (`Read`, `Grep`, `Glob`) are allowed regardless of mode.
    /// Write tools (`Edit`, `Write`, `Bash`) get `Allow` under `Bypass`, `Ask`
    /// under `Approval`/`Plan`. (The full chain — hooks → deny → ask → mode →
    /// allow → callback — is wired in T15's follow-up; this is the mode-step
    /// behavior.)
    pub fn resolve(&self, tool_name: &str) -> Permission {
        const READ_ONLY: &[&str] = &["Read", "Grep", "Glob"];
        if READ_ONLY.contains(&tool_name) {
            return Permission::Allow;
        }
        match self.mode {
            PermissionMode::Bypass => Permission::Allow,
            PermissionMode::Approval | PermissionMode::Plan => Permission::Ask,
            // Auto/Bubble delegate to the classifier/parent in the full impl;
            // for the stub, treat as Ask (safer default).
            PermissionMode::Auto | PermissionMode::Bubble => Permission::Ask,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T04: Permission::resolve — most-restrictive-wins (Deny > Ask > Allow).
    #[test]
    fn permission_resolve_most_restrictive_wins() {
        // empty -> Deny (no permission granted)
        let empty = Permission::resolve(vec![]);
        assert!(matches!(empty, Permission::Deny(_)));

        // all Allow -> Allow
        let all_allow = Permission::resolve(vec![Permission::Allow, Permission::Allow]);
        assert_eq!(all_allow, Permission::Allow);

        // one Deny -> Deny (dominates)
        let one_deny = Permission::resolve(vec![Permission::Allow, Permission::Deny("no".into())]);
        assert!(matches!(one_deny, Permission::Deny(_)));

        // one Ask, no Deny -> Ask
        let one_ask = Permission::resolve(vec![Permission::Allow, Permission::Ask]);
        assert_eq!(one_ask, Permission::Ask);

        // mixed -> Deny dominates
        let mixed = Permission::resolve(vec![
            Permission::Allow,
            Permission::Ask,
            Permission::Deny("x".into()),
        ]);
        assert!(matches!(mixed, Permission::Deny(_)));
    }

    /// T05: PermissionMode defaults to Approval and round-trips through serde as snake_case.
    #[test]
    fn permission_mode_default_and_serde() {
        assert_eq!(PermissionMode::default(), PermissionMode::Approval);

        for (mode, expected) in [
            (PermissionMode::Plan, "\"plan\""),
            (PermissionMode::Bypass, "\"bypass\""),
            (PermissionMode::Approval, "\"approval\""),
            (PermissionMode::Auto, "\"auto\""),
            (PermissionMode::Bubble, "\"bubble\""),
        ] {
            let json = serde_json::to_string(&mode).expect("serialize");
            assert_eq!(json, expected, "serde tag mismatch for {mode:?}");
            let back: PermissionMode = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(back, mode, "round-trip failed for {mode:?}");
        }
    }

    /// T15: PermissionResolver.resolve respects mode for read-only vs write tools.
    #[test]
    fn resolver_modes_respect_read_only_and_writes() {
        // Approval + Edit -> Ask
        let app = PermissionResolver::new(PermissionMode::Approval);
        assert_eq!(app.resolve("Edit"), Permission::Ask);
        // Approval + Read -> Allow (read-only always allowed)
        assert_eq!(app.resolve("Read"), Permission::Allow);
        assert_eq!(app.resolve("Grep"), Permission::Allow);
        assert_eq!(app.resolve("Glob"), Permission::Allow);

        // Bypass + Edit -> Allow (YOLO)
        let yolo = PermissionResolver::new(PermissionMode::Bypass);
        assert_eq!(yolo.resolve("Edit"), Permission::Allow);
        assert_eq!(yolo.resolve("Bash"), Permission::Allow);

        // Plan + Read -> Allow; Plan + Edit -> Ask
        let plan = PermissionResolver::new(PermissionMode::Plan);
        assert_eq!(plan.resolve("Read"), Permission::Allow);
        assert_eq!(plan.resolve("Edit"), Permission::Ask);
    }
}
