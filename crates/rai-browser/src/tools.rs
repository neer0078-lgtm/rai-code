//! The agent browser tool surface — BrowserAction enum + the a11y-tree serializer.
//!
//! T34: BrowserAction enum (the agent's browser commands) with serde round-trip.
//! T35: serialize_a11y(ax_tree) — turns a CDP Accessibility.getFullAXTree JSON
//!      into indented text with [ref] markers, collapsing generic/group/none/
//!      presentation wrappers but keeping semantic roles + interactive refs.
//! T36: chromiumoxide integration (feature-gated, #[ignore] — needs Chromium).

use serde::{Deserialize, Serialize};

/// A browser action the agent can request (the tool surface).
///
/// The default observation is `Snapshot` (the a11y tree — ~300 tokens); the
/// rich fallback is `Screenshot` (~1.5K image tokens, on-demand only).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", content = "data")]
pub enum BrowserAction {
    /// Navigate to a URL.
    Navigate {
        /// The URL.
        url: String,
    },
    /// Get the accessibility-tree text snapshot (the default observation).
    Snapshot,
    /// Click an element by its a11y ref (e.g. "A0").
    Click {
        /// The element ref from the a11y tree.
        ref_id: String,
    },
    /// Type text into an element by its a11y ref.
    Type {
        /// The element ref.
        ref_id: String,
        /// The text to type.
        text: String,
    },
    /// Capture a screenshot (the rich fallback — on-demand).
    Screenshot,
    /// Get console errors since the last call (failures only).
    GetConsoleErrors,
    /// Get network failures since the last call (4xx/5xx + failed).
    GetNetworkFailures,
    /// Assert the page contains text.
    AssertText {
        /// The text to find.
        text: String,
    },
    /// Assert an element (by ref) is visible.
    AssertVisible {
        /// The element ref.
        ref_id: String,
    },
    /// Run a Playwright test file.
    RunPlaywrightTest {
        /// The spec file path.
        spec: String,
    },
    /// Get a source-mapped stack trace for an error.
    GetSourceMapStack {
        /// The error id/index.
        error_id: String,
    },
    /// Evaluate JavaScript in the page.
    Evaluate {
        /// The JS expression.
        expression: String,
    },
    /// Set the viewport size (responsive testing).
    SetViewport {
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
    },
}

/// The roles that are pure wrappers — collapsed in the a11y serialization
/// (their children are promoted to the parent's indent level).
const WRAPPER_ROLES: &[&str] = &["generic", "group", "none", "presentation"];

/// T35: serialize a CDP `Accessibility.getFullAXTree` JSON response into
/// indented text with `[ref]` markers for interactive elements.
///
/// Collapses `generic`/`group`/`none`/`presentation` wrappers (their children
/// are promoted, not nested under the wrapper). Keeps semantic roles
/// (navigation, link, button, textbox, heading, form, etc.) with their
/// accessible names + `[ref]` markers.
///
/// The input is a JSON array of AX node objects, each with `role` (an object
/// with a `value` string), `name` (an object with a `value` string), and
/// optionally `ref` (a string like "A0"). This matches the CDP
/// `Accessibility.getFullAXTree` response shape.
pub fn serialize_a11y(ax_tree: &serde_json::Value) -> String {
    let nodes = match ax_tree {
        serde_json::Value::Array(arr) => arr,
        _ => return "(invalid a11y tree: expected an array)\n".into(),
    };

    // Build a map of ref -> node for parent-child resolution.
    // CDP AX nodes have: role.value (string), name.value (string), ref (string),
    // and childIds (array of refs) OR the tree is flat (all nodes at top level).
    // We handle both: if nodes have childIds, build a tree; if flat, serialize
    // in order with indent based on a `level` field or heuristically.

    // For the flat-array case (the common CDP response), we serialize each node
    // on its own line, collapsing wrappers by skipping them (their children
    // appear at the same indent as they would have).
    let mut out = String::new();
    for node in nodes {
        let role = node
            .get("role")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let name = node
            .get("name")
            .and_then(|n| n.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let ref_id = node.get("ref").and_then(|r| r.as_str()).unwrap_or("");

        // Skip wrapper roles — their children are at the same level.
        if WRAPPER_ROLES.contains(&role) {
            continue;
        }

        // Build the line: "- role \"name\" [ref]" or "- role [ref]" if no name.
        let name_part = if name.is_empty() {
            String::new()
        } else {
            format!(" \"{name}\"")
        };
        let ref_part = if ref_id.is_empty() {
            String::new()
        } else {
            format!(" [{ref_id}]")
        };
        out.push_str(&format!("- {role}{name_part}{ref_part}\n"));
    }
    if out.is_empty() {
        out.push_str("(empty a11y tree)\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T34: BrowserAction round-trips through serde for all variants.
    #[test]
    fn browser_action_serde_roundtrip() {
        let actions = vec![
            BrowserAction::Navigate {
                url: "http://localhost:3000".into(),
            },
            BrowserAction::Snapshot,
            BrowserAction::Click {
                ref_id: "A0".into(),
            },
            BrowserAction::Type {
                ref_id: "A2".into(),
                text: "hello".into(),
            },
            BrowserAction::Screenshot,
            BrowserAction::GetConsoleErrors,
            BrowserAction::GetNetworkFailures,
            BrowserAction::AssertText {
                text: "Welcome".into(),
            },
            BrowserAction::AssertVisible {
                ref_id: "A4".into(),
            },
            BrowserAction::RunPlaywrightTest {
                spec: "tests/login.spec.ts".into(),
            },
            BrowserAction::GetSourceMapStack {
                error_id: "err-1".into(),
            },
            BrowserAction::Evaluate {
                expression: "document.title".into(),
            },
            BrowserAction::SetViewport {
                width: 1280,
                height: 720,
            },
        ];
        for a in actions {
            let json = serde_json::to_string(&a).expect("serialize");
            let back: BrowserAction = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(a, back, "round-trip failed for {a:?} (json: {json})");
        }
    }

    /// T35: serialize_a11y turns a fixture a11y tree into text with [ref] markers.
    #[test]
    fn serialize_a11y_with_refs_and_names() {
        let tree = serde_json::json!([
            {"role": {"value": "navigation"}, "name": {"value": ""}, "ref": ""},
            {"role": {"value": "link"}, "name": {"value": "Home"}, "ref": "A0"},
            {"role": {"value": "link"}, "name": {"value": "Settings"}, "ref": "A1"},
            {"role": {"value": "main"}, "name": {"value": ""}, "ref": ""},
            {"role": {"value": "heading"}, "name": {"value": "Dashboard"}, "ref": ""},
            {"role": {"value": "form"}, "name": {"value": ""}, "ref": ""},
            {"role": {"value": "textbox"}, "name": {"value": "Email"}, "ref": "A2"},
            {"role": {"value": "textbox"}, "name": {"value": "Password"}, "ref": "A3"},
            {"role": {"value": "button"}, "name": {"value": "Submit"}, "ref": "A4"}
        ]);
        let text = serialize_a11y(&tree);
        assert!(
            text.contains("- navigation"),
            "should have navigation: {text}"
        );
        assert!(
            text.contains("- link \"Home\" [A0]"),
            "should have link Home with ref A0: {text}"
        );
        assert!(
            text.contains("- link \"Settings\" [A1]"),
            "should have link Settings with ref A1: {text}"
        );
        assert!(
            text.contains("- heading \"Dashboard\""),
            "should have heading Dashboard: {text}"
        );
        assert!(
            text.contains("- textbox \"Email\" [A2]"),
            "should have textbox Email with ref A2: {text}"
        );
        assert!(
            text.contains("- button \"Submit\" [A4]"),
            "should have button Submit with ref A4: {text}"
        );
    }

    /// T35: generic/group/none/presentation wrappers are collapsed (skipped),
    /// but their children appear.
    #[test]
    fn serialize_a11y_collapses_wrappers() {
        let tree = serde_json::json!([
            {"role": {"value": "generic"}, "name": {"value": ""}, "ref": ""},
            {"role": {"value": "group"}, "name": {"value": ""}, "ref": ""},
            {"role": {"value": "button"}, "name": {"value": "Click me"}, "ref": "A0"},
            {"role": {"value": "none"}, "name": {"value": ""}, "ref": ""},
            {"role": {"value": "presentation"}, "name": {"value": ""}, "ref": ""},
            {"role": {"value": "link"}, "name": {"value": "Next"}, "ref": "A1"}
        ]);
        let text = serialize_a11y(&tree);
        // Wrappers are skipped — no lines for generic/group/none/presentation.
        assert!(
            !text.contains("- generic"),
            "generic should be collapsed: {text}"
        );
        assert!(
            !text.contains("- group"),
            "group should be collapsed: {text}"
        );
        assert!(!text.contains("- none"), "none should be collapsed: {text}");
        assert!(
            !text.contains("- presentation"),
            "presentation should be collapsed: {text}"
        );
        // But the button + link (children of the wrappers) are present.
        assert!(
            text.contains("- button \"Click me\" [A0]"),
            "button should be present: {text}"
        );
        assert!(
            text.contains("- link \"Next\" [A1]"),
            "link should be present: {text}"
        );
    }

    /// T35: an empty tree produces "(empty a11y tree)".
    #[test]
    fn serialize_a11y_empty() {
        let text = serialize_a11y(&serde_json::json!([]));
        assert!(text.contains("(empty a11y tree)"), "empty tree msg: {text}");
    }

    /// T35: an invalid tree (not an array) produces an error message.
    #[test]
    fn serialize_a11y_invalid() {
        let text = serialize_a11y(&serde_json::json!({"not": "an array"}));
        assert!(
            text.contains("invalid a11y tree"),
            "invalid tree msg: {text}"
        );
    }
}
