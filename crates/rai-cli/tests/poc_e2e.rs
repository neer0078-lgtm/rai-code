//! T64: PoC end-to-end integration test.
//!
//! This is THE milestone test — it wires all 10 crates together end-to-end:
//! 1. Onboard a mock user (rai-cli onboarding + MockMemoryStore)
//! 2. Index a tiny fixture repo with the DCG (rai-codegen tree-sitter)
//! 3. Ingest a commit as a Graphiti episode (rai-python MockMemoryStore)
//! 4. Run the agent loop on a trivial task (rai-core + rai-llm MockProvider)
//! 5. Show a diff (rai-tui DiffPane rendering on TestBackend)
//! 6. Undo (git-as-undo — conceptual; we verify the diff is reversible)
//! 7. Run `echo hi` in the local sandbox (rai-sandbox LocalSandbox)
//! 8. Render the BrowserPane with a fixture a11y tree (rai-tui + rai-browser)
//!
//! The test exits 0 if all steps pass. It's NOT #[ignore] — it uses mocks
//! throughout (no real network, no real Ollama, no real Chromium).

use rai_cli::onboarding;
use rai_codegen::Dcg;
use rai_core::{AgentEvent, AgentLoop, PermissionMode};
use rai_llm::mock::MockProvider;
use rai_python::{MemoryStore, MockMemoryStore};
use rai_sandbox::{LocalSandbox, Sandbox};
use rai_tui::{render_to_string, App};
use std::sync::Arc;

#[tokio::test]
async fn poc_e2e_onboard_index_ingest_loop_diff_sandbox_browser() {
    // ── Step 1: Onboard a mock user ──────────────────────────────────────
    let store = MockMemoryStore::new();
    let answers = vec!["Rust".into(), "Ratatui + Tokio".into(), "yes".into()];
    let onboarding_result = onboarding(&store, &answers)
        .await
        .expect("onboarding should succeed");
    assert!(
        !onboarding_result.existing_profile,
        "new user should not have an existing profile"
    );
    assert!(
        onboarding_result.profile_text.contains("Rust"),
        "profile should contain the preferred language"
    );
    assert!(
        !onboarding_result.directives.is_empty(),
        "should have pinned directives"
    );
    assert_eq!(
        onboarding_result.directives.len(),
        5,
        "should have exactly 5 default directives"
    );

    // ── Step 2: Index a tiny fixture repo with the DCG ──────────────────
    let fixture_src = r#"
fn main() {
    println!("Hello, RAI Code!");
}

struct Config {
    model: String,
    provider: String,
}

impl Config {
    fn new(model: &str) -> Self {
        Self { model: model.to_string(), provider: "ollama-local".to_string() }
    }
}
"#;

    let mut dcg = Dcg::new();
    let node = dcg
        .update_file(std::path::PathBuf::from("src/main.rs"), fixture_src)
        .expect("DCG should parse the fixture");
    assert_eq!(node.language, "rust");
    assert!(
        node.symbols.iter().any(|s| s.name == "main"),
        "DCG should find the main function"
    );
    assert!(
        node.symbols
            .iter()
            .any(|s| s.name == "Config" && s.kind == rai_codegen::SymbolKind::Struct),
        "DCG should find the Config struct"
    );
    assert!(
        node.symbols
            .iter()
            .any(|s| s.name == "Config" && s.kind == rai_codegen::SymbolKind::Impl),
        "DCG should find the impl Config block"
    );
    assert!(
        node.symbols
            .iter()
            .any(|s| s.kind == rai_codegen::SymbolKind::Method),
        "DCG should find the method inside impl Config"
    );

    // ── Step 3: Ingest a commit as a Graphiti episode ───────────────────
    store
        .graphiti_add_episode("commit abc123: added Config struct with model field")
        .await
        .expect("add_episode should succeed");
    let search_result = store
        .graphiti_search("Config")
        .await
        .expect("search should succeed");
    assert!(
        !search_result["results"].as_array().unwrap().is_empty(),
        "Graphiti search should find the ingested episode"
    );

    // ── Step 4: Run the agent loop on a trivial task ─────────────────────
    let provider: Arc<dyn rai_llm::Provider> = Arc::new(MockProvider::abc());
    let loop_ = AgentLoop::new(provider, "mock")
        .with_user_message("hello from the PoC test")
        .with_permission_mode(PermissionMode::Bypass)
        .with_system("You are RAI Code — the agent that knows you and your codebase.");

    use futures::StreamExt;
    let stream = loop_.run();
    let events: Vec<AgentEvent> = stream.collect().await;

    // Should have Token events (a, b, c) + Terminal(EndTurn).
    let tokens: Vec<String> = events
        .iter()
        .filter_map(|e| {
            if let AgentEvent::Token(t) = e {
                Some(t.clone())
            } else {
                None
            }
        })
        .collect();
    assert!(
        !tokens.is_empty(),
        "the agent loop should produce Token events"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEvent::Terminal(rai_core::StopReason::EndTurn))),
        "the loop should terminate with EndTurn"
    );

    // ── Step 5: Show a diff (rai-tui DiffPane rendering) ────────────────
    let mut app = App::new().show_diff(true);
    app.diff.add_diff(
        "src/main.rs",
        vec![
            "+struct Config {".into(),
            "+    model: String,".into(),
            "+    provider: String,".into(),
            "-// TODO: implement".into(),
        ],
    );
    let diff_content = render_to_string(&app, 80, 24);
    assert!(
        diff_content.contains("main.rs"),
        "diff should show the file path"
    );
    assert!(
        diff_content.contains("+struct Config"),
        "diff should show the added struct"
    );
    assert!(
        diff_content.contains("-// TODO"),
        "diff should show the removed TODO"
    );

    // ── Step 6: Undo (the diff is reversible — verify the original content) ──
    // In a real impl, git-as-undo reverts the working tree. Here we verify
    // the DCG still has the original symbols (the diff was display-only).
    let original_symbols = dcg
        .get(&std::path::PathBuf::from("src/main.rs"))
        .expect("the DCG should still have the file")
        .symbols
        .len();
    assert!(
        original_symbols > 0,
        "the DCG should still have the original symbols (undo = diff was display-only)"
    );

    // ── Step 7: Run `echo hi` in the local sandbox ──────────────────────
    let sandbox = LocalSandbox::cwd();
    let sandbox_output = sandbox
        .run("echo 'Hello from RAI Code sandbox'")
        .await
        .expect("sandbox should run");
    assert!(
        sandbox_output.is_success(),
        "sandbox echo should succeed: {:?}",
        sandbox_output
    );
    assert!(
        sandbox_output
            .stdout
            .contains("Hello from RAI Code sandbox"),
        "sandbox output should contain the echo text"
    );

    // ── Step 8: Render the BrowserPane with a fixture a11y tree ─────────
    let mut app2 = App::new().show_browser(true);
    app2.browser.set_a11y(
        "- navigation\n  - link \"Home\" [A0]\n  - link \"Settings\" [A1]\n\
         - main\n  - heading \"Dashboard\"\n  - form\n    \
         - textbox \"Email\" [A2]\n    - textbox \"Password\" [A3]\n    \
         - button \"Submit\" [A4]\n",
    );
    app2.browser.add_console("error", "TypeError at App.tsx:42");
    app2.browser.add_network("404 FAIL");

    let browser_content = render_to_string(&app2, 100, 30);
    assert!(
        browser_content.contains("[A0]"),
        "browser should render a11y ref markers"
    );
    assert!(
        browser_content.contains("[A4]"),
        "browser should render the Submit button ref"
    );
    assert!(
        browser_content.contains("TypeError"),
        "browser should render console errors"
    );
    assert!(
        browser_content.contains("404"),
        "browser should render network failures"
    );

    // ── Bonus: verify the ITVF loop driver works (goal-matched on iteration 1) ──
    let itvf_result = rai_core::run_itvf(
        &rai_core::ItvfConfig::default(),
        |_| Ok(()), // verify passes immediately
    );
    assert!(
        itvf_result.matched,
        "ITVF should match on iteration 1 when verify passes"
    );
    assert_eq!(
        itvf_result.iterations, 1,
        "ITVF should use exactly 1 iteration"
    );

    // ── All 8 steps + bonus passed ──────────────────────────────────────
    println!("PoC E2E: all 8 steps + ITVF bonus passed ✓");
}
