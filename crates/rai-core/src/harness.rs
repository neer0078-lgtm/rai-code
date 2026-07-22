//! The HarnessConfig — a six-dimensional editable harness configuration,
//! inspired by MemoHarness (arxiv 2607.14159).
//!
//! MemoHarness's key insight: the harness (the external control layer around
//! the LLM) is itself the optimization target, and it should learn from
//! execution experience + adapt per-case. The harness is decomposed into six
//! editable control surfaces along the temporal flow of inference:
//!
//! 1. Context — how context is constructed (prompt assembly, retrieval, DCG/Graphiti)
//! 2. Tool — which tools are exposed (ToolRegistry, ToolSearch, MCP)
//! 3. Generation — decoding parameters (model, temperature, max_tokens, cache_control)
//! 4. Orchestration — how inference is orchestrated (single-loop vs plan-execute vs tree-search)
//! 5. Memory — what memory is retained (compaction, Hindsight, context management)
//! 6. Output — how outputs are validated and returned (ITVF verify, structured results)
//!
//! RAI Code formalizes these as a `HarnessConfig` struct — the single editable
//! object that the agent loop + the ITVF driver + the case-adaptation step
//! all read from + write to. This is the "harness as a first-class object"
//! pattern from MemoHarness, clean-room reimplemented.

use crate::escalation::EscalationMode;
use crate::perm::PermissionMode;
use rai_llm::CacheTtl;
use serde::{Deserialize, Serialize};

/// The six-dimensional harness configuration (MemoHarness's decomposition).
///
/// Each dimension is independently editable — the case-adaptation step can
/// change one dimension without touching the others. The ITVF loop's
/// diagnostics record WHICH dimension caused a failure, so the experience
/// bank can learn which dimensions matter for which task types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HarnessConfig {
    /// Dimension 1: Context — how context is constructed.
    pub context: ContextDim,
    /// Dimension 2: Tool — which tools are exposed.
    pub tool: ToolDim,
    /// Dimension 3: Generation — decoding parameters.
    pub generation: GenerationDim,
    /// Dimension 4: Orchestration — how inference is orchestrated.
    pub orchestration: OrchestrationDim,
    /// Dimension 5: Memory — what memory is retained.
    pub memory: MemoryDim,
    /// Dimension 6: Output — how outputs are validated.
    pub output: OutputDim,
}

/// Dimension 1: Context — how context is assembled for the model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextDim {
    /// The context window budget (max tokens to send to the model).
    pub max_context_tokens: usize,
    /// Whether to use the DCG (deterministic code graph) for structural context.
    pub use_dcg: bool,
    /// Whether to use Graphiti for semantic/temporal context.
    pub use_graphiti: bool,
    /// Whether to use Hindsight for user-profile context.
    pub use_hindsight: bool,
    /// The file-viewer window size (lines per view — SWE-agent's 100-line default).
    pub file_viewer_lines: usize,
    /// Whether to use tool-result clearing (keep last N, clear older).
    pub tool_result_clearing: bool,
    /// How many recent tool results to keep before clearing.
    pub keep_last_tool_results: usize,
}

/// Dimension 2: Tool — which tools are exposed to the model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolDim {
    /// Whether lazy-loading (ToolSearch) is enabled.
    pub lazy_load_tools: bool,
    /// Max tools loaded per ToolSearch query.
    pub max_tools_per_search: usize,
    /// Whether MCP tools are enabled.
    pub mcp_enabled: bool,
    /// Whether the browser tools are enabled.
    pub browser_enabled: bool,
}

/// Dimension 3: Generation — decoding parameters.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GenerationDim {
    /// The model id.
    pub model: String,
    /// The temperature (None = provider default).
    pub temperature: Option<f32>,
    /// Max output tokens (None = provider default).
    pub max_tokens: Option<u32>,
    /// Whether prompt caching is enabled (Anthropic cache_control).
    pub prompt_caching: bool,
    /// The cache TTL (5min or 1hr).
    pub cache_ttl: CacheTtl,
    /// Whether token-efficient tool use is enabled.
    pub token_efficient_tools: bool,
}

/// Dimension 4: Orchestration — how inference is orchestrated.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrchestrationDim {
    /// The escalation mode (None = single-loop, Agentless/PlanExecute/TreeSearch/...).
    pub escalation_mode: EscalationMode,
    /// Max turns per iteration.
    pub max_turns: u32,
    /// The permission mode (Plan/Bypass/Approval/Auto).
    pub permission_mode: PermissionMode,
    /// Whether sub-agents are enabled.
    pub sub_agents_enabled: bool,
    /// Max sub-agent depth.
    pub max_subagent_depth: u32,
}

/// Dimension 5: Memory — what memory is retained.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemoryDim {
    /// Whether compaction is enabled.
    pub compaction_enabled: bool,
    /// The autocompact threshold (utilization fraction, e.g. 0.87).
    pub autocompact_threshold: f64,
    /// Whether context folding is enabled (branch + return).
    pub context_folding_enabled: bool,
    /// Max branches for context folding.
    pub max_folding_branches: u32,
    /// Whether to store ITVF diagnostics to the experience bank.
    pub store_diagnostics: bool,
}

/// Dimension 6: Output — how outputs are validated.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OutputDim {
    /// Whether the ITVF verify step is enabled.
    pub itvf_verify_enabled: bool,
    /// Max ITVF iterations per task.
    pub itvf_max_iterations: u32,
    /// The circuit-breaker threshold (same failure N times).
    pub circuit_breaker_threshold: u32,
    /// Whether to use hybrid verification (execution + LLM judge).
    pub hybrid_verification: bool,
    /// Whether to store verification results as Graphiti episodes.
    pub store_verification_episodes: bool,
}

impl HarnessConfig {
    /// The default harness config for local mode (small LLM, offline).
    pub fn default_local(model: impl Into<String>) -> Self {
        Self {
            context: ContextDim {
                max_context_tokens: 32_768,
                use_dcg: true,
                use_graphiti: true,
                use_hindsight: true,
                file_viewer_lines: 100,
                tool_result_clearing: true,
                keep_last_tool_results: 5,
            },
            tool: ToolDim {
                lazy_load_tools: true,
                max_tools_per_search: 5,
                mcp_enabled: true,
                browser_enabled: true,
            },
            generation: GenerationDim {
                model: model.into(),
                temperature: None,
                max_tokens: None,
                prompt_caching: true,
                cache_ttl: CacheTtl::Ephemeral5m,
                token_efficient_tools: true,
            },
            orchestration: OrchestrationDim {
                escalation_mode: EscalationMode::None,
                max_turns: 50,
                permission_mode: PermissionMode::Approval,
                sub_agents_enabled: true,
                max_subagent_depth: 3,
            },
            memory: MemoryDim {
                compaction_enabled: true,
                autocompact_threshold: 0.87,
                context_folding_enabled: true,
                max_folding_branches: 10,
                store_diagnostics: true,
            },
            output: OutputDim {
                itvf_verify_enabled: true,
                itvf_max_iterations: 8,
                circuit_breaker_threshold: 3,
                hybrid_verification: true,
                store_verification_episodes: true,
            },
        }
    }

    /// The default harness config for a CPU-only 7B model (most constrained).
    pub fn default_cpu(model: impl Into<String>) -> Self {
        let mut cfg = Self::default_local(model);
        cfg.context.max_context_tokens = 8_192; // smaller context window
        cfg.context.file_viewer_lines = 50; // smaller viewer
        cfg.orchestration.sub_agents_enabled = false; // too slow for 7B
        cfg.orchestration.max_turns = 20; // fewer turns
        cfg.memory.context_folding_enabled = true; // essential for small context
        cfg.memory.max_folding_branches = 5;
        cfg
    }
}

/// A per-task diagnostic entry (MemoHarness's per-case execution entry).
///
/// Records WHICH dimension caused a failure + what was changed to fix it.
/// This is the "diagnostic, not just score-driven" insight from MemoHarness.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDiagnostic {
    /// The task description (for retrieval — "similar past cases").
    pub task_description: String,
    /// Which dimension caused the failure.
    pub failed_dimension: HarnessDimension,
    /// The specific failure message.
    pub failure_message: String,
    /// What was changed to fix it (the dimension edit).
    pub fix_description: String,
    /// Whether the fix worked.
    pub fix_succeeded: bool,
    /// The iteration this occurred on.
    pub iteration: u32,
}

/// The six harness dimensions (for diagnostic tracking).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[derive(Hash)]
pub enum HarnessDimension {
    /// Context dimension (prompt assembly, retrieval).
    Context,
    /// Tool dimension (which tools are exposed).
    Tool,
    /// Generation dimension (decoding parameters).
    Generation,
    /// Orchestration dimension (loop topology).
    Orchestration,
    /// Memory dimension (what's retained).
    Memory,
    /// Output dimension (validation).
    Output,
}

/// A distilled global pattern (MemoHarness's global pattern layer).
///
/// Cross-case regularities extracted from failure clusters — e.g.,
/// "multi-file refactors fail when context_folding is off" or
/// "shell-heavy tasks benefit from GPT-5.5 + Bypass mode".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalPattern {
    /// The pattern description.
    pub description: String,
    /// Which dimension it applies to.
    pub dimension: HarnessDimension,
    /// The task types it applies to (e.g. "multi-file", "shell-heavy").
    pub task_types: Vec<String>,
    /// The recommended config change.
    pub recommended_change: String,
    /// Confidence (0-1, based on how many cases support it).
    pub confidence: f64,
}

/// T66: the case-adaptation step (MemoHarness's test-time case adaptation).
///
/// Given a task description + the experience bank (past diagnostics + global
/// patterns), adapt the global harness config to a case-specific config.
/// No test-time feedback, no gradient updates — just retrieval + editing.
pub fn adapt_harness_for_task(
    global: &HarnessConfig,
    task_description: &str,
    diagnostics: &[TaskDiagnostic],
    patterns: &[GlobalPattern],
) -> HarnessConfig {
    let mut adapted = global.clone();

    // Find patterns that match this task type.
    let task_lower = task_description.to_lowercase();
    for pattern in patterns {
        // Check if any of the pattern's task types match the task description.
        let matches = pattern.task_types.iter().any(|tt| task_lower.contains(tt));
        if !matches {
            continue;
        }

        // Apply the recommended change to the relevant dimension.
        match pattern.dimension {
            HarnessDimension::Context => {
                if pattern.recommended_change.contains("folding") {
                    adapted.memory.context_folding_enabled = true;
                }
                if pattern.recommended_change.contains("more context") {
                    adapted.context.max_context_tokens =
                        (adapted.context.max_context_tokens * 3 / 2).min(131_072);
                }
            }
            HarnessDimension::Orchestration => {
                if pattern.recommended_change.contains("plan") {
                    adapted.orchestration.escalation_mode = EscalationMode::PlanAndExecute;
                }
                if pattern.recommended_change.contains("bypass") {
                    adapted.orchestration.permission_mode = PermissionMode::Bypass;
                }
            }
            HarnessDimension::Generation => {
                if pattern.recommended_change.contains("gpt-5") {
                    adapted.generation.model = "gpt-5.5".into();
                }
            }
            HarnessDimension::Memory if pattern.recommended_change.contains("compaction") => {
                adapted.memory.compaction_enabled = true;
            }
            _ => {}
        }
    }

    // Check past diagnostics for this task type — if the same dimension failed
    // before, preemptively adjust it.
    let similar_failures: Vec<&TaskDiagnostic> = diagnostics
        .iter()
        .filter(|d| task_lower.contains(&d.task_description.to_lowercase()))
        .collect();
    for failure in similar_failures {
        if !failure.fix_succeeded {
            continue;
        }
        // Apply the fix that worked before.
        match failure.failed_dimension {
            HarnessDimension::Context => {
                if failure.fix_description.contains("folding") {
                    adapted.memory.context_folding_enabled = true;
                }
            }
            HarnessDimension::Orchestration if failure.fix_description.contains("plan") => {
                adapted.orchestration.escalation_mode = EscalationMode::PlanAndExecute;
            }
            _ => {}
        }
    }

    adapted
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T66: HarnessConfig::default_local has sensible defaults.
    #[test]
    fn harness_config_default_local() {
        let cfg = HarnessConfig::default_local("qwen3-coder-32b");
        assert_eq!(cfg.generation.model, "qwen3-coder-32b");
        assert!(cfg.context.use_dcg);
        assert!(cfg.context.use_graphiti);
        assert!(cfg.context.use_hindsight);
        assert_eq!(cfg.context.file_viewer_lines, 100);
        assert!(cfg.generation.prompt_caching);
        assert!(cfg.output.itvf_verify_enabled);
        assert_eq!(cfg.output.itvf_max_iterations, 8);
        assert_eq!(cfg.output.circuit_breaker_threshold, 3);
    }

    /// T66: HarnessConfig::default_cpu is more constrained.
    #[test]
    fn harness_config_default_cpu() {
        let cfg = HarnessConfig::default_cpu("qwen3-coder-7b");
        assert_eq!(cfg.generation.model, "qwen3-coder-7b");
        assert!(cfg.context.max_context_tokens <= 8192);
        assert!(cfg.context.file_viewer_lines <= 50);
        assert!(!cfg.orchestration.sub_agents_enabled); // too slow for 7B
        assert!(cfg.memory.context_folding_enabled); // essential for small ctx
    }

    /// T66: adapt_harness_for_task applies patterns that match the task.
    #[test]
    fn adapt_harness_applies_matching_patterns() {
        let global = HarnessConfig::default_local("qwen3-coder-32b");
        let patterns = vec![GlobalPattern {
            description: "multi-file refactors need plan-execute".into(),
            dimension: HarnessDimension::Orchestration,
            task_types: vec!["refactor".into(), "multi-file".into()],
            recommended_change: "use plan-execute orchestration".into(),
            confidence: 0.8,
        }];

        let adapted = adapt_harness_for_task(&global, "refactor the auth module", &[], &patterns);
        assert_eq!(
            adapted.orchestration.escalation_mode,
            EscalationMode::PlanAndExecute
        );
    }

    /// T66: adapt_harness doesn't apply patterns that don't match.
    #[test]
    fn adapt_harness_skips_non_matching_patterns() {
        let global = HarnessConfig::default_local("qwen3-coder-32b");
        let patterns = vec![GlobalPattern {
            description: "shell tasks need bypass".into(),
            dimension: HarnessDimension::Orchestration,
            task_types: vec!["shell".into()],
            recommended_change: "use bypass mode".into(),
            confidence: 0.8,
        }];

        let adapted = adapt_harness_for_task(&global, "refactor the auth module", &[], &patterns);
        // The pattern is for shell tasks, not refactor — no change.
        assert_eq!(
            adapted.orchestration.permission_mode,
            global.orchestration.permission_mode
        );
    }

    /// T66: adapt_harness applies past diagnostic fixes for similar tasks.
    #[test]
    fn adapt_harness_applies_past_fixes() {
        let global = HarnessConfig::default_local("qwen3-coder-32b");
        let diagnostics = vec![TaskDiagnostic {
            task_description: "refactor auth".into(),
            failed_dimension: HarnessDimension::Orchestration,
            failure_message: "single-loop failed on multi-file".into(),
            fix_description: "switched to plan-execute".into(),
            fix_succeeded: true,
            iteration: 3,
        }];

        let adapted = adapt_harness_for_task(&global, "refactor auth module", &diagnostics, &[]);
        assert_eq!(
            adapted.orchestration.escalation_mode,
            EscalationMode::PlanAndExecute
        );
    }

    /// T66: HarnessDimension serde round-trips.
    #[test]
    fn harness_dimension_serde() {
        let dims = vec![
            HarnessDimension::Context,
            HarnessDimension::Tool,
            HarnessDimension::Generation,
            HarnessDimension::Orchestration,
            HarnessDimension::Memory,
            HarnessDimension::Output,
        ];
        for d in dims {
            let json = serde_json::to_string(&d).unwrap();
            let back: HarnessDimension = serde_json::from_str(&json).unwrap();
            assert_eq!(d, back);
        }
    }
}
