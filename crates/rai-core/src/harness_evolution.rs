//! Automated Harness Evolution — trace-driven harness optimization.
//!
//! From the research alignment (P1): Meta-Harness (+7.7 pts, 4x fewer tokens),
//! HarnessX (+14.5% avg), HarnessFix (+15.2-50%). The field has moved from
//! manual to automated harness search. This module implements the first step:
//! trace-driven evolution using the TaskDiagnostics from ITVF.
//!
//! The evolution loop:
//! 1. Run a task with the current harness config.
//! 2. If it fails, the ITVF diagnostics record which dimension failed + what fix worked.
//! 3. The evolution engine analyzes the diagnostics + adjusts the harness config
//!    for the next run (similar to MemoHarness's case-adaptation, but automated
//!    + cross-task, not just per-task retrieval).
//! 4. Repeat until the task passes or the evolution budget is exhausted.

use crate::escalation::EscalationMode;
use crate::harness::{GlobalPattern, HarnessConfig, HarnessDimension, TaskDiagnostic};
use crate::perm::PermissionMode;
use std::collections::HashMap;

/// The evolution engine — analyzes diagnostics + evolves the harness config.
///
/// This is the "automated" part: instead of manually tuning the six dimensions,
/// the engine looks at which dimensions failed + what fixes worked, and
// adjusts the config accordingly. Over multiple runs, it distills GlobalPatterns
// from the diagnostics (cross-task learning).
#[derive(Debug, Default)]
pub struct HarnessEvolution {
    /// The accumulated diagnostics from all runs.
    diagnostics: Vec<TaskDiagnostic>,
    /// The distilled global patterns (cross-task).
    patterns: Vec<GlobalPattern>,
    /// The evolution budget (max config adjustments).
    max_adjustments: u32,
    /// The number of adjustments made so far.
    adjustments_made: u32,
}

impl HarnessEvolution {
    /// Construct a new evolution engine with a budget.
    pub fn new(max_adjustments: u32) -> Self {
        Self {
            max_adjustments,
            ..Default::default()
        }
    }

    /// Record diagnostics from a run.
    pub fn record_diagnostics(&mut self, diagnostics: &[TaskDiagnostic]) {
        self.diagnostics.extend(diagnostics.iter().cloned());
    }

    /// Distill global patterns from the accumulated diagnostics.
    ///
    /// Groups failures by dimension + task type, and creates patterns for
    /// dimensions that failed frequently with a known fix.
    pub fn distill_patterns(&mut self) -> &[GlobalPattern] {
        // Group diagnostics by (dimension, whether a fix was found).
        let mut by_dimension: HashMap<HarnessDimension, Vec<&TaskDiagnostic>> = HashMap::new();
        for d in &self.diagnostics {
            by_dimension.entry(d.failed_dimension).or_default().push(d);
        }

        for (dim, diags) in &by_dimension {
            // Only create a pattern if there are enough failures (≥2) and at
            // least one had a successful fix.
            let successful_fixes = diags.iter().filter(|d| d.fix_succeeded).count();
            if diags.len() >= 2 && successful_fixes >= 1 {
                // Find the most common fix description among successful fixes.
                let mut fix_counts: HashMap<&str, usize> = HashMap::new();
                for d in diags {
                    if d.fix_succeeded {
                        *fix_counts.entry(&d.fix_description).or_insert(0) += 1;
                    }
                }
                let best_fix = fix_counts
                    .iter()
                    .max_by_key(|(_, &c)| c)
                    .map(|(f, _)| f.to_string());

                if let Some(fix) = best_fix {
                    // Check if we already have a pattern for this dimension + fix.
                    let exists = self
                        .patterns
                        .iter()
                        .any(|p| p.dimension == *dim && p.recommended_change.contains(&fix));
                    if !exists {
                        self.patterns.push(GlobalPattern {
                            description: format!(
                                "auto-distilled: {:?} dimension failures resolved by '{fix}'",
                                dim
                            ),
                            dimension: *dim,
                            task_types: vec![], // could be filled from task descriptions
                            recommended_change: fix,
                            confidence: successful_fixes as f64 / diags.len() as f64,
                        });
                    }
                }
            }
        }

        &self.patterns
    }

    /// Evolve the harness config for the next run.
    ///
    /// Uses the distilled patterns + the accumulated diagnostics to adapt
    /// the global config. This is the automated equivalent of
    /// `adapt_harness_for_task`, but cross-task (it learns from ALL past runs,
    /// not just similar ones).
    pub fn evolve(&mut self, global: &HarnessConfig) -> HarnessConfig {
        if self.adjustments_made >= self.max_adjustments {
            return global.clone();
        }

        // Distill patterns if not already done.
        if self.patterns.is_empty() && !self.diagnostics.is_empty() {
            self.distill_patterns();
        }

        // Apply the patterns to the global config.
        let mut evolved = global.clone();
        for pattern in &self.patterns {
            match pattern.dimension {
                HarnessDimension::Context => {
                    if pattern.recommended_change.contains("folding") {
                        evolved.memory.context_folding_enabled = true;
                    }
                    if pattern.recommended_change.contains("more context") {
                        evolved.context.max_context_tokens =
                            (evolved.context.max_context_tokens * 3 / 2).min(131_072);
                    }
                }
                HarnessDimension::Orchestration => {
                    if pattern.recommended_change.contains("plan") {
                        evolved.orchestration.escalation_mode = EscalationMode::PlanAndExecute;
                    }
                    if pattern.recommended_change.contains("bypass") {
                        evolved.orchestration.permission_mode = PermissionMode::Bypass;
                    }
                }
                HarnessDimension::Memory => {
                    if pattern.recommended_change.contains("compaction") {
                        evolved.memory.compaction_enabled = true;
                    }
                }
                HarnessDimension::Output if pattern.recommended_change.contains("iteration") => {
                    evolved.output.itvf_max_iterations =
                        (evolved.output.itvf_max_iterations + 2).min(20);
                }
                _ => {}
            }
        }

        self.adjustments_made += 1;
        evolved
    }

    /// The number of accumulated diagnostics.
    pub fn diagnostic_count(&self) -> usize {
        self.diagnostics.len()
    }

    /// The number of distilled patterns.
    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }

    /// Whether the evolution budget is exhausted.
    pub fn is_budget_exhausted(&self) -> bool {
        self.adjustments_made >= self.max_adjustments
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evolution_distills_patterns_from_diagnostics() {
        let mut evo = HarnessEvolution::new(10);
        evo.record_diagnostics(&[
            TaskDiagnostic {
                task_description: "refactor auth".into(),
                failed_dimension: HarnessDimension::Orchestration,
                failure_message: "single-loop failed".into(),
                fix_description: "switched to plan-execute".into(),
                fix_succeeded: true,
                iteration: 3,
            },
            TaskDiagnostic {
                task_description: "refactor routes".into(),
                failed_dimension: HarnessDimension::Orchestration,
                failure_message: "single-loop failed".into(),
                fix_description: "switched to plan-execute".into(),
                fix_succeeded: true,
                iteration: 4,
            },
        ]);

        let patterns = evo.distill_patterns();
        assert!(!patterns.is_empty());
        assert!(patterns
            .iter()
            .any(|p| p.dimension == HarnessDimension::Orchestration));
    }

    #[test]
    fn evolution_adjusts_harness_config() {
        let mut evo = HarnessEvolution::new(10);
        evo.record_diagnostics(&[
            TaskDiagnostic {
                task_description: "refactor auth".into(),
                failed_dimension: HarnessDimension::Orchestration,
                failure_message: "single-loop failed".into(),
                fix_description: "switched to plan-execute".into(),
                fix_succeeded: true,
                iteration: 3,
            },
            TaskDiagnostic {
                task_description: "refactor routes".into(),
                failed_dimension: HarnessDimension::Orchestration,
                failure_message: "single-loop failed".into(),
                fix_description: "switched to plan-execute".into(),
                fix_succeeded: true,
                iteration: 4,
            },
        ]);

        let global = HarnessConfig::default_local("qwen3-coder-32b");
        let evolved = evo.evolve(&global);
        assert_eq!(
            evolved.orchestration.escalation_mode,
            EscalationMode::PlanAndExecute
        );
    }

    #[test]
    fn evolution_budget_enforced() {
        let mut evo = HarnessEvolution::new(1);
        evo.record_diagnostics(&[
            TaskDiagnostic {
                task_description: "task".into(),
                failed_dimension: HarnessDimension::Orchestration,
                failure_message: "fail".into(),
                fix_description: "switched to plan-execute".into(),
                fix_succeeded: true,
                iteration: 1,
            },
            TaskDiagnostic {
                task_description: "task".into(),
                failed_dimension: HarnessDimension::Orchestration,
                failure_message: "fail".into(),
                fix_description: "switched to plan-execute".into(),
                fix_succeeded: true,
                iteration: 2,
            },
        ]);

        let global = HarnessConfig::default_local("qwen3-coder-32b");
        assert!(
            !evo.is_budget_exhausted(),
            "budget not exhausted before any evolve"
        );

        let _evolved1 = evo.evolve(&global);
        assert!(
            evo.is_budget_exhausted(),
            "budget exhausted after 1 evolve (max=1)"
        );

        let evolved2 = evo.evolve(&global);
        assert!(evo.is_budget_exhausted(), "budget still exhausted");
        // Second evolve with exhausted budget returns the global unchanged.
        assert_eq!(evolved2, global);
        // Second evolve with exhausted budget returns the global unchanged.
        assert_eq!(evolved2, global);
    }

    #[test]
    fn evolution_no_diagnostics_no_change() {
        let mut evo = HarnessEvolution::new(10);
        let global = HarnessConfig::default_local("qwen3-coder-32b");
        let evolved = evo.evolve(&global);
        assert_eq!(evolved, global);
    }

    #[test]
    fn evolution_does_not_duplicate_patterns() {
        let mut evo = HarnessEvolution::new(10);
        let diag = TaskDiagnostic {
            task_description: "task".into(),
            failed_dimension: HarnessDimension::Orchestration,
            failure_message: "fail".into(),
            fix_description: "switched to plan-execute".into(),
            fix_succeeded: true,
            iteration: 1,
        };
        // Record the same diagnostics twice.
        evo.record_diagnostics(&[diag.clone(), diag.clone()]);
        evo.distill_patterns();
        let count = evo.pattern_count();
        // Distill again — should not duplicate.
        evo.distill_patterns();
        assert_eq!(evo.pattern_count(), count);
    }
}
