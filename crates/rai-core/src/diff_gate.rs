//! The Diff Gate — scope enforcement that rejects writes to files outside the
//! task's scope. Addresses the #1 user complaint: over-editing (1.7-3.4x ratio,
//! ~11% bug-introducing PRs, NovVista Apr 2026).
//!
//! The gate works by:
//! 1. Extracting named entities (files, functions, symbols) from the user's
//!    request → the "in-scope" set.
//! 2. Marking everything else as read-only.
//! 3. When a write tool (Edit/Write/Bash) targets a file outside the scope,
//!    the gate rejects it with an error the agent can react to.
//! 4. If the agent genuinely needs to edit outside scope, it surfaces as a
//!    proposal (not a silent action) — the user is asked to approve.
//!
//! The gate is a pluggable layer between the permission check and the tool
//! execution. It's inspired by Aider's `/add` vs `/read-only` split and the
//! NovVista finding that "the first product to ship a genuinely scope-
//! disciplined agent loop will capture a significant share of real production
//! use cases."

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// The scope of a task — which files/symbols are in-scope for editing.
///
/// Extracted from the user's request (the files they named, the functions
/// they mentioned) + optionally expanded by the agent (with user approval).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskScope {
    /// The files explicitly in-scope (writable).
    in_scope_files: HashSet<String>,
    /// The files explicitly out-of-scope (read-only).
    /// If empty, all files not in `in_scope_files` are considered out-of-scope.
    out_of_scope_files: HashSet<String>,
    /// Whether the scope is "open" (any file is writable). Default false.
    /// Set to true in Bypass mode or when the user explicitly says "edit anything".
    pub open_scope: bool,
}

impl TaskScope {
    /// Construct a new, closed scope (no files in-scope by default).
    pub fn new() -> Self {
        Self::default()
    }

    /// Construct an open scope (all files writable — for Bypass mode).
    pub fn open() -> Self {
        Self {
            open_scope: true,
            ..Default::default()
        }
    }

    /// Add a file to the in-scope (writable) set.
    pub fn allow_file(&mut self, path: impl Into<String>) {
        self.in_scope_files.insert(path.into());
    }

    /// Add multiple files to the in-scope set.
    pub fn allow_files<I, S>(&mut self, paths: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for p in paths {
            self.in_scope_files.insert(p.into());
        }
    }

    /// Explicitly mark a file as out-of-scope (read-only).
    pub fn deny_file(&mut self, path: impl Into<String>) {
        self.out_of_scope_files.insert(path.into());
    }

    /// Check if a file path is in-scope (writable).
    ///
    /// Rules (in order):
    /// 1. If open_scope, everything is in-scope.
    /// 2. If the file is in out_of_scope_files, it's out-of-scope (explicit deny wins).
    /// 3. If the file is in in_scope_files, it's in-scope.
    /// 4. Otherwise: if in_scope_files is non-empty, the file is out-of-scope
    ///    (closed scope — only explicitly allowed files are writable).
    ///    If in_scope_files is empty, the file is in-scope (no scope set = open).
    pub fn is_in_scope(&self, path: &str) -> bool {
        if self.open_scope {
            return true;
        }
        // Normalize the path for comparison.
        let normalized = normalize_path(path);
        if self.out_of_scope_files.contains(&normalized) {
            return false;
        }
        if self.in_scope_files.contains(&normalized) {
            return true;
        }
        // Check if any in-scope file is a parent directory of this path.
        for scope_file in &self.in_scope_files {
            if normalized.starts_with(scope_file) {
                return true;
            }
        }
        // Closed scope: if there are in-scope files, anything else is out-of-scope.
        // Returns false (out-of-scope) when there are in-scope files but this file isn't one.
        // Returns true (in-scope) when there are NO in-scope files (empty scope = open).
        self.in_scope_files.is_empty()
    }

    /// Whether a tool is a "write" tool (subject to the gate).
    pub fn is_write_tool(tool_name: &str) -> bool {
        matches!(
            tool_name,
            "Write" | "Edit" | "MultiEdit" | "Bash" | "NotebookEdit"
        )
    }

    /// Extract the file path from a tool call's arguments.
    /// Returns None if the tool call doesn't target a file.
    pub fn extract_file_path(tool_name: &str, args: &serde_json::Value) -> Option<String> {
        match tool_name {
            "Write" | "Edit" | "MultiEdit" | "NotebookEdit" => args
                .get("file_path")
                .or_else(|| args.get("path"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            "Bash" => {
                // For Bash, extract file paths from the command (heuristic).
                // This is conservative — only flags obvious file writes.
                args.get("command")
                    .and_then(|v| v.as_str())
                    .and_then(extract_file_from_command)
            }
            _ => None,
        }
    }
}

/// The diff gate's decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum GateDecision {
    /// The write is allowed (the file is in-scope).
    Allow,
    /// The write is denied (the file is out-of-scope), with a reason.
    Deny(String),
    /// The write needs user approval (the file is outside the original scope
    /// but the agent is requesting an expansion).
    Propose(String),
}

/// The diff gate — checks a write tool call against the task scope.
pub struct DiffGate {
    /// The task scope.
    pub scope: TaskScope,
}

impl DiffGate {
    /// Construct a gate with the given scope.
    pub fn new(scope: TaskScope) -> Self {
        Self { scope }
    }

    /// Construct an open gate (allows everything — for Bypass mode).
    pub fn open() -> Self {
        Self::new(TaskScope::open())
    }

    /// Check a tool call against the gate.
    ///
    /// Returns:
    /// - `Allow` if the tool is not a write tool, or the file is in-scope.
    /// - `Deny(reason)` if the file is explicitly out-of-scope.
    /// - `Propose(reason)` if the file is outside the scope but not explicitly denied
    ///   (the agent can request a scope expansion).
    pub fn check(&self, tool_name: &str, args: &serde_json::Value) -> GateDecision {
        // Non-write tools are always allowed.
        if !TaskScope::is_write_tool(tool_name) {
            return GateDecision::Allow;
        }

        // Extract the target file path.
        let Some(file_path) = TaskScope::extract_file_path(tool_name, args) else {
            // Can't determine the target — allow (conservative).
            return GateDecision::Allow;
        };

        if self.scope.is_in_scope(&file_path) {
            GateDecision::Allow
        } else {
            // The file is out-of-scope.
            // If the scope is closed (has in-scope files) but not explicitly denied,
            // it's a "propose" (the agent can request expansion).
            let normalized = normalize_path(&file_path);
            let explicitly_denied = self.scope.out_of_scope_files.contains(&normalized);

            if explicitly_denied {
                GateDecision::Deny(format!(
                    "file '{file_path}' is explicitly marked read-only (out of task scope)"
                ))
            } else {
                GateDecision::Propose(format!(
                    "file '{file_path}' is outside the task scope. \
                     In-scope files: {}. \
                     If this edit is necessary, request a scope expansion.",
                    self.scope
                        .in_scope_files
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            }
        }
    }

    /// Expand the scope to include a new file (after user approval of a Propose).
    pub fn expand_scope(&mut self, path: impl Into<String>) {
        self.scope.allow_file(path);
    }
}

/// Normalize a file path for comparison (trim, strip leading ./).
fn normalize_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.starts_with("./") {
        trimmed.strip_prefix("./").unwrap_or(trimmed).to_string()
    } else {
        trimmed.to_string()
    }
}

/// Heuristically extract a file path from a shell command (conservative).
/// Only flags obvious file writes (>, >>, tee, sed -i, etc.).
fn extract_file_from_command(command: &str) -> Option<String> {
    // Look for redirect patterns: `> file`, `>> file`, `tee file`
    let tokens: Vec<&str> = command.split_whitespace().collect();
    for (i, token) in tokens.iter().enumerate() {
        if (*token == ">" || *token == ">>") && i + 1 < tokens.len() {
            return Some(tokens[i + 1].to_string());
        }
        if token.starts_with("tee") && i + 1 < tokens.len() {
            return Some(tokens[i + 1].to_string());
        }
        if token == &"sed" && tokens.iter().any(|t| t == &"-i") {
            // `sed -i 's/old/new/' file` — the last token is usually the file.
            return tokens.last().map(|s| s.to_string());
        }
    }
    None
}

/// Extract the task scope from a user's natural-language request.
///
/// This is a heuristic entity extractor — it looks for file paths, function
/// names, and symbol references in the user's text. A real impl would use an
/// LLM or a proper NER model; this is the deterministic baseline.
pub fn extract_scope_from_request(request: &str) -> TaskScope {
    let mut scope = TaskScope::new();

    // Look for file paths (strings ending in .rs, .py, .ts, .js, .go, .java, etc.).
    let extensions = [
        ".rs", ".py", ".ts", ".tsx", ".js", ".jsx", ".go", ".java", ".c", ".cpp", ".h", ".hpp",
        ".rb", ".php", ".swift", ".kt", ".scala", ".lua", ".sh", ".toml", ".yaml", ".yml", ".json",
        ".md", ".txt", ".sql", ".html", ".css", ".vue", ".svelte",
    ];
    for word in request.split_whitespace() {
        let cleaned = word.trim_matches(|c: char| {
            !c.is_alphanumeric() && c != '.' && c != '/' && c != '-' && c != '_'
        });
        for ext in &extensions {
            if cleaned.ends_with(ext) {
                scope.allow_file(normalize_path(cleaned));
                break;
            }
        }
    }

    // Look for directory paths (strings containing / that don't end in an extension).
    for word in request.split_whitespace() {
        let cleaned = word.trim_matches(|c: char| {
            !c.is_alphanumeric() && c != '/' && c != '.' && c != '-' && c != '_'
        });
        if cleaned.contains('/') && !cleaned.starts_with("http") {
            let has_ext = extensions.iter().any(|e| cleaned.ends_with(e));
            if !has_ext && cleaned.len() > 2 {
                // It's a directory path — allow all files under it.
                scope.allow_file(normalize_path(cleaned));
            }
        }
    }

    // If no files were found, the scope is open (can't determine scope from the request).
    if scope.in_scope_files.is_empty() {
        scope.open_scope = true;
    }

    scope
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Basic: a file in the in-scope set is allowed.
    #[test]
    fn gate_allows_in_scope_file() {
        let mut scope = TaskScope::new();
        scope.allow_file("src/main.rs");
        let gate = DiffGate::new(scope);
        let decision = gate.check("Write", &serde_json::json!({"file_path": "src/main.rs"}));
        assert_eq!(decision, GateDecision::Allow);
    }

    /// A file NOT in the in-scope set is proposed (not silently allowed).
    #[test]
    fn gate_proposes_out_of_scope_file() {
        let mut scope = TaskScope::new();
        scope.allow_file("src/main.rs");
        let gate = DiffGate::new(scope);
        let decision = gate.check("Write", &serde_json::json!({"file_path": "src/other.rs"}));
        assert!(matches!(decision, GateDecision::Propose(_)));
    }

    /// An explicitly denied file is rejected.
    #[test]
    fn gate_denies_explicitly_denied_file() {
        let mut scope = TaskScope::new();
        scope.allow_file("src/main.rs");
        scope.deny_file("src/config.rs");
        let gate = DiffGate::new(scope);
        let decision = gate.check("Write", &serde_json::json!({"file_path": "src/config.rs"}));
        assert!(matches!(decision, GateDecision::Deny(_)));
    }

    /// An open scope allows everything.
    #[test]
    fn gate_open_scope_allows_all() {
        let gate = DiffGate::open();
        let decision = gate.check("Write", &serde_json::json!({"file_path": "any/file.rs"}));
        assert_eq!(decision, GateDecision::Allow);
    }

    /// Non-write tools (Read, Grep) are always allowed.
    #[test]
    fn gate_allows_read_tools() {
        let mut scope = TaskScope::new();
        scope.allow_file("src/main.rs");
        let gate = DiffGate::new(scope);
        let decision = gate.check("Read", &serde_json::json!({"file_path": "src/other.rs"}));
        assert_eq!(decision, GateDecision::Allow);
    }

    /// Path normalization: ./src/main.rs == src/main.rs.
    #[test]
    fn gate_normalizes_paths() {
        let mut scope = TaskScope::new();
        scope.allow_file("src/main.rs");
        let gate = DiffGate::new(scope);
        let decision = gate.check("Write", &serde_json::json!({"file_path": "./src/main.rs"}));
        assert_eq!(decision, GateDecision::Allow);
    }

    /// Directory in-scope: allowing "src/" allows "src/anything.rs".
    #[test]
    fn gate_directory_scope() {
        let mut scope = TaskScope::new();
        scope.allow_file("src");
        let gate = DiffGate::new(scope);
        let decision = gate.check(
            "Write",
            &serde_json::json!({"file_path": "src/deep/nested/file.rs"}),
        );
        assert_eq!(decision, GateDecision::Allow);
    }

    /// Expand scope after user approves a proposal.
    #[test]
    fn gate_expand_scope() {
        let mut scope = TaskScope::new();
        scope.allow_file("src/main.rs");
        let mut gate = DiffGate::new(scope);

        // Initially out-of-scope.
        let decision = gate.check("Write", &serde_json::json!({"file_path": "src/utils.rs"}));
        assert!(matches!(decision, GateDecision::Propose(_)));

        // Expand scope.
        gate.expand_scope("src/utils.rs");

        // Now allowed.
        let decision = gate.check("Write", &serde_json::json!({"file_path": "src/utils.rs"}));
        assert_eq!(decision, GateDecision::Allow);
    }

    /// Bash with a redirect is checked.
    #[test]
    fn gate_checks_bash_redirect() {
        let mut scope = TaskScope::new();
        scope.allow_file("src/main.rs");
        let gate = DiffGate::new(scope);
        let decision = gate.check(
            "Bash",
            &serde_json::json!({"command": "echo hi > src/other.rs"}),
        );
        assert!(matches!(decision, GateDecision::Propose(_)));
    }

    /// Bash with a redirect to an in-scope file is allowed.
    #[test]
    fn gate_allows_bash_in_scope_redirect() {
        let mut scope = TaskScope::new();
        scope.allow_file("src/main.rs");
        let gate = DiffGate::new(scope);
        let decision = gate.check(
            "Bash",
            &serde_json::json!({"command": "echo hi > src/main.rs"}),
        );
        assert_eq!(decision, GateDecision::Allow);
    }

    /// extract_scope_from_request finds file paths in natural language.
    #[test]
    fn extract_scope_finds_files() {
        let scope =
            extract_scope_from_request("fix the bug in src/main.rs and update src/utils/helper.py");
        assert!(scope.is_in_scope("src/main.rs"));
        assert!(scope.is_in_scope("src/utils/helper.py"));
        assert!(!scope.is_in_scope("src/other.rs"));
    }

    /// extract_scope_from_request with no file references → open scope.
    #[test]
    fn extract_scope_no_files_open() {
        let scope = extract_scope_from_request("help me refactor the auth module");
        assert!(scope.open_scope);
    }

    /// extract_scope_from_request with a directory path.
    #[test]
    fn extract_scope_finds_directory() {
        let scope = extract_scope_from_request("update all files in src/auth/");
        assert!(scope.is_in_scope("src/auth/middleware.rs"));
        assert!(scope.is_in_scope("src/auth/routes.rs"));
    }

    /// Empty scope (no in-scope files, not open) → everything is in-scope (safe default).
    #[test]
    fn empty_scope_allows_all() {
        let scope = TaskScope::new();
        assert!(scope.is_in_scope("any/file.rs"));
    }

    /// Closed scope with multiple files.
    #[test]
    fn closed_scope_multiple_files() {
        let mut scope = TaskScope::new();
        scope.allow_files(["src/main.rs", "src/lib.rs", "tests/integration.rs"]);
        assert!(scope.is_in_scope("src/main.rs"));
        assert!(scope.is_in_scope("src/lib.rs"));
        assert!(scope.is_in_scope("tests/integration.rs"));
        assert!(!scope.is_in_scope("src/other.rs"));
        assert!(!scope.is_in_scope("Cargo.toml"));
    }

    /// The gate's Propose message includes the in-scope files for context.
    #[test]
    fn propose_message_includes_scope() {
        let mut scope = TaskScope::new();
        scope.allow_file("src/main.rs");
        let gate = DiffGate::new(scope);
        let decision = gate.check("Write", &serde_json::json!({"file_path": "src/other.rs"}));
        if let GateDecision::Propose(msg) = decision {
            assert!(
                msg.contains("src/main.rs"),
                "propose message should list in-scope files: {msg}"
            );
            assert!(
                msg.contains("src/other.rs"),
                "propose message should name the out-of-scope file: {msg}"
            );
        } else {
            panic!("expected Propose");
        }
    }
}
