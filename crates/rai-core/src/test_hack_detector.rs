//! The Test-Hacking Detector — rejects superficial verifiers in ITVF's Verify
//! stage. Prevents false "goal matched" when tests are gamed rather than
//! genuinely passed.
//!
//! From the research alignment (P0): SWE-Universe (arxiv 2602.02361) found
//! that agents can "hack" tests — making them pass without actually fixing the
//! bug. TDFlow found only 7/800 test-hacking instances were caught by standard
//! verifiers. The detector implements three checks:
//!
//! 1. **Static-matching detection**: rejects tests that use static string
//!    matching (assert!("expected") or assert_eq!("hardcoded", "hardcoded"))
//!    instead of actually executing the code under test.
//! 2. **Buggy-state verification**: runs the test against BOTH the buggy state
//!    (before the fix) and the fixed state (after the fix). A genuine test
//!    must FAIL on the buggy state and PASS on the fixed state. If the test
//!    passes on both, it's not testing anything.
//! 3. **Coverage-skip detection**: rejects tests that skip the code under test
//!    (e.g., `#[ignore]`, `return;` at the top, `if false { ... }`).

use serde::{Deserialize, Serialize};

/// The result of a test-hacking check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum HackCheckResult {
    /// The test appears genuine (no hacking detected).
    Genuine,
    /// The test is suspicious — static matching detected.
    StaticMatching(String),
    /// The test is suspicious — it passes on both buggy and fixed states.
    PassesOnBoth(String),
    /// The test is suspicious — it skips the code under test.
    CoverageSkip(String),
    /// The test is suspicious — it's too trivial (empty body or just `assert!(true)`).
    TrivialTest(String),
}

impl HackCheckResult {
    /// Whether this result indicates a hack (not genuine).
    pub fn is_hack(&self) -> bool {
        !matches!(self, HackCheckResult::Genuine)
    }

    /// The reason string (for diagnostics).
    pub fn reason(&self) -> String {
        match self {
            HackCheckResult::Genuine => "genuine".into(),
            HackCheckResult::StaticMatching(s) => format!("static matching: {s}"),
            HackCheckResult::PassesOnBoth(s) => format!("passes on both buggy + fixed: {s}"),
            HackCheckResult::CoverageSkip(s) => format!("coverage skip: {s}"),
            HackCheckResult::TrivialTest(s) => format!("trivial test: {s}"),
        }
    }
}

/// Check 1: Static-matching detection — does the test body use hardcoded
/// assertions instead of actually calling the code under test?
///
/// Detects:
/// - `assert!(true)`, `assert!(false)` without conditional logic
/// - `assert_eq!("hardcoded", "hardcoded")` with identical string literals
/// - Tests that don't reference any function/symbol from the code under test
/// - `assert_eq!(1, 1)` style tautologies
pub fn detect_static_matching(test_source: &str, code_under_test: &str) -> HackCheckResult {
    let test_lower = test_source.to_lowercase();

    // Check for assert!(true) or assert!(false) — trivial assertions.
    if test_lower.contains("assert!(true)") || test_lower.contains("assert!(false)") {
        return HackCheckResult::TrivialTest(
            "test contains assert!(true) or assert!(false) — not testing anything".into(),
        );
    }

    // Check for assert_eq!(X, X) where both sides are identical literals.
    // Simple heuristic: look for assert_eq! with the same string on both sides.
    for line in test_source.lines() {
        let trimmed = line.trim();
        if trimmed.contains("assert_eq!") {
            // Extract the two arguments (naive: split by comma inside the macro).
            if let Some(args) = extract_macro_args(trimmed, "assert_eq!") {
                if args.len() >= 2 {
                    let left = args[0].trim();
                    let right = args[1].trim();
                    // If both sides are identical string literals or numbers.
                    if left == right && (left.starts_with('"') || left.parse::<f64>().is_ok()) {
                        return HackCheckResult::StaticMatching(format!(
                            "assert_eq!({left}, {right}) — both sides are identical literals"
                        ));
                    }
                }
            }
        }
    }

    // Check if the test references any symbol from the code under test.
    // Extract function/struct/enum names from the code under test.
    let code_symbols = extract_code_symbols(code_under_test);
    if !code_symbols.is_empty() {
        let test_references_any = code_symbols.iter().any(|sym| test_source.contains(sym));
        if !test_references_any {
            return HackCheckResult::StaticMatching(format!(
                "test does not reference any symbol from the code under test ({})",
                code_symbols.join(", ")
            ));
        }
    }

    HackCheckResult::Genuine
}

/// Check 2: Buggy-state verification — run the test against both the buggy
/// and fixed states. A genuine test must FAIL on buggy and PASS on fixed.
///
/// This is the dual-state check from SWE-Universe: if a test passes on both
/// the pre-fix and post-fix code, it's not actually testing the fix.
///
/// The caller provides:
/// - `test_passes_on_buggy`: whether the test passes when run against the
///   pre-fix (buggy) code.
/// - `test_passes_on_fixed`: whether the test passes when run against the
///   post-fix (fixed) code.
pub fn verify_dual_state(
    test_passes_on_buggy: bool,
    test_passes_on_fixed: bool,
) -> HackCheckResult {
    match (test_passes_on_buggy, test_passes_on_fixed) {
        (false, true) => HackCheckResult::Genuine, // fails on buggy, passes on fixed = genuine
        (true, true) => HackCheckResult::PassesOnBoth(
            "test passes on BOTH buggy and fixed states — it's not testing the fix".into(),
        ),
        (false, false) => HackCheckResult::PassesOnBoth(
            "test fails on BOTH buggy and fixed states — the fix didn't actually fix the test"
                .into(),
        ),
        (true, false) => HackCheckResult::PassesOnBoth(
            "test passes on buggy but fails on fixed — the fix broke the test".into(),
        ),
    }
}

/// Check 3: Coverage-skip detection — does the test skip the code under test?
///
/// Detects:
/// - `#[ignore]` attribute
/// - `return;` at the top of the test body (before any assertion)
/// - `if false { ... }` guards that prevent execution
/// - Empty test bodies (just `fn test_x() {}` with no assertions)
pub fn detect_coverage_skip(test_source: &str) -> HackCheckResult {
    let test_lower = test_source.to_lowercase();

    // #[ignore] attribute.
    if test_lower.contains("#[ignore]") {
        return HackCheckResult::CoverageSkip(
            "test has #[ignore] attribute — it won't actually run".into(),
        );
    }

    // `return;` at the top of the test body (before any assertion).
    // Heuristic: if `return;` appears before any `assert` in the test.
    let return_pos = test_lower.find("return;");
    let assert_pos = test_lower.find("assert");
    if let Some(rp) = return_pos {
        if assert_pos.is_none() || rp < assert_pos.unwrap() {
            return HackCheckResult::CoverageSkip(
                "test has `return;` before any assertion — it exits without testing".into(),
            );
        }
    }

    // `if false { ... }` guards.
    if test_lower.contains("if false") {
        return HackCheckResult::CoverageSkip(
            "test has `if false { ... }` guard — the test body never executes".into(),
        );
    }

    // Empty test body — no assertions at all.
    if !test_lower.contains("assert") {
        return HackCheckResult::TrivialTest(
            "test has no assertions — it's an empty body that trivially passes".into(),
        );
    }

    HackCheckResult::Genuine
}

/// The full test-hacking check — runs all three checks in sequence.
/// Returns the first hack detected, or Genuine if all pass.
pub fn check_test_hacking(test_source: &str, code_under_test: &str) -> HackCheckResult {
    // Check 3 first (coverage skip) — if the test doesn't even run, the
    // other checks are moot.
    let skip_check = detect_coverage_skip(test_source);
    if skip_check.is_hack() {
        return skip_check;
    }

    // Check 1: static matching.
    let static_check = detect_static_matching(test_source, code_under_test);
    if static_check.is_hack() {
        return static_check;
    }

    // Check 2 (dual-state) is caller-driven (requires running the test against
    // both states). The caller calls verify_dual_state separately.
    // Here we return Genuine (the source-level checks passed).
    HackCheckResult::Genuine
}

/// The full verification with dual-state check (the caller provides the
/// buggy/fixed pass results from actually running the tests).
pub fn full_verify(
    test_source: &str,
    code_under_test: &str,
    test_passes_on_buggy: bool,
    test_passes_on_fixed: bool,
) -> HackCheckResult {
    // Source-level checks first.
    let source_check = check_test_hacking(test_source, code_under_test);
    if source_check.is_hack() {
        return source_check;
    }

    // Dual-state check.
    verify_dual_state(test_passes_on_buggy, test_passes_on_fixed)
}

/// Extract function/struct/enum names from Rust source code (for the
/// static-matching check — does the test reference any symbols from the
/// code under test?).
fn extract_code_symbols(source: &str) -> Vec<String> {
    let mut symbols = vec![];
    for line in source.lines() {
        let trimmed = line.trim();
        // fn name, struct name, enum name, impl name, trait name
        for keyword in &["fn ", "struct ", "enum ", "trait "] {
            if trimmed.starts_with(keyword) {
                let rest = trimmed.strip_prefix(keyword).unwrap_or("");
                // Take the identifier (up to (, <, {, or whitespace).
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    symbols.push(name);
                }
            }
        }
    }
    symbols
}

/// Naive macro argument extractor — splits the inside of `macro!(a, b, ...)`
/// by commas (ignoring nested parens).
fn extract_macro_args(source: &str, macro_name: &str) -> Option<Vec<String>> {
    let prefix = format!("{macro_name}!(");
    let start = source.find(&prefix)? + prefix.len();
    let end = source.rfind(')')?;
    if end <= start {
        return None;
    }
    let inner = &source[start..end];
    // Split by top-level commas (naive — doesn't handle nested commas perfectly,
    // but sufficient for the tautology check).
    let mut args = vec![];
    let mut depth = 0;
    let mut current = String::new();
    for ch in inner.chars() {
        match ch {
            '(' | '[' | '{' => {
                depth += 1;
                current.push(ch);
            }
            ')' | ']' | '}' => {
                depth -= 1;
                current.push(ch);
            }
            ',' if depth == 0 => {
                args.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    if !current.trim().is_empty() {
        args.push(current.trim().to_string());
    }
    Some(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Check 1: Static matching ──────────────────────────────────────

    #[test]
    fn detects_assert_true() {
        let test = "#[test] fn test_x() { assert!(true); }";
        let code = "fn foo() -> i32 { 42 }";
        let result = detect_static_matching(test, code);
        assert!(matches!(result, HackCheckResult::TrivialTest(_)));
    }

    #[test]
    fn detects_tautological_assert_eq() {
        let test = r#"#[test] fn test_x() { assert_eq!("hello", "hello"); }"#;
        let code = "fn foo() -> String { \"hello\".into() }";
        let result = detect_static_matching(test, code);
        assert!(matches!(result, HackCheckResult::StaticMatching(_)));
    }

    #[test]
    fn detects_numeric_tautology() {
        let test = "#[test] fn test_x() { assert_eq!(42, 42); }";
        let code = "fn foo() -> i32 { 42 }";
        let result = detect_static_matching(test, code);
        assert!(matches!(result, HackCheckResult::StaticMatching(_)));
    }

    #[test]
    fn detects_no_symbol_reference() {
        let test = "#[test] fn test_x() { assert_eq!(1 + 1, 2); }";
        let code = "fn calculate() -> i32 { 42 }";
        let result = detect_static_matching(test, code);
        assert!(matches!(result, HackCheckResult::StaticMatching(_)));
    }

    #[test]
    fn allows_genuine_test() {
        let test = "#[test] fn test_foo() { assert_eq!(foo(), 42); }";
        let code = "fn foo() -> i32 { 42 }";
        let result = detect_static_matching(test, code);
        assert_eq!(result, HackCheckResult::Genuine);
    }

    // ── Check 2: Dual-state verification ───────────────────────────────

    #[test]
    fn dual_state_genuine() {
        let result = verify_dual_state(false, true);
        assert_eq!(result, HackCheckResult::Genuine);
    }

    #[test]
    fn dual_state_passes_on_both() {
        let result = verify_dual_state(true, true);
        assert!(matches!(result, HackCheckResult::PassesOnBoth(_)));
    }

    #[test]
    fn dual_state_fails_on_both() {
        let result = verify_dual_state(false, false);
        assert!(matches!(result, HackCheckResult::PassesOnBoth(_)));
    }

    #[test]
    fn dual_state_passes_buggy_fails_fixed() {
        let result = verify_dual_state(true, false);
        assert!(matches!(result, HackCheckResult::PassesOnBoth(_)));
    }

    // ── Check 3: Coverage skip ─────────────────────────────────────────

    #[test]
    fn detects_ignore_attribute() {
        let test = "#[test] #[ignore] fn test_x() { assert!(true); }";
        let result = detect_coverage_skip(test);
        assert!(matches!(result, HackCheckResult::CoverageSkip(_)));
    }

    #[test]
    fn detects_early_return() {
        let test = "#[test] fn test_x() { return; assert_eq!(1, 1); }";
        let result = detect_coverage_skip(test);
        assert!(matches!(result, HackCheckResult::CoverageSkip(_)));
    }

    #[test]
    fn detects_if_false_guard() {
        let test = "#[test] fn test_x() { if false { assert_eq!(1, 1); } }";
        let result = detect_coverage_skip(test);
        assert!(matches!(result, HackCheckResult::CoverageSkip(_)));
    }

    #[test]
    fn detects_no_assertions() {
        let test = "#[test] fn test_x() { let x = 1; }";
        let result = detect_coverage_skip(test);
        assert!(matches!(result, HackCheckResult::TrivialTest(_)));
    }

    #[test]
    fn allows_genuine_test_coverage() {
        let test = "#[test] fn test_x() { assert_eq!(foo(), 42); }";
        let result = detect_coverage_skip(test);
        assert_eq!(result, HackCheckResult::Genuine);
    }

    // ── Full check ─────────────────────────────────────────────────────

    #[test]
    fn full_verify_genuine() {
        let test = "#[test] fn test_foo() { assert_eq!(foo(), 42); }";
        let code = "fn foo() -> i32 { 42 }";
        let result = full_verify(test, code, false, true);
        assert_eq!(result, HackCheckResult::Genuine);
    }

    #[test]
    fn full_verify_catches_trivial() {
        let test = "#[test] fn test_x() { assert!(true); }";
        let code = "fn foo() -> i32 { 42 }";
        let result = full_verify(test, code, false, true);
        assert!(result.is_hack());
    }

    #[test]
    fn full_verify_catches_static_matching() {
        let test = r#"#[test] fn test_x() { assert_eq!("same", "same"); }"#;
        let code = "fn foo() -> String { \"hello\".into() }";
        let result = full_verify(test, code, false, true);
        assert!(result.is_hack());
    }

    #[test]
    fn full_verify_catches_coverage_skip() {
        let test = "#[test] #[ignore] fn test_x() { assert_eq!(1, 1); }";
        let code = "fn foo() -> i32 { 42 }";
        let result = full_verify(test, code, false, true);
        assert!(result.is_hack());
    }

    #[test]
    fn full_verify_catches_passes_on_both() {
        let test = "#[test] fn test_foo() { assert_eq!(foo(), 42); }";
        let code = "fn foo() -> i32 { 42 }";
        // Test passes on BOTH buggy and fixed → hack.
        let result = full_verify(test, code, true, true);
        assert!(matches!(result, HackCheckResult::PassesOnBoth(_)));
    }

    #[test]
    fn full_verify_passes_when_source_ok_but_dual_state_not_checked() {
        let test = "#[test] fn test_foo() { assert_eq!(foo(), 42); }";
        let code = "fn foo() -> i32 { 42 }";
        // check_test_hacking (source only) should return Genuine.
        let result = check_test_hacking(test, code);
        assert_eq!(result, HackCheckResult::Genuine);
    }

    // ── HackCheckResult helpers ────────────────────────────────────────

    #[test]
    fn is_hack_works() {
        assert!(!HackCheckResult::Genuine.is_hack());
        assert!(HackCheckResult::StaticMatching("x".into()).is_hack());
        assert!(HackCheckResult::PassesOnBoth("x".into()).is_hack());
        assert!(HackCheckResult::CoverageSkip("x".into()).is_hack());
        assert!(HackCheckResult::TrivialTest("x".into()).is_hack());
    }

    #[test]
    fn reason_works() {
        assert_eq!(HackCheckResult::Genuine.reason(), "genuine");
        assert!(HackCheckResult::StaticMatching("detail".into())
            .reason()
            .contains("detail"));
    }

    // ── Integration with ITVF: the verify_fn can use the detector ──────

    #[test]
    fn itvf_with_hack_detector_catches_trivial_test() {
        // Simulate an ITVF run where the verify_fn uses the hack detector.
        let test_code = "#[test] fn test_x() { assert!(true); }";
        let code = "fn foo() -> i32 { 42 }";
        let result = crate::run_itvf(&crate::ItvfConfig::default(), |_| {
            let hack_check = check_test_hacking(test_code, code);
            if hack_check.is_hack() {
                Err(hack_check.reason())
            } else {
                Ok(())
            }
        });
        assert!(!result.matched);
        assert!(result
            .last_failure
            .as_deref()
            .unwrap_or("")
            .contains("trivial"));
    }

    #[test]
    fn itvf_with_hack_detector_passes_genuine_test() {
        let test_code = "#[test] fn test_foo() { assert_eq!(foo(), 42); }";
        let code = "fn foo() -> i32 { 42 }";
        let result = crate::run_itvf(&crate::ItvfConfig::default(), |_| {
            let hack_check = check_test_hacking(test_code, code);
            if hack_check.is_hack() {
                Err(hack_check.reason())
            } else {
                Ok(())
            }
        });
        assert!(result.matched);
    }
}
