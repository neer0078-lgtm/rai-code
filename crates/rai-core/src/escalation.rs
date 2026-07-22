//! Adaptive loop escalation — per the agentic-loops research (2025-2026).
//!
//! DEFAULT: single `while(tool_call)` loop (Claude Code pattern). The model IS the planner.
//! ADAPTIVE ESCALATION: the loop classifies the task and switches mode when warranted.
//!
//! Modes (the proven-vs-hype table from research):
//!  - Agentless       : single-issue bug — localize -> repair -> validate (no loop). 32% SWE-bench Lite.
//!  - PlanAndExecute  : multi-file refactor — plan + dependency graph + change-impact (CodePlan 5/7 vs 0/7).
//!  - TreeSearch      : high-stakes / verification-critical — MCTS + hybrid verifier (R2E-Gym 51% vs 42-43%).
//!  - DynamicWorkflow : parallelizable — model writes JS, runtime executes with agent() (ultracode).
//!  - Explore         : search-heavy — FastContext dedicated explorer sub-agent (60% token cut).
//!  - ContextFolding  : long-horizon — branch+return, fold sub-trajectories (62% SWE-bench w/ 32K matching 327K).
//!  - None            : default single-loop.
//!
//! HYPE / DO NOT USE: pure Reflexion (memory confabulation), massive multi-agent swarms,
//! over-orchestrated DAGs in the hot path.

use serde::{Deserialize, Serialize};

/// The escalation mode selected for the current task.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EscalationMode {
    /// Default single while(tool_call) loop.
    #[default]
    None,
    /// Single-issue bug: localize -> repair -> validate (no agent loop).
    Agentless,
    /// Multi-file refactor: plan + dependency graph + change-impact.
    PlanAndExecute,
    /// High-stakes: MCTS + R2E-Gym hybrid verifier.
    TreeSearch,
    /// Parallelizable: model writes JS, runtime executes with agent().
    DynamicWorkflow,
    /// Search-heavy: FastContext dedicated explorer sub-agent.
    Explore,
    /// Long-horizon: branch + return, fold sub-trajectories.
    ContextFolding,
}

/// The context-management strategy (compaction vs folding).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ContextStrategy {
    /// C4 config: keep last N tool calls + summarize older (91.6% vs 71% full, -63% tokens).
    C4 {
        /// How many recent tool calls to keep verbatim.
        keep_last: u32,
        /// How many evicted messages to fold into one summary.
        summarize_window: u32,
    },
    /// Context Folding: branch sub-trajectories, fold on completion (10x smaller active ctx).
    Folding {
        /// Maximum concurrent branches.
        max_branches: u32,
        /// Context-budget fraction that triggers folding (e.g. 0.85).
        budget_threshold: f64,
    },
    /// SWE-Pruner: line-level task-aware pruning (23-38% cut).
    Pruning {
        /// The goal hint used to score line relevance.
        goal_hint: String,
    },
}

/// Classify a task into an escalation mode. Heuristics; the model can also self-classify.
pub fn classify_task(
    estimated_files: u32,
    complexity: Complexity,
    parallelizable: bool,
    verification_critical: bool,
    requires_exploration: bool,
) -> EscalationMode {
    if estimated_files == 1 && complexity == Complexity::Low {
        return EscalationMode::Agentless;
    }
    if estimated_files > 3 {
        return EscalationMode::PlanAndExecute;
    }
    if requires_exploration {
        return EscalationMode::Explore;
    }
    if parallelizable {
        return EscalationMode::DynamicWorkflow;
    }
    if verification_critical {
        return EscalationMode::TreeSearch;
    }
    EscalationMode::None
}

/// Task complexity (used by the escalation classifier).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Complexity {
    /// Low complexity — single-file, well-understood change.
    Low,
    /// Medium complexity.
    Medium,
    /// High complexity — multi-file, cross-cutting.
    High,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T06: classify_task returns the right escalation mode per branch + the None default.
    #[test]
    fn classify_task_chooses_right_mode() {
        // single-file low -> Agentless
        assert_eq!(
            classify_task(1, Complexity::Low, false, false, false),
            EscalationMode::Agentless
        );
        // >3 files -> PlanAndExecute
        assert_eq!(
            classify_task(4, Complexity::High, false, false, false),
            EscalationMode::PlanAndExecute
        );
        // requires exploration -> Explore
        assert_eq!(
            classify_task(2, Complexity::Medium, false, false, true),
            EscalationMode::Explore
        );
        // parallelizable -> DynamicWorkflow
        assert_eq!(
            classify_task(2, Complexity::Medium, true, false, false),
            EscalationMode::DynamicWorkflow
        );
        // verification-critical -> TreeSearch
        assert_eq!(
            classify_task(2, Complexity::Medium, false, true, false),
            EscalationMode::TreeSearch
        );
        // default (2 files, medium, none of the flags) -> None
        assert_eq!(
            classify_task(2, Complexity::Medium, false, false, false),
            EscalationMode::None
        );
    }
}
