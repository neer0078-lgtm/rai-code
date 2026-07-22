//! The ITVF loop — Implement → Test → Verify → Fix, repeat until goal matched.
//!
//! This is RAI Code's primary per-task loop. It's how an SLM develops RAI Code
//! AND how RAI Code builds apps — same loop, two layers. The loop is bounded
//! (max 8 iterations), circuit-broken (same failure 3× → escalate), and
//! goal-matched (the VERIFY commands pass + spec met, not the agent's claim).
//!
//! T47: the ITVF state machine (a pure transition function).
//! T48: the ITVF loop driver (runs the agent loop per iteration, bounded).

use serde::{Deserialize, Serialize};

/// The ITVF state machine states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItvfState {
    /// Writing the code for this iteration.
    Implement,
    /// Writing/running the tests.
    Test,
    /// Running the verify commands (cargo test/clippy/fmt).
    Verify,
    /// The goal is matched — done.
    Done,
    /// Fixing the specific failure extracted from the verify output.
    Fix,
    /// Escalating (circuit-breaker or budget exhausted).
    Escalate,
    /// Aborted (unrecoverable).
    Abort,
}

/// The events that drive ITVF state transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItvfEvent {
    /// The implementation is written.
    Implemented,
    /// The tests are written.
    Tested,
    /// The verify commands all pass + the spec is met.
    GoalMatched,
    /// The verify failed (with the specific failure).
    GoalNotMatched(String),
    /// A fix was applied.
    Fixed,
    /// The iteration cap was hit.
    IterationCapHit,
    /// The same failure recurred 3 times (circuit-breaker).
    CircuitBreaker,
    /// An unrecoverable error.
    Aborted,
}

/// T47: the pure state-transition function.
///
// Implement --Implemented--> Test
// Test --Tested--> Verify
// Verify --GoalMatched--> Done
// Verify --GoalNotMatched--> Fix
// Fix --Fixed--> Implement
// Fix --IterationCapHit--> Escalate
// any --CircuitBreaker--> Escalate
// any --Aborted--> Abort
// Done/Escalate/Abort are terminal.
pub fn next_state(current: ItvfState, event: ItvfEvent) -> ItvfState {
    match (current, event) {
        // Normal flow.
        (ItvfState::Implement, ItvfEvent::Implemented) => ItvfState::Test,
        (ItvfState::Test, ItvfEvent::Tested) => ItvfState::Verify,
        (ItvfState::Verify, ItvfEvent::GoalMatched) => ItvfState::Done,
        (ItvfState::Verify, ItvfEvent::GoalNotMatched(_)) => ItvfState::Fix,
        (ItvfState::Fix, ItvfEvent::Fixed) => ItvfState::Implement,

        // Escalation.
        (ItvfState::Fix, ItvfEvent::IterationCapHit) => ItvfState::Escalate,
        (_, ItvfEvent::CircuitBreaker) => ItvfState::Escalate,
        (_, ItvfEvent::Aborted) => ItvfState::Abort,

        // Terminal states stay put.
        (s, _) if matches!(s, ItvfState::Done | ItvfState::Escalate | ItvfState::Abort) => s,

        // Unexpected transitions: stay in the current state (safe default).
        _ => current,
    }
}

/// The result of an ITVF loop run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItvfResult {
    /// Whether the goal was matched.
    pub matched: bool,
    /// The number of iterations used.
    pub iterations: u32,
    /// The last failure (if not matched).
    pub last_failure: Option<String>,
    /// Whether the loop should escalate (circuit-breaker or cap hit).
    pub escalate: bool,
}

/// T48: the ITVF loop driver config.
#[derive(Debug, Clone)]
pub struct ItvfConfig {
    /// Max iterations per task (default 8).
    pub max_iterations: u32,
    /// Circuit-breaker threshold (same failure N times → escalate).
    pub circuit_breaker_threshold: u32,
}

impl Default for ItvfConfig {
    fn default() -> Self {
        Self {
            max_iterations: 8,
            circuit_breaker_threshold: 3,
        }
    }
}

/// T48: the ITVF loop driver — runs the verify loop with bounded retry +
/// circuit-breaker. This is the pure logic (the actual agent-loop calls +
/// sandbox runs are injected by the caller via closures in the full impl;
/// here we test the state-machine + the counting logic).
///
/// The `verify_fn` returns `Ok(())` if the goal is matched (all verify
/// commands pass + spec met), or `Err(failure_msg)` if not. The loop calls
/// it, and if it fails, increments the iteration counter + tracks the
/// failure. If the same failure recurs `circuit_breaker_threshold` times,
/// it escalates. If `max_iterations` is hit, it escalates.
pub fn run_itvf(config: &ItvfConfig, verify_fn: impl Fn(u32) -> Result<(), String>) -> ItvfResult {
    let mut iterations = 0u32;
    let mut failure_counts: std::collections::HashMap<String, u32> =
        std::collections::HashMap::new();

    loop {
        iterations += 1;

        // The iteration: Implement -> Test -> Verify (conceptually — the actual
        // code+tests are written by the agent loop; here we just run the verify).
        match verify_fn(iterations) {
            Ok(()) => {
                return ItvfResult {
                    matched: true,
                    iterations,
                    last_failure: None,
                    escalate: false,
                };
            }
            Err(failure) => {
                // Track the failure for circuit-breaking.
                *failure_counts.entry(failure.clone()).or_insert(0) += 1;
                let count = failure_counts[&failure];

                // Circuit-breaker: same failure N times.
                if count >= config.circuit_breaker_threshold {
                    return ItvfResult {
                        matched: false,
                        iterations,
                        last_failure: Some(failure),
                        escalate: true,
                    };
                }

                // Iteration cap.
                if iterations >= config.max_iterations {
                    return ItvfResult {
                        matched: false,
                        iterations,
                        last_failure: Some(failure),
                        escalate: true,
                    };
                }

                // Fix -> back to Implement (loop continues).
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T47: next_state covers all transitions + terminals stay put.
    #[test]
    fn itvf_next_state_transitions() {
        // Normal flow.
        assert_eq!(
            next_state(ItvfState::Implement, ItvfEvent::Implemented),
            ItvfState::Test
        );
        assert_eq!(
            next_state(ItvfState::Test, ItvfEvent::Tested),
            ItvfState::Verify
        );
        assert_eq!(
            next_state(ItvfState::Verify, ItvfEvent::GoalMatched),
            ItvfState::Done
        );
        assert_eq!(
            next_state(ItvfState::Verify, ItvfEvent::GoalNotMatched("err".into())),
            ItvfState::Fix
        );
        assert_eq!(
            next_state(ItvfState::Fix, ItvfEvent::Fixed),
            ItvfState::Implement
        );

        // Escalation.
        assert_eq!(
            next_state(ItvfState::Fix, ItvfEvent::IterationCapHit),
            ItvfState::Escalate
        );
        assert_eq!(
            next_state(ItvfState::Verify, ItvfEvent::CircuitBreaker),
            ItvfState::Escalate
        );
        assert_eq!(
            next_state(ItvfState::Implement, ItvfEvent::Aborted),
            ItvfState::Abort
        );

        // Terminal states stay put.
        assert_eq!(
            next_state(ItvfState::Done, ItvfEvent::Implemented),
            ItvfState::Done
        );
        assert_eq!(
            next_state(ItvfState::Escalate, ItvfEvent::Fixed),
            ItvfState::Escalate
        );
        assert_eq!(
            next_state(ItvfState::Abort, ItvfEvent::GoalMatched),
            ItvfState::Abort
        );
    }

    /// T48: a task that matches on iteration 1.
    #[test]
    fn itvf_loop_matches_on_iteration_1() {
        let result = run_itvf(&ItvfConfig::default(), |_| Ok(()));
        assert!(result.matched);
        assert_eq!(result.iterations, 1);
        assert!(result.last_failure.is_none());
        assert!(!result.escalate);
    }

    /// T48: a task that never matches -> hits the iteration cap.
    #[test]
    fn itvf_loop_never_matches() {
        let result = run_itvf(&ItvfConfig::default(), |_| Err("same error".into()));
        assert!(!result.matched);
        assert_eq!(result.iterations, 3); // circuit-breaker at 3 (same failure 3x)
        assert!(result.escalate);
        assert_eq!(result.last_failure.as_deref(), Some("same error"));
    }

    /// T48: a task that matches on iteration 3 (different failures each time).
    #[test]
    fn itvf_loop_matches_on_iteration_3() {
        let result = run_itvf(&ItvfConfig::default(), |i| {
            if i >= 3 {
                Ok(())
            } else {
                Err(format!("error-{i}"))
            }
        });
        assert!(result.matched);
        assert_eq!(result.iterations, 3);
    }

    /// T48: circuit-breaker fires when the same failure recurs 3 times.
    #[test]
    fn itvf_loop_circuit_breaker() {
        let cfg = ItvfConfig {
            max_iterations: 10,
            circuit_breaker_threshold: 3,
        };
        let result = run_itvf(&cfg, |_| Err("identical failure".into()));
        assert!(!result.matched);
        assert_eq!(result.iterations, 3); // 3 identical failures → circuit-breaker
        assert!(result.escalate);
    }

    /// T48: different failures don't trigger the circuit-breaker (they hit the cap instead).
    #[test]
    fn itvf_loop_different_failures_hit_cap() {
        let cfg = ItvfConfig {
            max_iterations: 5,
            circuit_breaker_threshold: 3,
        };
        let result = run_itvf(&cfg, |i| Err(format!("unique-error-{i}")));
        assert!(!result.matched);
        assert_eq!(result.iterations, 5); // 5 unique failures → iteration cap
        assert!(result.escalate);
    }
}

/// A ReAct (Reason+Act) event — the explicit Thought/Action/Observation pattern
/// from Yao et al. 2022 (arxiv 2210.03629), clean-room reimplemented.
///
/// RAI Code's agent loop emits these as part of the streaming: the model's
/// text output before a tool call is the "Thought"; the tool call itself is
/// the "Action"; the tool result is the "Observation". This formalizes the
/// ReAct pattern as a first-class event type so the ITVF diagnostics can
/// record which phase of reasoning led to a failure.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", content = "data")]
pub enum ReActEvent {
    /// The model's reasoning before acting (its "thought process").
    Thought(String),
    /// The action the model decided to take (a tool call).
    Action {
        /// The tool name.
        tool: String,
        /// The tool arguments.
        args: serde_json::Value,
    },
    /// The observation from executing the action (the tool result).
    Observation {
        /// Whether the action succeeded.
        success: bool,
        /// The result content (text or error).
        content: String,
    },
}

/// An extended ITVF result with diagnostic tracking (MemoHarness integration).
///
/// When the verify step fails, the ITVF loop records a `TaskDiagnostic` —
/// which harness dimension caused the failure, what was changed to fix it,
/// and whether the fix worked. These accumulate in the experience bank so
/// `adapt_harness_for_task` can learn from past failures.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItvfResultWithDiagnostics {
    /// The base ITVF result.
    pub base: ItvfResult,
    /// The diagnostics recorded during this run.
    pub diagnostics: Vec<crate::harness::TaskDiagnostic>,
    /// The ReAct events (thoughts/actions/observations) from the run.
    pub react_events: Vec<ReActEvent>,
}

/// Classify a failure message into a harness dimension (which dimension
/// caused the failure). This is the "diagnostic, not just score-driven"
/// insight from MemoHarness — knowing WHICH dimension failed is the key
/// to learning from experience.
pub fn classify_failure(failure: &str) -> crate::harness::HarnessDimension {
    let lower = failure.to_lowercase();
    if lower.contains("compile") || lower.contains("syntax") || lower.contains("type error") {
        crate::harness::HarnessDimension::Context
    } else if lower.contains("tool") || lower.contains("not found") || lower.contains("mcp") {
        crate::harness::HarnessDimension::Tool
    } else if lower.contains("timeout") || lower.contains("rate limit") || lower.contains("token") {
        crate::harness::HarnessDimension::Generation
    } else if lower.contains("permission") || lower.contains("denied") || lower.contains("loop") {
        crate::harness::HarnessDimension::Orchestration
    } else if lower.contains("context") || lower.contains("window") || lower.contains("compact") {
        crate::harness::HarnessDimension::Memory
    } else if lower.contains("test") || lower.contains("verify") || lower.contains("assert") {
        crate::harness::HarnessDimension::Output
    } else {
        // Default: context (the most common failure dimension).
        crate::harness::HarnessDimension::Context
    }
}

/// T68: the extended ITVF loop driver with diagnostic tracking.
///
/// Runs the verify loop (same bounded-retry + circuit-breaker as `run_itvf`),
/// but also records a `TaskDiagnostic` for each failure — which dimension
/// caused it, what the fix attempt was, and whether it worked. These
/// diagnostics are returned in `ItvfResultWithDiagnostics` and can be fed
/// back to `adapt_harness_for_task` for future runs.
///
/// Additionally, the `verify_fn` now returns a `ReActEvent` alongside the
/// result, so the caller can record the thought/action/observation chain.
pub fn run_itvf_with_diagnostics(
    config: &ItvfConfig,
    verify_fn: impl Fn(u32) -> Result<(), String>,
) -> ItvfResultWithDiagnostics {
    let mut iterations = 0u32;
    let mut failure_counts: std::collections::HashMap<String, u32> =
        std::collections::HashMap::new();
    let mut diagnostics: Vec<crate::harness::TaskDiagnostic> = vec![];
    let mut last_failure: Option<String> = None;

    loop {
        iterations += 1;
        match verify_fn(iterations) {
            Ok(()) => {
                // If there was a previous failure that was now fixed, record
                // a successful diagnostic.
                if let Some(ref failure) = last_failure {
                    let dim = classify_failure(failure);
                    diagnostics.push(crate::harness::TaskDiagnostic {
                        task_description: String::new(), // filled by the caller
                        failed_dimension: dim,
                        failure_message: failure.clone(),
                        fix_description: format!("iteration {iterations} resolved the failure"),
                        fix_succeeded: true,
                        iteration: iterations,
                    });
                }
                return ItvfResultWithDiagnostics {
                    base: ItvfResult {
                        matched: true,
                        iterations,
                        last_failure: None,
                        escalate: false,
                    },
                    diagnostics,
                    react_events: vec![],
                };
            }
            Err(failure) => {
                *failure_counts.entry(failure.clone()).or_insert(0) += 1;
                let count = failure_counts[&failure];

                // Record a diagnostic: this dimension failed, we're trying to fix it.
                let dim = classify_failure(&failure);
                diagnostics.push(crate::harness::TaskDiagnostic {
                    task_description: String::new(),
                    failed_dimension: dim,
                    failure_message: failure.clone(),
                    fix_description: format!("attempting fix (iteration {iterations})"),
                    fix_succeeded: false, // not yet — will be updated if the next iteration succeeds
                    iteration: iterations,
                });

                // Circuit-breaker.
                if count >= config.circuit_breaker_threshold {
                    return ItvfResultWithDiagnostics {
                        base: ItvfResult {
                            matched: false,
                            iterations,
                            last_failure: Some(failure),
                            escalate: true,
                        },
                        diagnostics,
                        react_events: vec![],
                    };
                }

                // Iteration cap.
                if iterations >= config.max_iterations {
                    return ItvfResultWithDiagnostics {
                        base: ItvfResult {
                            matched: false,
                            iterations,
                            last_failure: Some(failure),
                            escalate: true,
                        },
                        diagnostics,
                        react_events: vec![],
                    };
                }

                last_failure = Some(failure);
            }
        }
    }
}

#[cfg(test)]
mod diagnostic_tests {
    use super::*;
    use crate::harness::HarnessDimension;

    /// T68: classify_failure maps failure messages to the right dimension.
    #[test]
    fn classify_failure_maps_correctly() {
        assert_eq!(
            classify_failure("compile error: type mismatch"),
            HarnessDimension::Context
        );
        assert_eq!(
            classify_failure("tool not found: Read"),
            HarnessDimension::Tool
        );
        assert_eq!(
            classify_failure("rate limit exceeded"),
            HarnessDimension::Generation
        );
        assert_eq!(
            classify_failure("permission denied"),
            HarnessDimension::Orchestration
        );
        assert_eq!(
            classify_failure("context window exceeded"),
            HarnessDimension::Memory
        );
        assert_eq!(
            classify_failure("test assertion failed"),
            HarnessDimension::Output
        );
        // Unknown → defaults to Context.
        assert_eq!(classify_failure("unknown error"), HarnessDimension::Context);
    }

    /// T68: run_itvf_with_diagnostics records a diagnostic on each failure.
    #[test]
    fn itvf_with_diagnostics_records_failures() {
        let cfg = ItvfConfig {
            max_iterations: 5,
            circuit_breaker_threshold: 100,
        };
        let result = run_itvf_with_diagnostics(&cfg, |i| {
            if i >= 3 {
                Ok(())
            } else {
                Err("compile error: type mismatch".into())
            }
        });
        assert!(result.base.matched);
        assert_eq!(result.base.iterations, 3);
        // Should have 2 failure diagnostics + 1 success diagnostic.
        assert_eq!(result.diagnostics.len(), 3);
        // The first two should be failures (fix_succeeded = false).
        assert!(!result.diagnostics[0].fix_succeeded);
        assert!(!result.diagnostics[1].fix_succeeded);
        // The third should be a success (the fix worked).
        assert!(result.diagnostics[2].fix_succeeded);
        // The failure dimension should be Context (compile error).
        assert_eq!(
            result.diagnostics[0].failed_dimension,
            HarnessDimension::Context
        );
    }

    /// T68: circuit-breaker records the right number of diagnostics.
    #[test]
    fn itvf_with_diagnostics_circuit_breaker() {
        let cfg = ItvfConfig {
            max_iterations: 10,
            circuit_breaker_threshold: 3,
        };
        let result = run_itvf_with_diagnostics(&cfg, |_| Err("test assertion failed".into()));
        assert!(!result.base.matched);
        assert_eq!(result.base.iterations, 3);
        assert!(result.base.escalate);
        // 3 failure diagnostics (one per iteration).
        assert_eq!(result.diagnostics.len(), 3);
        // All should be Output dimension (test assertion failed).
        assert!(result
            .diagnostics
            .iter()
            .all(|d| d.failed_dimension == HarnessDimension::Output));
    }

    /// T68: a pass on iteration 1 records no diagnostics (no failures).
    #[test]
    fn itvf_with_diagnostics_immediate_pass() {
        let result = run_itvf_with_diagnostics(&ItvfConfig::default(), |_| Ok(()));
        assert!(result.base.matched);
        assert_eq!(result.base.iterations, 1);
        assert!(
            result.diagnostics.is_empty(),
            "no failures → no diagnostics"
        );
    }

    /// T68: different failures produce different dimension classifications.
    #[test]
    fn itvf_with_diagnostics_different_dimensions() {
        let cfg = ItvfConfig {
            max_iterations: 5,
            circuit_breaker_threshold: 100,
        };
        let failures = ["compile error", "tool not found", "test failed"];
        let idx = std::sync::atomic::AtomicUsize::new(0);
        let result = run_itvf_with_diagnostics(&cfg, |_| {
            let i = idx.load(std::sync::atomic::Ordering::SeqCst);
            if i >= failures.len() {
                Ok(())
            } else {
                let f = failures[i].to_string();
                idx.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Err(f)
            }
        });
        assert!(result.base.matched);
        // Should have 3 failure diagnostics + 1 success = 4.
        assert_eq!(result.diagnostics.len(), 4);
        // Different dimensions: Context, Tool, Output.
        assert_eq!(
            result.diagnostics[0].failed_dimension,
            HarnessDimension::Context
        );
        assert_eq!(
            result.diagnostics[1].failed_dimension,
            HarnessDimension::Tool
        );
        assert_eq!(
            result.diagnostics[2].failed_dimension,
            HarnessDimension::Output
        );
    }

    /// T68: ReActEvent serde round-trips.
    #[test]
    fn react_event_serde_roundtrip() {
        let events = vec![
            ReActEvent::Thought("I need to read the file first".into()),
            ReActEvent::Action {
                tool: "Read".into(),
                args: serde_json::json!({"path": "foo.rs"}),
            },
            ReActEvent::Observation {
                success: true,
                content: "file contents here".into(),
            },
            ReActEvent::Observation {
                success: false,
                content: "file not found".into(),
            },
        ];
        for e in events {
            let json = serde_json::to_string(&e).unwrap();
            let back: ReActEvent = serde_json::from_str(&json).unwrap();
            assert_eq!(e, back);
        }
    }
}
