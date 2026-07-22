# TASKS.md — RAI Code ITVF Work Queue

> Each task is **ITVF-sized**: small, self-contained, with a machine-checkable `GOAL`. Run ITVF (Implement → Test → Verify → Fix) on each until the goal is matched, then commit and move to the next. See `docs/ITVF-LOOP.md`.
>
> **Read `BUILD.md` first.** Follow conventions (§6), clean-room rules (§7), commit format (§8).
>
- *Built by RAI Labs P. Ltd. · [www.railabs.in](https://www.railabs.in) · reach@railabs.in*

**Legend:** `[ ]` unblocked · `[x]` done · `[~]` in progress · `[!]` blocked · `[ ]*` post-PoC
**Verify ground truth:** `cargo check -p <crate>` · `cargo test -p <crate>` · `cargo clippy -p <crate> -- -D warnings` · `cargo fmt -- --check`

---

## Phase 0 — Workspace health (do these first; they're tiny)

### T00 — [x] Workspace compiles as scaffolded
- **GOAL:** `cargo check` at repo root exits 0 with all 11 crates compiling (stubs allowed).
- **FILES:** `Cargo.toml`, `crates/*/Cargo.toml`, `crates/*/src/lib.rs` (fix only what's needed to compile).
- **TESTS:** none (compile-only).
- **VERIFY:** `cargo check` (root) — exit 0; `cargo fmt -- --check` — exit 0.
- **DEPS:** none.
- **NOTES:** The scaffold compiles today; if a crate's stub has a typo, fix it. Don't expand scope. This task exists to confirm the baseline.

### T01 — [x] `rai-core` types compile under clippy strict
- **GOAL:** `cargo clippy -p rai-core -- -D warnings` exits 0 with zero warnings (fix any dead-code/missing-doc warnings by adding `#[allow(dead_code)]` only where the stub is genuinely a placeholder, with a comment `// TODO(T<id>): implement in <phase>`).
- **FILES:** `crates/rai-core/src/*.rs`.
- **TESTS:** none.
- **VERIFY:** `cargo clippy -p rai-core -- -D warnings` — exit 0; `cargo fmt -- --check` — exit 0.
- **DEPS:** T00.

### T02 — [x] `StopReason` round-trips through serde
- **GOAL:** `StopReason` serializes to JSON and deserializes back equal to the original, for all 11 variants.
- **FILES:** `crates/rai-core/src/loop_.rs` (add `#[derive(PartialEq)]` if missing + the test).
- **TESTS:** `#[test] fn stop_reason_serde_roundtrip()` covering all 11 variants.
- **VERIFY:** `cargo test -p rai-core stop_reason_serde` — pass; `cargo clippy -p rai-core -- -D warnings` — exit 0.
- **DEPS:** T01.

### T03 — [x] `StopReason` exhaustiveness test
- **GOAL:** A test that asserts `StopReason` has exactly the 11 variants currently defined (fails if one is added/removed without intent).
- **FILES:** `crates/rai-core/src/loop_.rs`.
- **TESTS:** `#[test] fn stop_reason_has_eleven_variants()` — constructs all 11, asserts `len == 11`. (Use a const `EXPECTED_VARIANTS: usize = 11` so changes are intentional.)
- **VERIFY:** `cargo test -p rai-core stop_reason_has_eleven` — pass; `cargo clippy -p rai-core -- -D warnings` — exit 0.
- **DEPS:** T02.

### T04 — [x] `Permission` resolve (most-restrictive-wins)
- **GOAL:** `Permission::resolve(vec)` returns the most restrictive (Deny > Ask > Allow) across mixed inputs, including empty (→ Deny with reason) and all-Allow (→ Allow).
- **FILES:** `crates/rai-core/src/perm.rs`.
- **TESTS:** `#[test] fn permission_resolve_*` — at least 5 cases: empty→Deny, all Allow→Allow, one Deny→Deny, one Ask no Deny→Ask, mixed→Deny.
- **VERIFY:** `cargo test -p rai-core permission_resolve` — pass; `cargo clippy -p rai-core -- -D warnings` — exit 0.
- **DEPS:** T01.

### T05 — [x] `PermissionMode` defaults to Approval + serde
- **GOAL:** `PermissionMode::default() == Approval`; all 5 modes round-trip through serde as snake_case.
- **FILES:** `crates/rai-core/src/perm.rs`.
- **TESTS:** `#[test] fn permission_mode_default_and_serde()`.
- **VERIFY:** `cargo test -p rai-core permission_mode` — pass; clippy clean.
- **DEPS:** T04.

### T06 — [x] `classify_task` escalation heuristics
- **GOAL:** `classify_task(...)` returns the right `EscalationMode` for: single-file low→Agentless, >3 files→PlanAndExecute, requires_exploration→Explore, parallelizable→DynamicWorkflow, verification_critical→TreeSearch, else→None.
- **FILES:** `crates/rai-core/src/escalation.rs`.
- **TESTS:** `#[test] fn classify_task_*` — one assertion per branch + the None default.
- **VERIFY:** `cargo test -p rai-core classify_task` — pass; clippy clean.
- **DEPS:** T01.

### T07 — [x] `AutoCompactSummary` defaults + serde
- **GOAL:** `AutoCompactSummary::default()` produces empty fields; the struct round-trips through serde.
- **FILES:** `crates/rai-core/src/compact.rs`.
- **TESTS:** `#[test] fn autocompact_summary_default_and_serde()`.
- **VERIFY:** `cargo test -p rai-core autocompact_summary` — pass; clippy clean.
- **DEPS:** T01.

### T08 — [x] `CompactConfig::default` thresholds are sane
- **GOAL:** `CompactConfig::default()` has `collapse_threshold > autocompact_threshold`, `post_compact_keep_files <= 5`, `post_compact_keep_user_msgs >= 5`.
- **TESTS:** `#[test] fn compact_config_default_sane()` asserting the above invariants.
- **VERIFY:** `cargo test -p rai-core compact_config` — pass; clippy clean.
- **DEPS:** T07.

### T09 — [x] `Isolation` defaults to Worktree + serde
- **GOAL:** `Isolation::default() == Worktree`; all 3 variants round-trip through serde as snake_case.
- **FILES:** `crates/rai-core/src/subagent.rs`.
- **TESTS:** `#[test] fn isolation_default_and_serde()`.
- **VERIFY:** `cargo test -p rai-core isolation` — pass; clippy clean.
- **DEPS:** T01.

### T10 — [x] `AppState::default` is empty + `SharedAppState` constructs
- **GOAL:** `AppState::default()` has empty messages, no streaming token, default panes, Approval mode, None escalation; `Arc::new(RwLock::new(AppState::default()))` compiles.
- **FILES:** `crates/rai-core/src/state.rs`.
- **TESTS:** `#[test] fn app_state_default_empty()`.
- **VERIFY:** `cargo test -p rai-core app_state_default` — pass; clippy clean.
- **DEPS:** T01.

---

## Phase 1 — `rai-core` loop (the heart)

### T11 — [x] `AgentEvent` serde + the `Terminal` variant carries `StopReason`
- **GOAL:** `AgentEvent` round-trips through serde for all variants; `AgentEvent::Terminal(StopReason::EndTurn)` survives the round trip.
- **FILES:** `crates/rai-core/src/loop_.rs` (add `#[derive(PartialEq)]` to `AgentEvent` if needed + the test).
- **TESTS:** `#[test] fn agent_event_serde_roundtrip()`.
- **VERIFY:** `cargo test -p rai-core agent_event_serde` — pass; clippy clean.
- **DEPS:** T03.

### T12 — [x] `Tool` trait shape compiles with a concrete read-only tool
- **GOAL:** A `#[cfg(test)]` `EchoTool` implementing `Tool` (`is_concurrency_safe() == true`, `execute` returns the args as `ToolContent::Text`) compiles and its `execute` returns the expected result.
- **FILES:** `crates/rai-core/src/tool.rs`.
- **TESTS:** `#[test] fn echo_tool_executes()` — construct `EchoTool`, call `execute` with a `ToolCall`, assert the `ToolContent::Text` matches.
- **VERIFY:** `cargo test -p rai-core echo_tool` — pass; clippy clean.
- **DEPS:** T01.

### T13 — [x] `ToolContent` structured-result serde
- **GOAL:** All 5 `ToolContent` variants round-trip through serde; `ToolResult { is_error: true }` survives.
- **FILES:** `crates/rai-core/src/tool.rs`.
- **TESTS:** `#[test] fn tool_content_serde_roundtrip()`.
- **VERIFY:** `cargo test -p rai-core tool_content_serde` — pass; clippy clean.
- **DEPS:** T12.

### T14 — [x] `StreamingToolExecutor` concurrency classifier (scaffold logic)
- **GOAL:** A pure function `can_execute_tool(pending: &[bool], incoming: bool) -> bool` (true iff all pending are concurrency-safe AND incoming is concurrency-safe OR pending is empty) is correct for all input combinations.
- **FILES:** `crates/rai-core/src/tool.rs` (extract the classifier as a testable pure fn).
- **TESTS:** `#[test] fn can_execute_tool_*` — empty+safe→true, empty+unsafe→true, all-safe+safe→true, any-unsafe→false, safe-pending+unsafe-incoming→false.
- **VERIFY:** `cargo test -p rai-core can_execute_tool` — pass; clippy clean.
- **DEPS:** T12.

### T15 — [x] `PermissionResolver::new` + a `resolve(call)` stub that respects mode
- **GOAL:** `PermissionResolver::new(Plan)` and `Bypass` and `Approval` construct; a stub `resolve(tool_name, args)` returns `Allow` for read-only tools (`Read`/`Grep`/`Glob`) regardless of mode, `Ask` for write tools under `Approval`/`Plan`, `Allow` for all under `Bypass`.
- **FILES:** `crates/rai-core/src/perm.rs`.
- **TESTS:** `#[test] fn resolver_modes_*` — Approval+Edit→Ask, Bypass+Edit→Allow, Plan+Read→Allow, Plan+Edit→Ask.
- **VERIFY:** `cargo test -p rai-core resolver_modes` — pass; clippy clean.
- **DEPS:** T04, T05.

### T16 — [x] `Hook` trait + a `HookOutcome::resolve` aggregator
- **GOAL:** A pure `aggregate_hooks(outcomes: Vec<HookOutcome>) -> HookOutcome` that returns `Deny` if any Deny, else `ShortCircuit` if any, else `ModifyInput` if any (last wins), else `Allow`. (Most-restrictive-wins, with Deny dominant.)
- **FILES:** `crates/rai-core/src/hook.rs`.
- **TESTS:** `#[test] fn aggregate_hooks_*` — empty→Allow, one Deny→Deny, Deny+Allow→Deny, ModifyInput+Allow→ModifyInput, ShortCircuit+Allow→ShortCircuit.
- **VERIFY:** `cargo test -p rai-core aggregate_hooks` — pass; clippy clean.
- **DEPS:** T01.

### T17 — [x] `AgentLoop::run` yields a `Terminal(EndTurn)` for an empty config (skeleton)
- **GOAL:** `AgentLoop::run(AgentLoop::default_or_new())` produces a `Stream` that yields exactly one event: `AgentEvent::Terminal(StopReason::EndTurn)`. (This is the skeleton — the real loop comes in T28.)
- **FILES:** `crates/rai-core/src/loop_.rs` (the `run` body already does this; make it testable).
- **TESTS:** `#[tokio::test] async fn agent_loop_yields_endturn()` — collect the stream, assert exactly one Terminal/EndTurn.
- **VERIFY:** `cargo test -p rai-core agent_loop_yields` — pass; clippy clean.
- **DEPS:** T11.
- **NOTES:** Use `futures::StreamExt::collect::<Vec<_>>()`.

---

## Phase 2 — `rai-llm` (multi-provider client)

### T18 — [x] `rai-llm` lib compiles + a `Provider` trait shape
- **GOAL:** `rai-llm` compiles with a `Provider` async trait (`async fn stream(req) -> Stream<ChatChunk>`, `async fn complete(req) -> ChatResponse`, `fn name() -> &str`) and an `AnthropicProvider` stub.
- **FILES:** `crates/rai-llm/src/{lib.rs,provider.rs,client.rs}`.
- **TESTS:** `#[test] fn provider_trait_compiles()` (a no-op test that constructs the stub).
- **VERIFY:** `cargo check -p rai-llm` — exit 0; `cargo clippy -p rai-llm -- -D warnings` — exit 0.
- **DEPS:** T00.
- **NOTES:** Don't wire genai yet — just the trait shape + stub. genai wiring is T20.

### T19 — [x] `ChatChunk` + `ChatResponse` types serde
- **GOAL:** `ChatChunk { delta: Option<String>, tool_calls: Vec<ToolCallChunk>, finish: Option<StopReason> }` and `ChatResponse { content, tool_calls, stop_reason, usage }` round-trip through serde.
- **FILES:** `crates/rai-llm/src/provider.rs`.
- **TESTS:** `#[test] fn chat_chunk_and_response_serde()`.
- **VERIFY:** `cargo test -p rai-llm chat_chunk` — pass; clippy clean.
- **DEPS:** T18.

### T20 — [x] genai-backed `Client` with a custom-endpoint config (no network in test)
- **GOAL:** A `Client::new(Config { base_url, model, api_key })` constructs a `genai::Client` (or wraps it); a `Client::endpoint_for(model)` returns the right URL for known providers (anthropic, openai, ollama-local) **without** making a network call.
- **FILES:** `crates/rai-llm/src/client.rs`.
- **TESTS:** `#[test] fn endpoint_for_known_providers()` — anthropic→api.anthropic.com, ollama-local→http://localhost:11434, etc.
- **VERIFY:** `cargo test -p rai-llm endpoint_for` — pass; clippy clean.
- **DEPS:** T18, T19.
- **NOTES:** genai supports custom endpoints; this is config-only, no HTTP in the test.

### T21 — [x] `CacheControl` prompt-caching config (Anthropic)
- **GOAL:** A `CacheControl` struct (`{ kind: ephemeral|persistent, ttl: 5min|1hr }`) + a `with_cache_breakpoint(messages, at_index)` helper that inserts a `cache_control` marker; serde round-trip; the helper is pure (no network).
- **FILES:** `crates/rai-llm/src/client.rs` (or a new `caching.rs`).
- **TESTS:** `#[test] fn cache_control_marker_inserted()` — assert the marker lands at the right index.
- **VERIFY:** `cargo test -p rai-llm cache_control` — pass; clippy clean.
- **DEPS:** T20.

---

## Phase 3 — `rai-mcp` (rmcp wrapper)

### T22 — [x] `rai-mcp` lib compiles + `McpClient` stub
- **GOAL:** `rai-mcp` compiles with an `McpClient` stub (`async fn list_tools()`, `async fn call_tool(name, args)`) and an `McpServer` stub. No real rmcp wiring yet.
- **FILES:** `crates/rai-mcp/src/{lib.rs,client.rs,server.rs}`.
- **TESTS:** `#[test] fn mcp_stubs_compile()`.
- **VERIFY:** `cargo check -p rai-mcp` — exit 0; clippy clean.
- **DEPS:** T00.

### T23 — [x] rmcp `TokioChildProcess` spawn helper (no real server in test)
- **GOAL:** A `spawn_stdio_server(cmd: &str) -> anyhow::Result<...>` that builds the rmcp `TokioChildProcess` transport for a stdio MCP server; the helper's signature + error path is testable **without** spawning a real server (test the config builder, not the spawn).
- **FILES:** `crates/rai-mcp/src/client.rs`.
- **TESTS:** `#[test] fn spawn_stdio_server_config_builds()` — assert the transport config builds for a dummy command.
- **VERIFY:** `cargo test -p rai-mcp spawn_stdio` — pass; clippy clean.
- **DEPS:** T22.

### T24 — [x] ToolSearch catalog (client-side deferred tools)
- **GOAL:** A `ToolCatalog { names: Vec<String>, loaded: HashSet<String> }` with `search(query, max=5) -> Vec<&str>` (keyword substring match, returns up to 5 not-yet-loaded) and `mark_loaded(name)`. Pure, no network.
- **FILES:** `crates/rai-mcp/src/client.rs` (or a new `toolsearch.rs`).
- **TESTS:** `#[test] fn toolsearch_*` — empty query, query matching 3 (returns 3), max=2 caps at 2, loaded excluded.
- **VERIFY:** `cargo test -p rai-mcp toolsearch` — pass; clippy clean.
- **DEPS:** T22.

---

## Phase 4 — `rai-security` (AgentK pattern, native Rust)

### T25 — [x] `rai-security` lib compiles + `SecurityKernel` trait
- **GOAL:** `rai-security` compiles with a `SecurityKernel` async trait (`async fn mediate_tool_call(tool, args) -> Permission`, `async fn check_syscall(syscall) -> Permission`, `fn flight_log_hash() -> String`) and a `DefaultKernel` stub (allow-all for now — hardened later).
- **FILES:** `crates/rai-security/src/{lib.rs,kernel.rs}`.
- **TESTS:** `#[test] fn security_kernel_trait_compiles()`.
- **VERIFY:** `cargo check -p rai-security` — exit 0; clippy clean.
- **DEPS:** T00.

### T26 — [x] `FlightRecorder` append + hash-chain integrity
- **GOAL:** A `FlightRecorder` that appends JSONL entries to a `Vec<u8>` buffer; each entry's hash = `sha256(prev_hash || entry_json)`; a `verify_chain()` returns `true` for an unmodified chain, `false` if any byte is flipped.
- **FILES:** `crates/rai-security/src/flight.rs` (add `sha2` dep to the crate).
- **TESTS:** `#[test] fn flight_recorder_*` — append 3 entries, verify true; flip a byte, verify false.
- **VERIFY:** `cargo test -p rai-security flight_recorder` — pass; clippy clean.
- **DEPS:** T25.

### T27 — [x] `TaintLabel` propagate through tool args
- **GOAL:** A `TaintLabel` enum (`Clean`, `UserSecret`, `UserData`) + a `taint_of(args: &serde_json::Value) -> TaintLabel` that marks any value containing a key named `secret`/`password`/`token`/`api_key` (case-insensitive) as `UserSecret`, recursively.
- **FILES:** `crates/rai-security/src/kernel.rs`.
- **TESTS:** `#[test] fn taint_of_*` — clean args→Clean, `{secret: "x"}`→UserSecret, nested `{"db": {"password": 1}}`→UserSecret.
- **VERIFY:** `cargo test -p rai-security taint_of` — pass; clippy clean.
- **DEPS:** T25.

---

## Phase 5 — `rai-codegen` (DCG layer 1)

### T28 — [x] `rai-codegen` lib compiles + `Dcg` struct
- **GOAL:** `rai-codegen` compiles with a `Dcg` struct (`{ files: HashMap<PathBuf, FileNode> }`) and a `FileNode { path, symbols: Vec<Symbol>, language }` + `Symbol { name, kind, span }` + `SymbolKind` enum (Function/Class/Method/Module/...).
- **FILES:** `crates/rai-codegen/src/{lib.rs,dcg.rs}`.
- **TESTS:** `#[test] fn dcg_constructs()`.
- **VERIFY:** `cargo check -p rai-codegen` — exit 0; clippy clean.
- **DEPS:** T00.

### T29 — [x] tree-sitter parse a tiny Rust file → symbols
- **GOAL:** A `parse_rust(path, src) -> Vec<Symbol>` that uses `tree-sitter` + the rust grammar to extract top-level `fn`/`struct`/`enum`/`impl` symbols with names + spans. (Add `tree-sitter-rust` dev-dep.)
- **FILES:** `crates/rai-codegen/src/dcg.rs`.
- **TESTS:** `#[test] fn parse_rust_extracts_symbols()` — given `"fn foo() {}\nstruct Bar { x: i32 }\nenum Baz { A, B }\nimpl Bar { fn m(&self){} }"`, returns foo (Function), Bar (Class/Struct), Baz (Class/Enum), Bar::m (Method).
- **VERIFY:** `cargo test -p rai-codegen parse_rust` — pass; clippy clean.
- **DEPS:** T28.

### T30 — [x] `Dcg::update_file` incremental (re-parse one file)
- **GOAL:** `Dcg::update_file(path, src)` re-parses one file and replaces its `FileNode`; querying the same path returns updated symbols; querying a different path is unchanged.
- **FILES:** `crates/rai-codegen/src/dcg.rs`.
- **TESTS:** `#[test] fn dcg_update_file_*` — add file A, add file B, update A → A's symbols reflect new content, B unchanged.
- **VERIFY:** `cargo test -p rai-codegen dcg_update_file` — pass; clippy clean.
- **DEPS:** T29.

---

## Phase 6 — `rai-sandbox` (pluggable)

### T31 — [x] `Sandbox` trait + `LocalSandbox` (subprocess)
- **GOAL:** `Sandbox` async trait (`start/run/stop`) compiles; `LocalSandbox` runs a command via `tokio::process::Command`, captures stdout/stderr, returns `SandboxOutput { stdout, stderr, exit_code }`.
- **FILES:** `crates/rai-sandbox/src/{lib.rs,sandbox.rs}`.
- **TESTS:** `#[tokio::test] async fn local_sandbox_runs_echo()` — `run("echo hi")` → stdout contains "hi", exit 0.
- **VERIFY:** `cargo test -p rai-sandbox local_sandbox` — pass; clippy clean.
- **DEPS:** T00.

### T32 — [x] `LocalSandbox` enforces a timeout + captures non-zero exit
- **GOAL:** `LocalSandbox::run_with_timeout(cmd, secs)` kills the process after the timeout and returns `SandboxOutput { timed_out: true, .. }`; a failing command returns the non-zero exit code + stderr.
- **FILES:** `crates/rai-sandbox/src/sandbox.rs`.
- **TESTS:** `#[tokio::test] async fn local_sandbox_timeout_and_failure()` — `sleep 5` with 1s timeout → timed_out; `false` → exit 1.
- **VERIFY:** `cargo test -p rai-sandbox timeout_and_failure` — pass; clippy clean.
- **DEPS:** T31.

### T33 — [x] E2B client stub (feature-gated, no network)
- **GOAL:** Behind `feature = "e2b"`, an `E2bSandbox` stub that constructs an `E2bConfig { api_key, template }` and has `start/run/stop` signatures; the config builder is testable without network.
- **FILES:** `crates/rai-sandbox/src/sandbox.rs`.
- **TESTS:** `#[cfg(feature = "e2b")] #[test] fn e2b_config_builds()`.
- **VERIFY:** `cargo test -p rai-sandbox --features e2b e2b_config` — pass; clippy clean (with --features e2b).
- **DEPS:** T31.

---

## Phase 7 — `rai-browser` (chromiumoxide + ratatui-image)

### T34 — [x] `rai-browser` lib compiles + `BrowserTool` enum
- **GOAL:** `rai-browser` compiles with a `BrowserAction` enum (`Navigate(url)`, `Snapshot`, `Click(ref)`, `Type(ref, text)`, `Screenshot`, `GetConsoleErrors`, `GetNetworkFailures`) + serde round-trip.
- **FILES:** `crates/rai-browser/src/{lib.rs,tools.rs}`.
- **TESTS:** `#[test] fn browser_action_serde()`.
- **VERIFY:** `cargo check -p rai-browser` — exit 0; clippy clean.
- **DEPS:** T00.

### T35 — [x] a11y-tree serializer (pure, from a JSON fixture)
- **GOAL:** A `serialize_a11y(ax_tree: &serde_json::Value) -> String` that turns a CDP `Accessibility.getFullAXTree` JSON into indented text with `[ref]` markers, collapsing `generic`/`group`/`none`/`presentation` wrappers but keeping semantic roles. Test against a fixture JSON.
- **FILES:** `crates/rai-browser/src/tools.rs`.
- **TESTS:** `#[test] fn serialize_a11y_*` — a fixture with a `navigation` containing two `link`s → text with both `[ref]` markers; a `generic` wrapper is collapsed but its `button` child is kept.
- **VERIFY:** `cargo test -p rai-browser serialize_a11y` — pass; clippy clean.
- **DEPS:** T34.

### T36 — [x] `chromiumoxide` connect + navigate (integration, gated)
- **GOAL:** Behind `feature = "integration"`, a `Browser::launch()` that starts headless Chromium via chromiumoxide and `navigate(url)` returns the page title. Skip if no Chromium available (mark `#[ignore]`).
- **FILES:** `crates/rai-browser/src/cdp.rs`.
- **TESTS:** `#[tokio::test] #[ignore] async fn browser_navigate_title()` (run with `-- --ignored`).
- **VERIFY:** `cargo test -p rai-browser --features integration -- --ignored browser_navigate` — pass when Chromium is present; clippy clean.
- **DEPS:** T34, T35.

---

## Phase 8 — `rai-python` (Graphiti + Hindsight sidecar, Phase 1)

### T37 — [x] `MemoryStore` trait + a `MockMemoryStore` for tests
- **GOAL:** `MemoryStore` async trait compiles; `MockMemoryStore` (in-memory HashMap) implements it; `graphiti_search`/`graphiti_add_episode`/`hindsight_recall`/`hindsight_retain` work against the mock.
- **FILES:** `crates/rai-python/src/{lib.rs,store.rs}`.
- **TESTS:** `#[tokio::test] async fn mock_memory_store_*` — retain then recall returns the retained content.
- **VERIFY:** `cargo test -p rai-python mock_memory_store` — pass; clippy clean.
- **DEPS:** T00.

### T38 — [x] JSON-RPC wire types (request/response/error)
- **GOAL:** `JsonRpcRequest { id, method, params }` + `JsonRpcResponse { id, result }` + `JsonRpcError { code, message }` round-trip through serde; a `parse_request(buf) -> Result<JsonRpcRequest>` handles malformed JSON.
- **FILES:** `crates/rai-python/src/store.rs` (or a new `wire.rs`).
- **TESTS:** `#[test] fn jsonrpc_*` — round-trip; malformed → Err; missing `method` → Err.
- **VERIFY:** `cargo test -p rai-python jsonrpc` — pass; clippy clean.
- **DEPS:** T37.

### T39 — [x] `SidecarMemoryStore` spawn + handshake (no real Python in test)
- **GOAL:** `SidecarMemoryStore::spawn(python_cmd, script_path)` constructs the subprocess config + stdin/stdout pipes; the **config** (args, env) is testable without actually spawning. A `handshake()` protocol reads a `{"ready": true}` line from stdout.
- **FILES:** `crates/rai-python/src/{store.rs,graphiti.rs,hindsight.rs}`.
- **TESTS:** `#[test] fn sidecar_spawn_config_builds()` — assert the `Command` gets the right args; `#[tokio::test] #[ignore] async fn sidecar_handshake()` (real Python, ignored).
- **VERIFY:** `cargo test -p rai-python sidecar_spawn_config` — pass; clippy clean.
- **DEPS:** T37, T38.

### T40 — [x] the Python sidecar script (Graphiti + Hindsight wrapper)
- **GOAL:** A `scripts/sidecar.py` asyncio JSON-RPC server that imports `graphiti_core` + `hindsight` (with graceful fallback if not installed), handles `graphiti_search`/`graphiti_add_episode`/`hindsight_recall`/`hindsight_retain`, reads JSON-RPC from stdin, writes to stdout. A `--self-test` flag runs an offline in-memory path.
- **FILES:** `scripts/sidecar.py`, `scripts/requirements.txt` (graphiti-core, hindsight-all, optional neo4j/kuzu).
- **TESTS:** `python3 scripts/sidecar.py --self-test` exits 0 (offline path).
- **VERIFY:** `python3 scripts/sidecar.py --self-test` — exit 0; the Rust `sidecar_spawn_config` test still passes.
- **DEPS:** T39.
- **NOTES:** This is the self-contained kit's memory substrate. In `--local` mode, Graphiti uses Kuzu (embedded) + Hindsight uses SQLite — both via the sidecar.

---

## Phase 9 — Loop wiring (the real agent loop)

### T41 — [x] `AgentLoop` holds messages + tool registry + resolver + hooks + cancellation
- **GOAL:** `AgentLoop::new(config)` constructs with an empty `Vec<Message>`, an empty `ToolRegistry`, a `PermissionResolver`, a `HookRegistry`, a `CancellationToken`, a `Compactor`. `register_tool(tool)` adds to the registry; `register_hook(hook)` adds to the registry.
- **FILES:** `crates/rai-core/src/{loop_.rs,tool.rs,hook.rs,perm.rs,compact.rs}`.
- **TESTS:** `#[test] fn agent_loop_construction_*` — new + register_tool + register_hook; registries have the right counts.
- **VERIFY:** `cargo test -p rai-core agent_loop_construction` — pass; clippy clean.
- **DEPS:** T11, T12, T15, T16, T07.

### T42 — [x] `ToolRegistry::get(name)` + `schema_for_model()` (collects all tool schemas)
- **GOAL:** `ToolRegistry` holds `HashMap<String, Arc<dyn Tool>>`; `get(name)` returns the tool; `schema_for_model()` returns a `Vec<serde_json::Value>` of all tools' `input_schema()` + names + descriptions (the payload sent to the LLM).
- **FILES:** `crates/rai-core/src/tool.rs`.
- **TESTS:** `#[test] fn tool_registry_*` — register EchoTool, get returns it; schema_for_model has 1 entry with the right name.
- **VERIFY:** `cargo test -p rai-core tool_registry` — pass; clippy clean.
- **DEPS:** T12.

### T43 — [x] the loop calls the LLM and yields `Token` events (mock provider)
- **GOAL:** With a `MockProvider` (returns a fixed `ChatResponse` + a 3-token stream), `AgentLoop::run` yields `Token("a")`, `Token("b")`, `Token("c")`, then `Terminal(EndTurn)`. The loop is wired: `messages → provider.stream() → yield Tokens → yield Terminal`.
- **FILES:** `crates/rai-core/src/loop_.rs`, `crates/rai-llm/src/provider.rs` (add a `MockProvider` behind `#[cfg(test)]` or a `mock` feature).
- **TESTS:** `#[tokio::test] async fn loop_yields_tokens_then_endturn()`.
- **VERIFY:** `cargo test -p rai-core loop_yields_tokens` — pass; clippy clean.
- **DEPS:** T17, T19, T41, T42.

### T44 — [x] the loop executes a tool call (mock provider returns a tool_use)
- **GOAL:** `MockProvider` returns a `tool_use` for `EchoTool`; the loop yields `ToolCalls([...])`, executes the tool via `StreamingToolExecutor`, yields `ToolResult(...)`, then `Terminal(EndTurn)`.
- **FILES:** `crates/rai-core/src/{loop_.rs,tool.rs}`.
- **TESTS:** `#[tokio::test] async fn loop_executes_tool_call()`.
- **VERIFY:** `cargo test -p rai-core loop_executes_tool_call` — pass; clippy clean.
- **DEPS:** T14, T43.

### T45 — [x] permission check before tool execution (Approval mode → Ask)
- **GOAL:** In `Approval` mode, a write tool's execution yields `PermissionRequest(...)` first; the loop awaits the user's `Allow`/`Deny` via a channel; on `Allow` it proceeds, on `Deny` it yields `Terminal(StopHookPrevented("user denied"))`.
- **FILES:** `crates/rai-core/src/{loop_.rs,perm.rs}`.
- **TESTS:** `#[tokio::test] async fn loop_permission_ask_*` — Allow→tool runs; Deny→StopHookPrevented.
- **VERIFY:** `cargo test -p rai-core loop_permission_ask` — pass; clippy clean.
- **DEPS:** T15, T44.

### T46 — [x] cancellation propagates (CancellationToken → Terminal(StreamAborted))
- **GOAL:** While the loop is awaiting the provider, `cancel_token.cancel()` causes the stream to yield `Terminal(StopReason::StreamAborted)` and end.
- **FILES:** `crates/rai-core/src/loop_.rs`.
- **TESTS:** `#[tokio::test] async fn loop_cancellation_aborts()`.
- **VERIFY:** `cargo test -p rai-core loop_cancellation` — pass; clippy clean.
- **DEPS:** T43.

### T47 — [x] the ITVF loop state machine (Implement/Test/Verify/Fix/Done/Escalate/Abort)
- **GOAL:** An `ItvfState` enum + a pure `next_state(current, event) -> ItvfState` transition function covering: Implement→Test (on `Implemented`), Test→Verify (on `Tested`), Verify→Done (on `GoalMatched`), Verify→Fix (on `GoalNotMatched(failure)`), Fix→Implement (on `Fixed`), Fix→Abort (on `IterationCapHit`), any→Escalate (on `CircuitBreaker`), Done/Escalate/Abort are terminal.
- **FILES:** `crates/rai-core/src/escalation.rs` (or a new `itvf.rs`).
- **TESTS:** `#[test] fn itvf_next_state_*` — one assertion per transition + the terminals.
- **VERIFY:** `cargo test -p rai-core itvf_next_state` — pass; clippy clean.
- **DEPS:** T06.

### T48 — [x] the ITVF loop driver (uses the agent loop per iteration, bounded)
- **GOAL:** `ItvfLoop::run(task, max_iter=8)` runs the agent loop per iteration; after each, it runs the `VERIFY` commands (via `Sandbox`), extracts the failure if any, feeds it back as a Fix prompt; stops at `GoalMatched` or `max_iter` or circuit-breaker; returns `ItvfResult { matched, iterations, last_failure, escalate }`.
- **FILES:** `crates/rai-core/src/itvf.rs` (new module; wire into `lib.rs`).
- **TESTS:** `#[tokio::test] async fn itvf_loop_*` — a mock task that "matches" on iteration 1 → `matched=true, iterations=1`; a mock task that never matches → `matched=false, iterations=8, escalate=true`; a task with the same failure 3× → circuit-breaker.
- **VERIFY:** `cargo test -p rai-core itvf_loop` — pass; clippy clean.
- **DEPS:** T46, T47, T31 (Sandbox for VERIFY).
- **NOTES:** This is the loop you (the SLM) are using formalized in Rust. Build it well.

---

## Phase 10 — `rai-tui` (the shell)

### T49 — [x] `rai-tui` lib compiles + `App` struct + `TestBackend` render
- **GOAL:** `App::new(SharedAppState)` constructs; `App::run(terminal)` renders a single frame on `TestBackend` without panic; the frame shows the status bar text "RAI Code · RAI Labs".
- **FILES:** `crates/rai-tui/src/{lib.rs,app.rs}`.
- **TESTS:** `#[test] fn app_renders_status_bar()` — TestBackend 80×24, render, assert buffer contains "RAI Code" and "RAI Labs".
- **VERIFY:** `cargo test -p rai-tui app_renders_status_bar` — pass; clippy clean.
- **DEPS:** T10, T00.

### T50 — [x] `ChatPane` renders streaming tokens from `AppState`
- **GOAL:** Given an `AppState` with `messages` + a `streaming_token`, `ChatPane::render` shows the messages + the in-progress token; on `TestBackend`, the buffer contains the message text.
- **FILES:** `crates/rai-tui/src/panes/chat.rs`.
- **TESTS:** `#[test] fn chat_pane_renders_messages_and_streaming()`.
- **VERIFY:** `cargo test -p rai-tui chat_pane` — pass; clippy clean.
- **DEPS:** T49.

### T51 — [x] `DiffPane` renders a unified diff (syntax-styled)
- **GOAL:** Given a `Diff { path, hunks }`, `DiffPane::render` shows added lines (green `+`) and removed lines (red `-`) with the path header; on TestBackend the buffer contains `+`/`-` markers and the path.
- **FILES:** `crates/rai-tui/src/panes/diff.rs`.
- **TESTS:** `#[test] fn diff_pane_renders_hunks()`.
- **VERIFY:** `cargo test -p rai-tui diff_pane` — pass; clippy clean.
- **DEPS:** T49.

### T52 — [x] `FileTreePane` renders a tree from a `PathBuf` list
- **GOAL:** Given `Vec<PathBuf>`, `FileTreePane::render` shows an indented tree (dirs + files); on TestBackend the buffer contains the file names at the right indentation.
- **FILES:** `crates/rai-tui/src/panes/file_tree.rs`.
- **TESTS:** `#[test] fn file_tree_pane_renders_tree()`.
- **VERIFY:** `cargo test -p rai-tui file_tree_pane` — pass; clippy clean.
- **DEPS:** T49.

### T53 — [x] `CommandPalette` fuzzy-filters commands
- **GOAL:** `CommandPalette::filter(query, commands) -> Vec<Command>` does substring + fuzzy scoring; empty query returns all; "mode" matches "switch mode" + "mode plan" but not "diff".
- **FILES:** `crates/rai-tui/src/palette.rs`.
- **TESTS:** `#[test] fn palette_filter_*`.
- **VERIFY:** `cargo test -p rai-tui palette_filter` — pass; clippy clean.
- **DEPS:** T49.

### T54 — [x] `BrowserPane` renders the a11y-tree text + console pane
- **GOAL:** Given an a11y-tree text + a `Vec<ConsoleEvent>`, `BrowserPane::render` shows the tree in the left sub-pane + console in the right; on TestBackend the buffer contains a `[ref]` marker + an `[ERR]` line.
- **FILES:** `crates/rai-tui/src/panes/browser.rs`.
- **TESTS:** `#[test] fn browser_pane_renders_a11y_and_console()`.
- **VERIFY:** `cargo test -p rai-tui browser_pane` — pass; clippy clean.
- **DEPS:** T49, T35.

### T55 — [x] event loop: crossterm async + `tokio::select!` for keys vs agent stream
- **GOAL:** `App::run` loops on `tokio::select!` between crossterm key events and `AgentStream` events; a `q` key sets a flag that exits the loop; an agent `Token` event appends to `AppState.streaming_token`. Test with a mock stream + simulated key events.
- **FILES:** `crates/rai-tui/src/app.rs`.
- **TESTS:** `#[tokio::test] async fn app_event_loop_handles_keys_and_stream()`.
- **VERIFY:** `cargo test -p rai-tui app_event_loop` — pass; clippy clean.
- **DEPS:** T50, T55.

### T56 — [x] permission-prompt UX (Approval mode → render the request, await key)
- **GOAL:** When `AppState.permission_queue` has a pending `PermissionRequest`, `App::run` renders a focused prompt ("Allow <tool>? [y/n]") and awaits a `y`/`n` key, sending `Allow`/`Deny` back on the channel.
- **FILES:** `crates/rai-tui/src/app.rs` + `panes/chat.rs`.
- **TESTS:** `#[tokio::test] async fn permission_prompt_ux()`.
- **VERIFY:** `cargo test -p rai-tui permission_prompt_ux` — pass; clippy clean.
- **DEPS:** T45, T55.

### T57 — [x] ITVF loop UX: per-task iteration card
- **GOAL:** Given an `ItvfStatus { task_id, iteration, max_iter, last_verify: VerifyResult }`, `render` shows "T<id> ITVF N/M [✓/✗] last: <failure-or-passed>"; on TestBackend the buffer contains the iteration count + the verify symbol.
- **FILES:** `crates/rai-tui/src/panes/chat.rs` (or a new `itvf_card.rs`).
- **TESTS:** `#[test] fn itvf_card_renders()`.
- **VERIFY:** `cargo test -p rai-tui itvf_card` — pass; clippy clean.
- **DEPS:** T48, T50.

---

## Phase 11 — `rai-cli` (the binary, onboarding-first)

### T58 — [x] `rai --version` prints RAI Labs branding
- **GOAL:** `rai --version` prints `rai 0.1.0 · RAI Labs P. Ltd. · www.railabs.in`.
- **FILES:** `crates/rai-cli/src/main.rs`.
- **TESTS:** `#[test] fn version_prints_branding()` (assert Command output contains "RAI Labs").
- **VERIFY:** `cargo test -p rai-cli version_prints` — pass; clippy clean.
- **DEPS:** T00.

### T59 — [x] `rai --local` detects Ollama + recommends a model tier
- **GOAL:** `rai --local` checks `http://localhost:11434/api/tags` (via reqwest, with a short timeout); if reachable, lists models; recommends a tier based on a heuristic (model name contains "32b"→24GB GPU tier, "14b"→16GB, "7b"→CPU/16GB); if unreachable, prints "install Ollama" instructions. No panics on network failure.
- **FILES:** `crates/rai-cli/src/main.rs` (+ a `detect.rs` module).
- **TESTS:** `#[test] fn tier_recommendation_*` (pure: given a model name → tier) + `#[tokio::test] #[ignore] async fn detect_ollama()` (real network, ignored).
- **VERIFY:** `cargo test -p rai-cli tier_recommendation` — pass; clippy clean.
- **DEPS:** T58.

### T60 — [x] onboarding-first flow (recall from Hindsight or 3–5 Qs)
- **GOAL:** `rai` (no args) calls `MemoryStore::hindsight_recall("user profile")`; if a profile is returned, prints "I see you're … — still right? [y/n]"; if not, asks 3 questions (preferred language, stack, diff-before-edit preference), stores via `hindsight_retain`. Use `MockMemoryStore` for the test.
- **FILES:** `crates/rai-cli/src/main.rs` (+ an `onboarding.rs` module).
- **TESTS:** `#[tokio::test] async fn onboarding_existing_profile()` + `onboarding_new_profile()` (with MockMemoryStore).
- **VERIFY:** `cargo test -p rai-cli onboarding` — pass; clippy clean.
- **DEPS:** T37, T58.

### T61 — [x] `rai -p "prompt"` headless one-shot runs the loop and prints tokens
- **GOAL:** `rai -p "hello"` constructs an `AgentLoop` with a `MockProvider`, runs it, prints the streamed tokens to stdout, exits 0. (Real provider wiring is opt-in via config/env.)
- **FILES:** `crates/rai-cli/src/main.rs`.
- **TESTS:** `#[test] fn headless_prints_tokens()` (Command output contains the mock tokens).
- **VERIFY:** `cargo test -p rai-cli headless_prints` — pass; clippy clean.
- **DEPS:** T43, T58.

---

## Phase 12 — `rai-daemon` (post-PoC)

### T62* — `rai-daemon` roster file + start/stop
- **GOAL:** `rai-daemon` writes a roster JSON (`~/.rai/daemon/roster.json`) on start, removes its entry on graceful stop; a `status` subcommand prints the roster.
- **FILES:** `crates/rai-daemon/src/main.rs`.
- **TESTS:** `#[test] fn roster_write_read()` (temp dir).
- **VERIFY:** `cargo test -p rai-daemon roster` — pass; clippy clean.
- **DEPS:** T48.  *[post-PoC]*

### T63* — `rai --bg "prompt"` dispatches to the daemon
- **GOAL:** `rai --bg "prompt"` connects to the daemon's Unix socket, sends the prompt, gets a session id, returns immediately; `rai agents` lists sessions.
- **FILES:** `crates/rai-cli/src/main.rs`, `crates/rai-daemon/src/main.rs`.
- **TESTS:** `#[tokio::test] #[ignore] async fn bg_dispatch()` (real daemon, ignored).
- **VERIFY:** `cargo test -p rai-cli -- --ignored bg_dispatch` — pass.
- **DEPS:** T62.  *[post-PoC]*

---

## Phase 13 — Integration / PoC milestone

### T64 — [x] PoC end-to-end: onboard + index + ingest + loop + diff + undo + sandbox + browser
- **GOAL:** A single integration test (in `tests/poc_e2e.rs`, `#[ignore]`-gated) that: onboards a mock user, indexes a tiny fixture repo with the DCG (T29/T30), ingests one commit as a Graphiti episode via the sidecar (T40), runs the agent loop on a trivial task with a mock provider + the EchoTool, shows a diff, undoes it, runs `echo hi` in the local sandbox, and renders the BrowserPane with a fixture a11y tree. The test exits 0.
- **FILES:** `tests/poc_e2e.rs`, fixture files in `tests/fixtures/`.
- **TESTS:** the test itself.
- **VERIFY:** `cargo test -- --ignored poc_e2e` — pass; clippy clean; `cargo fmt -- --check`.
- **DEPS:** T48, T54, T57, T60, T61, T40, T30, T31.

### T65 — [x] docs: README + AGENTS + LICENSE branded + BUILD/ITVF/TASKS cross-linked
- **GOAL:** `README.md` has RAI Labs branding + the stack + a "Build with an SLM" section linking BUILD.md/TASKS.md; `AGENTS.md` updated; `LICENSE` is Apache-2.0 with `Copyright (c) 2026 RAI Labs P. Ltd.`; all docs cross-link.
- **FILES:** `README.md`, `AGENTS.md`, `LICENSE`, `BUILD.md`, `docs/ITVF-LOOP.md`, `TASKS.md`.
- **TESTS:** `#[test] fn docs_branding_present()` (a tiny test that reads the files and asserts key strings: "RAI Labs", "railabs.in", "reach@railabs.in", "Apache-2.0").
- **VERIFY:** `cargo test docs_branding_present` — pass; clippy clean.
- **DEPS:** T64.

---

## How to use this file

- Pick the lowest-numbered unblocked `[ ]` task whose `DEPS` are all `[x]`.
- Run ITVF on it (Implement → Test → Verify → Fix, ≤8 iterations, circuit-break on 3× same failure).
- When the `GOAL` is matched (all `VERIFY` pass + spec met), commit with the format in `BUILD.md` §8 and mark the task `[x]` in this file (commit that change too).
- If blocked, write `BLOCKED.md`, mark the task `[!]`, and move to the next unblocked task (don't stall the whole build on one blocker).
- `[ ]*` tasks are post-PoC — skip until T64 is done.

**You are RAI Code's first user.** Build it the way it will build apps. Welcome to RAI Labs. 🚀

— *RAI Labs P. Ltd. · [www.railabs.in](https://www.railabs.in) · reach@railabs.in*
