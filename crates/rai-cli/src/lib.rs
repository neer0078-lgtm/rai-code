//! rai-cli — the RAI Code CLI library (testable logic, separate from main.rs).
//!
//! T58: --version branding. T59: Ollama detection + tier recommendation.
//! T60: onboarding-first flow. T61: headless -p mode.

use rai_python::MemoryStore;
#[cfg(test)]
use rai_python::MockMemoryStore;
use std::sync::Arc;

/// T58: the version string with RAI Labs branding.
pub const VERSION_STRING: &str = "rai 0.1.0 · RAI Labs P. Ltd. · www.railabs.in";

/// T58: the help/about string.
pub const ABOUT_STRING: &str =
    "RAI Code — the agent that knows you and your codebase, and actually tests what it builds.";

/// T59: the hardware tier for a local model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HardwareTier {
    /// CPU-only (16-32 GB RAM) — 7B model.
    Cpu,
    /// 16 GB GPU — 14B model.
    Gpu16gb,
    /// 24 GB GPU (RTX 4090 / 5070 Ti) — 32B model (recommended).
    Gpu24gb,
    /// 48+ GB GPU — 32B Q8 or larger.
    Gpu48gb,
}

impl HardwareTier {
    /// The recommended model for this tier.
    pub fn recommended_model(&self) -> &str {
        match self {
            HardwareTier::Cpu => "qwen3-coder-7b",
            HardwareTier::Gpu16gb => "qwen3-coder-14b",
            HardwareTier::Gpu24gb | HardwareTier::Gpu48gb => "qwen3-coder-32b",
        }
    }

    /// Human-readable description.
    pub fn description(&self) -> &str {
        match self {
            HardwareTier::Cpu => "CPU-only (16-32 GB RAM) — Qwen3-Coder 7B (Q4)",
            HardwareTier::Gpu16gb => "16 GB GPU — Qwen3-Coder 14B (Q4, ~9 GB VRAM)",
            HardwareTier::Gpu24gb => {
                "24 GB GPU (RTX 4090) — Qwen3-Coder 32B (Q4, ~19 GB VRAM) [recommended]"
            }
            HardwareTier::Gpu48gb => "48+ GB GPU — Qwen3-Coder 32B (Q8) or DeepSeek-Coder-V4",
        }
    }
}

/// T59: recommend a hardware tier from a model name (pure heuristic, no network).
///
/// If the model name contains "32b" -> Gpu24gb (or Gpu48gb if "q8" is also present).
/// If "14b" -> Gpu16gb. If "7b" -> Cpu. Otherwise Cpu (safe default).
pub fn tier_recommendation(model_name: &str) -> HardwareTier {
    let lower = model_name.to_lowercase();
    if lower.contains("32b") {
        if lower.contains("q8") || lower.contains("fp16") {
            HardwareTier::Gpu48gb
        } else {
            HardwareTier::Gpu24gb
        }
    } else if lower.contains("14b") {
        HardwareTier::Gpu16gb
    } else {
        HardwareTier::Cpu
    }
}

/// T59: the Ollama detection result.
#[derive(Debug, Clone)]
pub struct OllamaDetection {
    /// Whether Ollama was detected at localhost:11434.
    pub available: bool,
    /// The detected models (names).
    pub models: Vec<String>,
    /// The recommended tier (based on the first detected model, or Cpu default).
    pub recommended_tier: HardwareTier,
}

/// T59: detect Ollama at localhost:11434 (real network — use #[ignore] in tests).
pub async fn detect_ollama() -> OllamaDetection {
    let url = "http://localhost:11434/api/tags";
    match reqwest::get(url).await {
        Ok(resp) => {
            if !resp.status().is_success() {
                return OllamaDetection {
                    available: false,
                    models: vec![],
                    recommended_tier: HardwareTier::Cpu,
                };
            }
            let body: serde_json::Value = match resp.json().await {
                Ok(v) => v,
                Err(_) => {
                    return OllamaDetection {
                        available: false,
                        models: vec![],
                        recommended_tier: HardwareTier::Cpu,
                    }
                }
            };
            let models: Vec<String> = body["models"]
                .as_array()
                .unwrap_or(&vec![])
                .iter()
                .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
                .collect();
            let tier = models
                .first()
                .map(|m| tier_recommendation(m))
                .unwrap_or(HardwareTier::Cpu);
            OllamaDetection {
                available: true,
                models,
                recommended_tier: tier,
            }
        }
        Err(_) => OllamaDetection {
            available: false,
            models: vec![],
            recommended_tier: HardwareTier::Cpu,
        },
    }
}

/// T60: the onboarding result.
#[derive(Debug, Clone)]
pub struct OnboardingResult {
    /// Whether a profile was recalled (existing user) or created (new user).
    pub existing_profile: bool,
    /// The user profile text (what the agent knows about the user).
    pub profile_text: String,
    /// The pinned directives.
    pub directives: Vec<String>,
}

/// T60: the onboarding-first flow — recall from Hindsight, or ask 3-5 questions.
///
/// Uses a `MemoryStore` (MockMemoryStore for tests; the real SidecarMemoryStore
/// in production). If `hindsight_recall("user profile")` returns results, it's
/// an existing user — return their profile. If not, create a minimal profile
/// from the provided answers.
pub async fn onboarding(
    store: &dyn MemoryStore,
    answers: &[String],
) -> anyhow::Result<OnboardingResult> {
    // Try to recall an existing profile.
    let recall = store.hindsight_recall("user profile").await?;
    let existing = recall["results"]
        .as_array()
        .map(|a| !a.is_empty())
        .unwrap_or(false);

    if existing {
        // Existing user — return their recalled profile.
        let profile_text = recall["results"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|v| v.as_str())
            .unwrap_or("existing user")
            .to_string();
        Ok(OnboardingResult {
            existing_profile: true,
            profile_text,
            directives: default_directives(),
        })
    } else {
        // New user — build a profile from the answers.
        let profile_text = if answers.is_empty() {
            "new user (no answers provided)".to_string()
        } else {
            format!(
                "new user — preferred language: {}, stack: {}, diff-before-edit: {}",
                answers.first().cloned().unwrap_or_default(),
                answers.get(1).cloned().unwrap_or_default(),
                answers.get(2).cloned().unwrap_or_default(),
            )
        };
        // Retain the profile to Hindsight.
        store.hindsight_retain(&profile_text).await?;
        Ok(OnboardingResult {
            existing_profile: false,
            profile_text,
            directives: default_directives(),
        })
    }
}

/// T60: the default pinned directives (survive compaction — governance).
pub fn default_directives() -> Vec<String> {
    vec![
        "Show diffs before applying any edit".into(),
        "Never auto-commit — user confirms each commit".into(),
        "Run tests before marking a task done".into(),
        "Escalate after 3 failed fixes (don't keep retrying)".into(),
        "When in doubt, ask — don't guess".into(),
    ]
}

/// T61: run the agent loop in headless mode (no TUI) with a MockProvider,
/// printing streamed tokens to stdout. Returns the full response text.
pub async fn run_headless(
    prompt: &str,
    provider: Arc<dyn rai_llm::Provider>,
) -> anyhow::Result<String> {
    use futures::StreamExt;
    use rai_core::{AgentEvent, AgentLoop, PermissionMode};
    let loop_ = AgentLoop::new(provider, "mock")
        .with_user_message(prompt)
        .with_permission_mode(PermissionMode::Bypass);

    let mut stream = loop_.run();
    let mut output = String::new();
    while let Some(event) = stream.next().await {
        match event {
            AgentEvent::Token(t) => {
                print!("{t}");
                output.push_str(&t);
            }
            AgentEvent::Terminal(_) => break,
            _ => {}
        }
    }
    println!();
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T58: VERSION_STRING contains RAI Labs branding.
    #[test]
    fn version_string_branding() {
        assert!(VERSION_STRING.contains("RAI Labs"));
        assert!(VERSION_STRING.contains("www.railabs.in"));
        assert!(VERSION_STRING.contains("rai 0.1"));
    }

    /// T59: tier_recommendation returns the right tier for model names.
    #[test]
    fn tier_recommendation_for_known_models() {
        assert_eq!(
            tier_recommendation("qwen3-coder-32b"),
            HardwareTier::Gpu24gb
        );
        assert_eq!(
            tier_recommendation("qwen3-coder-32b-q8"),
            HardwareTier::Gpu48gb
        );
        assert_eq!(
            tier_recommendation("qwen3-coder-14b"),
            HardwareTier::Gpu16gb
        );
        assert_eq!(tier_recommendation("qwen3-coder-7b"), HardwareTier::Cpu);
        assert_eq!(tier_recommendation("unknown-model"), HardwareTier::Cpu);
    }

    /// T59: HardwareTier::recommended_model returns the right model.
    #[test]
    fn hardware_tier_recommended_model() {
        assert_eq!(HardwareTier::Cpu.recommended_model(), "qwen3-coder-7b");
        assert_eq!(HardwareTier::Gpu16gb.recommended_model(), "qwen3-coder-14b");
        assert_eq!(HardwareTier::Gpu24gb.recommended_model(), "qwen3-coder-32b");
    }

    /// T60: onboarding with a new user (empty MockMemoryStore) creates a profile.
    #[tokio::test]
    async fn onboarding_new_profile() {
        let store = MockMemoryStore::new();
        let answers = vec!["Rust".into(), "FastAPI + PostgreSQL".into(), "yes".into()];
        let result = onboarding(&store, &answers).await.unwrap();
        assert!(!result.existing_profile);
        assert!(result.profile_text.contains("new user"));
        assert!(result.profile_text.contains("Rust"));
        assert!(!result.directives.is_empty());
    }

    /// T60: onboarding with an existing user (pre-populated MockMemoryStore).
    #[tokio::test]
    async fn onboarding_existing_profile() {
        // Pre-populate the store with a retained profile.
        let store = MockMemoryStore::new();
        store
            .hindsight_retain("user profile: senior Python dev")
            .await
            .unwrap();
        // The mock's recall checks for substring match on the query "user profile".
        // Since the retained content contains "user profile", it should be found.
        let result = onboarding(&store, &[]).await.unwrap();
        assert!(result.existing_profile);
        assert!(result.profile_text.contains("senior Python dev"));
    }

    /// T60: default_directives returns the 5 governance rules.
    #[test]
    fn default_directives_present() {
        let d = default_directives();
        assert_eq!(d.len(), 5);
        assert!(d.iter().any(|s| s.contains("diffs before")));
        assert!(d.iter().any(|s| s.contains("auto-commit")));
        assert!(d.iter().any(|s| s.contains("tests before")));
        assert!(d.iter().any(|s| s.contains("3 failed")));
        assert!(d.iter().any(|s| s.contains("don't guess")));
    }

    /// T61: run_headless streams tokens + returns the full text.
    #[tokio::test]
    async fn run_headless_streams_tokens() {
        let provider: Arc<dyn rai_llm::Provider> = Arc::new(rai_llm::mock::MockProvider::abc());
        let output = run_headless("hello", provider).await.unwrap();
        // MockProvider::abc() streams "a" + "b" + "c" then EndTurn.
        assert!(output.contains("a"));
        assert!(output.contains("b"));
        assert!(output.contains("c"));
    }
}
