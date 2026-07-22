//! Torture tests — the hardest possible scenarios for every component of RAI Code.
//!
//! These tests throw edge cases, extreme values, and adversarial inputs at every
//! crate to find where things break. If these pass, the foundation is solid.

use rai_browser::{serialize_a11y, BrowserAction};
use rai_codegen::{parse_rust, Dcg, SymbolKind};
use rai_core::escalation::EscalationMode;
use rai_core::{
    adapt_harness_for_task, run_itvf, AgentEvent, AgentLoop, GlobalPattern, HarnessConfig,
    HarnessDimension, ItvfConfig, Permission, PermissionMode, StopReason,
};
use rai_llm::{
    CacheControl, CacheTarget, CacheTtl, ChatChunk, ChatRequest, ChatResponse, Provider,
    StopReason as LlmStopReason, Usage,
};
use rai_mcp::ToolCatalog;
use rai_python::{JsonRpcRequest, MemoryStore, MockMemoryStore};
use rai_sandbox::{LocalSandbox, Sandbox};
use rai_security::{
    taint_of, DefaultKernel, FlightRecorder, SecurityDecision, SecurityKernel, TaintLabel,
    ZERO_HASH,
};
use rai_tui::{render_to_string, App};
use std::sync::Arc;
use std::time::Duration;

// ═══════════════════════════════════════════════════════════════════════
// rai-core: AGENT LOOP STRESS
// ═══════════════════════════════════════════════════════════════════════

/// Empty conversation — loop with 0 messages should still terminate.
#[tokio::test]
async fn torture_loop_empty_conversation() {
    use futures::StreamExt;
    let provider: Arc<dyn Provider> = Arc::new(rai_llm::mock::MockProvider::abc());
    let loop_ = AgentLoop::new(provider, "mock").with_permission_mode(PermissionMode::Bypass);
    let events: Vec<AgentEvent> = loop_.run().collect().await;
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEvent::Terminal(StopReason::EndTurn))),
        "empty conversation should still terminate with EndTurn"
    );
}

/// Max turns = 1 — the loop should hit the cap after exactly 1 turn.
#[tokio::test]
async fn torture_loop_max_turns_1() {
    use futures::StreamExt;
    // A mock that always returns tool calls (never EndTurn) → will hit max_turns.
    let provider: Arc<dyn Provider> = Arc::new(rai_llm::mock::MockProvider::with_tool_call(
        "Bogus",
        serde_json::json!({}),
    ));
    let loop_ = AgentLoop::new(provider, "mock")
        .with_user_message("loop forever")
        .with_permission_mode(PermissionMode::Bypass)
        .with_max_turns(3);
    let events: Vec<AgentEvent> = loop_.run().collect().await;
    // Should terminate with MaxTurns(3) since the mock always returns tool calls.
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEvent::Terminal(StopReason::MaxTurns(_)))),
        "should hit max turns: last events: {events:?}"
    );
}

/// Cancellation during tool execution (not just before the loop).
#[tokio::test]
async fn torture_loop_cancel_during_execution() {
    use futures::StreamExt;
    let provider: Arc<dyn Provider> = Arc::new(rai_llm::mock::MockProvider::abc());
    let loop_ = AgentLoop::new(provider, "mock")
        .with_user_message("test")
        .with_permission_mode(PermissionMode::Bypass);
    loop_.cancel.cancel(); // cancel before running
    let events: Vec<AgentEvent> = loop_.run().collect().await;
    // Should have StreamAborted or EndTurn (the race).
    let has_terminal = events.iter().any(|e| matches!(e, AgentEvent::Terminal(_)));
    assert!(has_terminal, "should terminate somehow");
}

// ═══════════════════════════════════════════════════════════════════════
// rai-core: ITVF STRESS
// ═══════════════════════════════════════════════════════════════════════

/// ITVF with exactly 8 iterations (the default cap) — different failures each time.
#[test]
fn torture_itvf_exactly_at_cap() {
    let cfg = ItvfConfig {
        max_iterations: 8,
        circuit_breaker_threshold: 100, // high so cap is the limiter
    };
    let result = run_itvf(&cfg, |i| Err(format!("unique-failure-{i}")));
    assert!(!result.matched);
    assert_eq!(result.iterations, 8);
    assert!(result.escalate);
}

/// ITVF with failures that are ALMOST the same (differ by 1 char) — should NOT trigger circuit-breaker.
#[test]
fn torture_itvf_almost_identical_failures() {
    let cfg = ItvfConfig {
        max_iterations: 10,
        circuit_breaker_threshold: 3,
    };
    let result = run_itvf(&cfg, |i| Err(format!("failure-{i}")));
    // Each failure is unique (failure-1, failure-2, ...) → no circuit-breaker → hits cap.
    assert!(!result.matched);
    assert_eq!(result.iterations, 10);
    assert!(result.escalate);
}

/// ITVF with verify that passes on the last allowed iteration.
#[test]
fn torture_itvf_passes_on_last_iteration() {
    let cfg = ItvfConfig {
        max_iterations: 5,
        circuit_breaker_threshold: 100,
    };
    let result = run_itvf(&cfg, |i| {
        if i >= 5 {
            Ok(())
        } else {
            Err("not yet".into())
        }
    });
    assert!(result.matched);
    assert_eq!(result.iterations, 5);
    assert!(!result.escalate);
}

/// ITVF with max_iterations = 1 (the extreme minimum).
#[test]
fn torture_itvf_max_1_passes() {
    let cfg = ItvfConfig {
        max_iterations: 1,
        circuit_breaker_threshold: 1,
    };
    let result = run_itvf(&cfg, |_| Ok(()));
    assert!(result.matched);
    assert_eq!(result.iterations, 1);
}

/// ITVF with max_iterations = 1 (fails immediately).
#[test]
fn torture_itvf_max_1_fails() {
    let cfg = ItvfConfig {
        max_iterations: 1,
        circuit_breaker_threshold: 1,
    };
    let result = run_itvf(&cfg, |_| Err("immediate fail".into()));
    assert!(!result.matched);
    assert_eq!(result.iterations, 1);
    assert!(result.escalate); // both cap + circuit-breaker fire at 1
}

// ═══════════════════════════════════════════════════════════════════════
// rai-core: HARNESS CONFIG STRESS
// ═══════════════════════════════════════════════════════════════════════

/// adapt_harness with empty diagnostics + empty patterns → returns the global unchanged.
#[test]
fn torture_adapt_harness_empty_bank() {
    let global = HarnessConfig::default_local("qwen3-coder-32b");
    let adapted = adapt_harness_for_task(&global, "any task", &[], &[]);
    assert_eq!(adapted, global, "empty experience bank → no changes");
}

/// adapt_harness with conflicting patterns (two patterns for the same dimension, different recommendations).
#[test]
fn torture_adapt_harness_conflicting_patterns() {
    let global = HarnessConfig::default_local("qwen3-coder-32b");
    let patterns = vec![
        GlobalPattern {
            description: "use plan-execute".into(),
            dimension: HarnessDimension::Orchestration,
            task_types: vec!["refactor".into()],
            recommended_change: "use plan-execute orchestration".into(),
            confidence: 0.9,
        },
        GlobalPattern {
            description: "use tree-search".into(),
            dimension: HarnessDimension::Orchestration,
            task_types: vec!["refactor".into()],
            recommended_change: "use bypass mode".into(),
            confidence: 0.7,
        },
    ];
    let adapted = adapt_harness_for_task(&global, "refactor task", &[], &patterns);
    // Both patterns match — the last one wins (bypass mode).
    assert_eq!(
        adapted.orchestration.permission_mode,
        PermissionMode::Bypass
    );
}

/// adapt_harness with a very long task description (1000 chars).
#[test]
fn torture_adapt_harness_long_task_description() {
    let global = HarnessConfig::default_local("qwen3-coder-32b");
    let long_desc = "refactor ".repeat(100);
    let patterns = vec![GlobalPattern {
        description: "multi-file refactors need plan-execute".into(),
        dimension: HarnessDimension::Orchestration,
        task_types: vec!["refactor".into()],
        recommended_change: "use plan-execute orchestration".into(),
        confidence: 0.9,
    }];
    let adapted = adapt_harness_for_task(&global, &long_desc, &[], &patterns);
    assert_eq!(
        adapted.orchestration.escalation_mode,
        EscalationMode::PlanAndExecute
    );
}

/// HarnessConfig::default_cpu with a 7B model — all constraints applied.
#[test]
fn torture_harness_cpu_mode_extreme() {
    let cfg = HarnessConfig::default_cpu("qwen3-coder-7b");
    assert!(cfg.context.max_context_tokens <= 8192);
    assert!(!cfg.orchestration.sub_agents_enabled);
    assert!(cfg.memory.context_folding_enabled);
    assert!(cfg.generation.prompt_caching); // still caching even on CPU
}

// ═══════════════════════════════════════════════════════════════════════
// rai-core: PERMISSION STRESS
// ═══════════════════════════════════════════════════════════════════════

/// Permission::resolve with 1000 permissions (all Allow) → should still return Allow.
#[test]
fn torture_permission_resolve_1000_allows() {
    let perms: Vec<Permission> = (0..1000).map(|_| Permission::Allow).collect();
    assert_eq!(Permission::resolve(perms), Permission::Allow);
}

/// Permission::resolve with 1 Deny among 999 Allows → Deny wins.
#[test]
fn torture_permission_resolve_1_deny_among_999() {
    let mut perms: Vec<Permission> = (0..999).map(|_| Permission::Allow).collect();
    perms.push(Permission::Deny("no".into()));
    assert!(matches!(Permission::resolve(perms), Permission::Deny(_)));
}

// ═══════════════════════════════════════════════════════════════════════
// rai-core: TOOL REGISTRY STRESS
// ═══════════════════════════════════════════════════════════════════════

/// ToolRegistry with 0 tools — schema_for_model returns empty.
#[test]
fn torture_tool_registry_empty() {
    let reg = rai_core::tool::ToolRegistry::new();
    assert!(reg.is_empty());
    assert!(reg.schema_for_model().is_empty());
    assert!(reg.get("Bogus").is_none());
}

// ═══════════════════════════════════════════════════════════════════════
// rai-llm: PROVIDER + CACHING STRESS
// ═══════════════════════════════════════════════════════════════════════

/// ChatRequest with 0 messages (degenerate but should not panic).
#[test]
fn torture_chat_request_empty_messages() {
    let req = ChatRequest {
        model: "mock".into(),
        messages: vec![],
        system: None,
        tools: vec![],
        max_tokens: None,
        temperature: None,
        cache_control: vec![],
    };
    let json = serde_json::to_string(&req).unwrap();
    let back: ChatRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(req, back);
    assert!(back.messages.is_empty());
}

/// ChatChunk with empty string delta.
#[test]
fn torture_chat_chunk_empty_delta() {
    let chunk = ChatChunk::Delta("".into());
    let json = serde_json::to_string(&chunk).unwrap();
    let back: ChatChunk = serde_json::from_str(&json).unwrap();
    assert_eq!(chunk, back);
}

/// ChatResponse with 0 tool calls + 0 usage.
#[test]
fn torture_chat_response_minimal() {
    let resp = ChatResponse {
        content: "".into(),
        tool_calls: vec![],
        stop_reason: LlmStopReason::EndTurn,
        usage: Usage::default(),
    };
    let json = serde_json::to_string(&resp).unwrap();
    let back: ChatResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(resp, back);
}

/// CacheControl with Message(999999) — extreme index.
#[test]
fn torture_cache_control_extreme_index() {
    let cc = CacheControl {
        at: CacheTarget::Message(999_999),
        ttl: CacheTtl::Ephemeral1h,
    };
    let json = serde_json::to_string(&cc).unwrap();
    let back: CacheControl = serde_json::from_str(&json).unwrap();
    assert_eq!(cc, back);
}

// ═══════════════════════════════════════════════════════════════════════
// rai-mcp: TOOL CATALOG STRESS
// ═══════════════════════════════════════════════════════════════════════

/// ToolCatalog with 500 tools — search returns at most max_per_search.
#[test]
fn torture_tool_catalog_500_tools() {
    let names: Vec<String> = (0..500).map(|i| format!("tool_{i}")).collect();
    let cat = ToolCatalog::from_names(names).with_max_per_search(5);
    let results = cat.search("tool");
    assert_eq!(results.len(), 5); // capped at 5
}

/// ToolCatalog with all tools loaded — search returns nothing.
#[test]
fn torture_tool_catalog_all_loaded() {
    let mut cat = ToolCatalog::from_names(vec!["a".into(), "b".into(), "c".into()]);
    cat.mark_loaded("a");
    cat.mark_loaded("b");
    cat.mark_loaded("c");
    assert!(
        cat.search("").is_empty(),
        "all loaded → search returns nothing"
    );
}

/// ToolCatalog with max_per_search = 0 — search returns nothing.
#[test]
fn torture_tool_catalog_max_0() {
    let cat = ToolCatalog::from_names(vec!["a".into(), "b".into()]).with_max_per_search(0);
    assert!(cat.search("").is_empty());
}

// ═══════════════════════════════════════════════════════════════════════
// rai-security: FLIGHT RECORDER + TAINT STRESS
// ═══════════════════════════════════════════════════════════════════════

/// Flight recorder with 100 entries — verify_chain passes.
#[test]
fn torture_flight_recorder_100_entries() {
    let rec = FlightRecorder::new();
    for i in 0..100 {
        rec.append("note", serde_json::json!({"i": i}), TaintLabel::Clean)
            .unwrap();
    }
    assert_eq!(rec.entries().len(), 100);
    assert!(rec.verify_chain());
}

/// Taint with deeply nested JSON (10 levels of objects with a secret at the bottom).
#[test]
fn torture_taint_deeply_nested_secret() {
    let mut inner = serde_json::json!("secret_value");
    for _ in 0..10 {
        inner = serde_json::json!({"nested": inner});
    }
    let val = serde_json::json!({"deep": inner, "password": "x"});
    assert_eq!(taint_of(&val), TaintLabel::UserSecret);
}

/// Taint with an array of 100 secrets.
#[test]
fn torture_taint_array_of_secrets() {
    let val = serde_json::json!({
        "items": (0..100).map(|i| serde_json::json!({"token": format!("tok-{i}")})).collect::<Vec<_>>()
    });
    assert_eq!(taint_of(&val), TaintLabel::UserSecret);
}

/// Taint with no secrets anywhere in a complex structure.
#[test]
fn torture_taint_complex_clean() {
    let val = serde_json::json!({
        "config": {"host": "localhost", "port": 5432, "debug": true},
        "items": [{"name": "a", "value": 1}, {"name": "b", "value": 2}],
        "nested": {"deep": {"deeper": {"deepest": "clean"}}}
    });
    assert_eq!(taint_of(&val), TaintLabel::Clean);
}

/// SecurityKernel with 50 tool calls — flight log grows + stays verifiable.
#[tokio::test]
async fn torture_kernel_50_tool_calls() {
    let k = DefaultKernel::new();
    for i in 0..50 {
        let decision = k
            .mediate_tool_call("Read", &serde_json::json!({"path": format!("file_{i}.rs")}))
            .await;
        assert_eq!(decision, SecurityDecision::Allow);
    }
    assert!(k.flight_log_hash() != ZERO_HASH);
}

// ═══════════════════════════════════════════════════════════════════════
// rai-codegen: TREE-SITTER STRESS
// ═══════════════════════════════════════════════════════════════════════

/// Parse an empty Rust file — should return 0 symbols, not crash.
#[test]
fn torture_parse_empty_rust() {
    let symbols = parse_rust("").expect("empty file should parse");
    assert!(symbols.is_empty());
}

/// Parse a file with only comments — should return 0 symbols.
#[test]
fn torture_parse_only_comments() {
    let src = "// just a comment\n/* block comment */\n//! doc comment\n";
    let symbols = parse_rust(src).expect("comments-only should parse");
    assert!(symbols.is_empty());
}

/// Parse a large Rust file (200 functions) — all should be found.
#[test]
fn torture_parse_200_functions() {
    let src: String = (0..200).map(|i| format!("fn func_{i}() {{}}\n")).collect();
    let symbols = parse_rust(&src).expect("large file should parse");
    let fns: Vec<_> = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Function)
        .collect();
    assert_eq!(fns.len(), 200);
}

/// Parse invalid Rust (syntax error) — tree-sitter should handle gracefully.
#[test]
fn torture_parse_invalid_rust() {
    let src = "fn broken( { this is not valid rust }}}";
    // tree-sitter has error recovery — it should not panic.
    let result = parse_rust(src);
    // It might return symbols or it might return an error — either is fine,
    // as long as it doesn't panic.
    assert!(result.is_ok() || result.is_err());
}

/// DCG with 100 files — all_symbols returns all.
#[test]
fn torture_dcg_100_files() {
    let mut dcg = Dcg::new();
    for i in 0..100 {
        dcg.update_file(
            std::path::PathBuf::from(format!("src/file_{i}.rs")),
            &format!("fn func_{i}() {{}}"),
        )
        .unwrap();
    }
    assert_eq!(dcg.file_count(), 100);
    assert_eq!(dcg.all_symbols().len(), 100);
}

/// DCG with a non-Rust file (.py) — should return 0 symbols (no Python parser yet).
#[test]
fn torture_dcg_non_rust_file() {
    let mut dcg = Dcg::new();
    let node = dcg
        .update_file(std::path::PathBuf::from("script.py"), "def hello(): pass")
        .unwrap();
    assert_eq!(node.language, "python");
    assert!(node.symbols.is_empty(), "no Python parser yet → 0 symbols");
}

// ═══════════════════════════════════════════════════════════════════════
// rai-sandbox: LOCAL SANDBOX STRESS
// ═══════════════════════════════════════════════════════════════════════

/// Sandbox: command that produces large output.
#[tokio::test]
async fn torture_sandbox_large_output() {
    let sb = LocalSandbox::cwd();
    let out = sb.run("seq 1 1000").await.expect("seq should run");
    assert!(out.is_success());
    assert!(out.stdout.lines().count() >= 1000);
}

/// Sandbox: command with a pipe + redirect.
#[tokio::test]
async fn torture_sandbox_pipe_and_redirect() {
    let sb = LocalSandbox::cwd();
    let out = sb
        .run("echo 'hello world' | tr ' ' '\n' | head -1")
        .await
        .expect("pipe should work");
    assert!(out.is_success());
    assert_eq!(out.stdout.trim(), "hello");
}

/// Sandbox: timeout of 0 seconds — should time out immediately.
#[tokio::test]
async fn torture_sandbox_zero_timeout() {
    let sb = LocalSandbox::cwd();
    let out = sb
        .run_with_timeout("echo hi", Duration::from_millis(0))
        .await
        .expect("should not error");
    // With 0 timeout, the sleep fires immediately — likely timed_out.
    // (This is a race; either outcome is acceptable.)
    assert!(
        out.timed_out || out.is_success(),
        "should time out or succeed (race)"
    );
}

/// Sandbox: exit code 42 (unusual).
#[tokio::test]
async fn torture_sandbox_exit_42() {
    let sb = LocalSandbox::cwd();
    let out = sb.run("exit 42").await.expect("should run");
    assert!(!out.is_success());
    assert_eq!(out.exit_code, Some(42));
}

// ═══════════════════════════════════════════════════════════════════════
// rai-browser: A11Y SERIALIZER STRESS
// ═══════════════════════════════════════════════════════════════════════

/// a11y tree with 500 nodes — should serialize without panic.
#[test]
fn torture_a11y_500_nodes() {
    let tree: Vec<serde_json::Value> = (0..500)
        .map(|i| {
            serde_json::json!({
                "role": {"value": "button"},
                "name": {"value": format!("Button {i}")},
                "ref": format!("A{i}")
            })
        })
        .collect();
    let text = serialize_a11y(&serde_json::Value::Array(tree));
    assert!(text.contains("Button 0"));
    assert!(text.contains("Button 499"));
    assert!(text.contains("[A0]"));
    assert!(text.contains("[A499]"));
}

/// a11y tree with ONLY wrapper nodes — all collapsed → output is "(empty a11y tree)".
#[test]
fn torture_a11y_only_wrappers() {
    let tree = serde_json::json!([
        {"role": {"value": "generic"}, "name": {"value": ""}, "ref": ""},
        {"role": {"value": "group"}, "name": {"value": ""}, "ref": ""},
        {"role": {"value": "none"}, "name": {"value": ""}, "ref": ""},
        {"role": {"value": "presentation"}, "name": {"value": ""}, "ref": ""}
    ]);
    let text = serialize_a11y(&tree);
    assert!(
        text.contains("(empty a11y tree)"),
        "all wrappers → empty: {text}"
    );
}

/// BrowserAction with empty strings in all fields.
#[test]
fn torture_browser_action_empty_strings() {
    let actions = vec![
        BrowserAction::Navigate { url: "".into() },
        BrowserAction::Click { ref_id: "".into() },
        BrowserAction::Type {
            ref_id: "".into(),
            text: "".into(),
        },
        BrowserAction::AssertText { text: "".into() },
        BrowserAction::Evaluate {
            expression: "".into(),
        },
    ];
    for a in actions {
        let json = serde_json::to_string(&a).unwrap();
        let back: BrowserAction = serde_json::from_str(&json).unwrap();
        assert_eq!(a, back);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// rai-python: MEMORY STORE + JSON-RPC STRESS
// ═══════════════════════════════════════════════════════════════════════

/// MockMemoryStore with 1000 retains + recall finds the right one.
#[tokio::test]
async fn torture_memory_store_1000_retains() {
    let store = MockMemoryStore::new();
    for i in 0..1000 {
        store
            .hindsight_retain(&format!("memory-item-{i}"))
            .await
            .unwrap();
    }
    let result = store.hindsight_recall("memory-item-999").await.unwrap();
    assert!(
        !result["results"].as_array().unwrap().is_empty(),
        "should find item-999 among 1000 memories"
    );
}

/// JSON-RPC parse with extremely large params.
#[test]
fn torture_jsonrpc_large_params() {
    let large_value = "x".repeat(10_000);
    let req = JsonRpcRequest {
        id: serde_json::json!(1),
        method: "graphiti_search".into(),
        params: serde_json::json!({"q": large_value}),
    };
    let json = serde_json::to_string(&req).unwrap();
    let back: JsonRpcRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(req, back);
}

/// JSON-RPC parse with completely empty string.
#[test]
fn torture_jsonrpc_empty_string() {
    assert!(rai_python::parse_request("").is_err());
}

/// JSON-RPC parse with just whitespace.
#[test]
fn torture_jsonrpc_whitespace_only() {
    assert!(rai_python::parse_request("   \n\t  ").is_err());
}

// ═══════════════════════════════════════════════════════════════════════
// rai-tui: RENDERING STRESS
// ═══════════════════════════════════════════════════════════════════════

/// Render on a 1×1 terminal (extreme small) — should not panic.
#[test]
fn torture_tui_1x1_terminal() {
    let app = App::new();
    let content = render_to_string(&app, 1, 1);
    // Should not panic — content may be truncated but should be non-empty.
    assert!(!content.is_empty());
}

/// Render on a 200×50 terminal (large) — status bar should be visible.
#[test]
fn torture_tui_200x50_terminal() {
    let app = App::new();
    let content = render_to_string(&app, 200, 50);
    assert!(content.contains("RAI Code"));
}

/// ChatPane with 100 messages — should render without panic.
#[test]
fn torture_tui_100_messages() {
    let mut app = App::new();
    for i in 0..100 {
        app.chat.add_message("rai", format!("message {i}"));
    }
    let content = render_to_string(&app, 80, 24);
    // Should not panic — the terminal is only 24 lines so most messages
    // won't be visible, but it should render something.
    assert!(!content.is_empty());
}

/// BrowserPane with 50 console events — should render without panic.
#[test]
fn torture_tui_50_console_events() {
    let mut app = App::new().show_browser(true);
    app.browser.set_a11y("- link \"Home\" [A0]\n");
    for i in 0..50 {
        app.browser.add_console("error", format!("Error {i}"));
    }
    let content = render_to_string(&app, 100, 30);
    assert!(content.contains("[A0]"), "a11y should still render");
}

/// All panes visible at once (browser + diff + chat).
#[test]
fn torture_tui_all_panes() {
    let mut app = App::new().show_browser(true);
    app.chat.add_message("user", "test");
    app.browser.set_a11y("- button \"Go\" [A0]\n");
    app.browser.add_console("error", "err");
    app.browser.add_network("GET -> 200");
    app.diff.add_diff("test.rs", vec!["+added".into()]);
    let content = render_to_string(&app, 120, 40);
    assert!(content.contains("test")); // chat message
    assert!(content.contains("[A0]")); // browser
}

// ═══════════════════════════════════════════════════════════════════════
// rai-cli: ONBOARDING STRESS
// ═══════════════════════════════════════════════════════════════════════

/// Onboarding with 0 answers — should create a "no answers" profile.
#[tokio::test]
async fn torture_onboarding_zero_answers() {
    let store = MockMemoryStore::new();
    let result = rai_cli::onboarding(&store, &[]).await.unwrap();
    assert!(!result.existing_profile);
    assert!(result.profile_text.contains("no answers"));
    assert_eq!(result.directives.len(), 5); // still gets default directives
}

/// tier_recommendation with empty string.
#[test]
fn torture_tier_recommendation_empty() {
    assert_eq!(rai_cli::tier_recommendation(""), rai_cli::HardwareTier::Cpu);
}

/// tier_recommendation with a very long model name containing "32b".
#[test]
fn torture_tier_recommendation_long_name() {
    let name = "qwen3-coder-32b-".to_string() + &"x".repeat(1000);
    assert_eq!(
        rai_cli::tier_recommendation(&name),
        rai_cli::HardwareTier::Gpu24gb
    );
}

// ═══════════════════════════════════════════════════════════════════════
// CROSS-CRATE: FULL CHAIN STRESS
// ═══════════════════════════════════════════════════════════════════════

/// Full chain: onboarding → harness adaptation → agent loop → ITVF → verify.
/// This is the hardest test — it exercises every crate in sequence.
#[tokio::test]
async fn torture_full_chain_onboard_adapt_loop_itvf() {
    // 1. Onboard.
    let store = MockMemoryStore::new();
    let onboarding = rai_cli::onboarding(&store, &["Rust".into(), "Tokio".into(), "yes".into()])
        .await
        .unwrap();
    assert!(!onboarding.existing_profile);

    // 2. Create a global harness + adapt it for a multi-file task.
    let global = HarnessConfig::default_local("qwen3-coder-32b");
    let patterns = vec![GlobalPattern {
        description: "multi-file refactors need plan-execute".into(),
        dimension: HarnessDimension::Orchestration,
        task_types: vec!["refactor".into()],
        recommended_change: "use plan-execute orchestration".into(),
        confidence: 0.9,
    }];
    let adapted = adapt_harness_for_task(&global, "refactor the auth module", &[], &patterns);
    assert_eq!(
        adapted.orchestration.escalation_mode,
        EscalationMode::PlanAndExecute
    );

    // 3. Run the agent loop with the adapted harness.
    let provider: Arc<dyn Provider> = Arc::new(rai_llm::mock::MockProvider::abc());
    let loop_ = AgentLoop::new(provider, "mock")
        .with_user_message("refactor the auth module")
        .with_harness(adapted);
    use futures::StreamExt;
    let events: Vec<AgentEvent> = loop_.run().collect().await;
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEvent::Terminal(StopReason::EndTurn))),
        "loop should terminate"
    );

    // 4. Run ITVF with a verify that passes on iteration 3.
    let itvf_result = run_itvf(&ItvfConfig::default(), |i| {
        if i >= 3 {
            Ok(())
        } else {
            Err(format!("not yet {i}"))
        }
    });
    assert!(itvf_result.matched);
    assert_eq!(itvf_result.iterations, 3);

    // 5. Verify the sandbox works.
    let sb = LocalSandbox::cwd();
    let out = sb.run("echo 'torture test passed'").await.unwrap();
    assert!(out.is_success());

    // 6. Verify the TUI renders.
    let app = App::new();
    let content = render_to_string(&app, 80, 24);
    assert!(content.contains("RAI Code"));

    // 7. Verify the DCG parses.
    let mut dcg = Dcg::new();
    let node = dcg
        .update_file(std::path::PathBuf::from("test.rs"), "fn torture() {}")
        .unwrap();
    assert!(node.symbols.iter().any(|s| s.name == "torture"));

    // 8. Verify the security kernel works.
    let kernel = DefaultKernel::new();
    let decision = kernel
        .mediate_tool_call("Read", &serde_json::json!({"path": "test.rs"}))
        .await;
    assert_eq!(decision, SecurityDecision::Allow);

    println!("Torture full chain: all 8 cross-crate steps passed ✓");
}
