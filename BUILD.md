# BUILD.md — RAI Code Self-Contained Build Kit

> **Load this file first.** This is the single self-contained document a small local model (SLM) consumes to develop RAI Code, task-by-task, using the **ITVF loop** (Implement → Test → Verify → Fix, repeat until goal matched, then next task).
>
> **Built by RAI Labs P. Ltd.** · [www.railabs.in](https://www.railabs.in) · reach@railabs.in

---

## 0. What you are building

**RAI Code** is a modern **Rust + Ratatui** agentic software engineering harness for long-horizon, production-ready, secure, comprehensively-tested software. Tagline: *"The agent that knows you and your codebase — and actually tests what it builds."*

RAI Code combines:
1. **Claude Code's proven harness architecture**, clean-room reimplemented in Rust (patterns only, never copied source — see §7).
2. A **temporal code brain**: deterministic code graph (tree-sitter + LSP + SCIP) + **Graphiti** (bi-temporal semantic layer).
3. A **behavioral user model**: **Hindsight** (onboarding-first — knows the user before driving).
4. An **embedded browser** (chromiumoxide + ratatui-image) that tests + debugs the apps it builds.
5. An **11-gate evidence-gated production pipeline** ("done = a receipt, not a claim").
6. **Execution-layer security** (AgentK pattern, native Rust).
7. **Token-saving as the north star** (caching, tool-result clearing, graph offload, sub-agent isolation, model routing).
8. **The ITVF loop** as the primary per-task loop — tuned so a **small local LLM** can do real agentic coding (this is how *you*, the SLM, will develop RAI Code itself).

**You (the SLM) are the developer.** You work task-by-task from `TASKS.md`. Each task is ITVF-sized: small, self-contained, with a machine-checkable goal. You run ITVF on each until the goal is met, then move to the next.

---

## 1. How you work — the ITVF protocol

**ITVF = Implement → Test → Verify → Fix, repeat until GOAL MATCHED, then next task.**

For each task in `TASKS.md`:

1. **Read the task.** Note its `GOAL` (machine-checkable acceptance criteria), `FILES` (what to touch), `TESTS` (what to write), `VERIFY` (the exact commands to run), `DEPS` (prior tasks that must be done).
2. **Implement** — write the code in the listed files, following the conventions (§6).
3. **Test** — write the tests listed (inline `#[cfg(test)]` modules or `tests/` integration tests).
4. **Verify** — run the exact `VERIFY` commands. **Read the actual output** (don't assume).
5. **Goal-matched?** — YES iff all `VERIFY` commands pass AND the code meets the task spec. If yes → commit (§8) → next task.
6. **Fix** — if not matched: extract the **specific** failure from the test/compiler output (not "it failed" — the exact error: file, line, message). Apply a targeted fix. Go back to step 3.

**Rules:**
- **Max 8 iterations per task.** If you hit 8, STOP and write a `BLOCKED.md` note (which task, what failed, what you tried) — do not keep burning cycles.
- **Circuit-breaker:** if the **same** failure recurs 3 times, STOP — you're in a loop. Write `BLOCKED.md` and escalate.
- **Never claim done.** "Goal matched" is decided by the `VERIFY` commands passing, not by you. If `cargo test` fails, the task is not done — no exceptions.
- **Small, atomic changes.** One task = one coherent change. Don't do three tasks' worth of work in one ITVF run.
- **Read before write.** Always `read_file` a file before editing it (per repo conventions).
- **Verify with the real toolchain.** `cargo check -p <crate>`, `cargo test -p <crate>`, `cargo clippy -p <crate> -- -D warnings`, `cargo fmt -- --check`. The output is ground truth.

`docs/ITVF-LOOP.md` has the distilled protocol + a worked example. Read it.

---

## 2. RAI Labs branding (use everywhere)

- **Company:** RAI Labs P. Ltd.
- **Website:** [www.railabs.in](https://www.railabs.in)
- **Contact:** reach@railabs.in
- **Product:** RAI Code
- **Tagline:** *"The agent that knows you and your codebase — and actually tests what it builds."*
- **License:** Apache-2.0 (copyright holder: RAI Labs P. Ltd.)

Use this in `README.md`, `AGENTS.md`, `LICENSE`, crate `description` fields, `--version` output, the TUI header, and the docs.

---

## 3. The locked stack

| Layer | Choice | Crate | License |
|---|---|---|---|
| Language | Rust (edition 2021, MSRV 1.75) | — | — |
| TUI | Ratatui + crossterm + **ratatui-image** | ratatui, crossterm, ratatui-image | MIT |
| Async runtime | tokio + tokio-util (CancellationToken) | tokio | MIT |
| Agent loop streams | futures + async-stream | futures, async-stream | MIT |
| Python bridge (Graphiti + Hindsight) | **Phase 1: Python sidecar (JSON-RPC over stdin/stdout).** Phase 2: PyO3 in-process (opt-in). | pyo3 (opt) | Apache-2.0/MIT |
| MCP | **rmcp** (official Rust SDK) — client + server, lazy-loaded via ToolSearch | rmcp | MIT |
| LLM | **genai** (26+ providers, native protocols, Anthropic CacheControl, custom endpoints, Ollama/vLLM local) + optional LiteLLM proxy | genai | MIT |
| Browser | **chromiumoxide** (pure-Rust CDP) + ratatui-image | chromiumoxide | MIT |
| Code intelligence | tree-sitter + **tree-house** (Helix highlighter) + **async-lsp** | tree-sitter, async-lsp | MIT/MPL-2.0 |
| Security | **AgentK** pattern (native Rust — no port) | rai-security | MIT |
| Dynamic workflows | **rustyscript** (deno_core/V8), `agent()` as a custom Deno op | rustyscript | MIT |
| Git (worktree isolation) | git2 | git2 | MIT/GPL |
| Sandbox | E2B (REST) + Daytona (feature-gated) + local subprocess + AgentK approval | — | Apache-2.0/MIT |
| Serialization/errors/logging | serde + serde_json + anyhow + thiserror + tracing | — | MIT |

**Why these (decisive reasons):**
- **Rust + Ratatui** — `codex-rs/tui` is the blueprint; `ratatui-image` is the BrowserPane renderer (Kitty/iTerm2/Sixel/half-blocks).
- **Graphiti (kept, not helix-db)** — bi-temporal edges + episode provenance + LLM entity resolution + MIT. helix-db is a great Rust graph-vector *storage* but not a temporal-KG *framework* (no bi-temporal, no episodes, AGPL). See `docs/HELIX-DB-EVAL.md`.
- **AgentK native Rust** — the execution-layer security kernel needs no port. Big win.
- **Python sidecar first, PyO3 later** — IPC is ~0.1ms (negligible vs Graphiti's seconds-per-episode); crash-isolated; no GIL/deadlock. The `rai-python` `MemoryStore` trait has both impls behind a feature flag.

---

## 4. The workspace (already scaffolded)

```
rai-code/
├── Cargo.toml              # workspace (default=local, full=python+managed)
├── crates/
│   ├── rai-core/           # loop_, tool, perm, hook, subagent, compact, escalation, state, workflow
│   ├── rai-tui/            # app + panes (chat/diff/plan/file_tree/browser) + palette
│   ├── rai-llm/            # genai client + provider trait
│   ├── rai-mcp/            # rmcp client + server
│   ├── rai-sandbox/        # Sandbox trait + E2B/Daytona/Local
│   ├── rai-browser/        # cdp + pane + tools (chromiumoxide + ratatui-image)
│   ├── rai-security/       # kernel + flight (AgentK pattern)
│   ├── rai-codegen/        # dcg + lsp + highlight (tree-sitter + async-lsp + tree-house)
│   ├── rai-python/         # MemoryStore trait + graphiti + hindsight (sidecar Phase 1, PyO3 Phase 2)
│   ├── rai-daemon/         # /bg supervisor binary
│   └── rai-cli/            # the `rai` binary
├── references/             # 10 harnesses (claude-code + 9 OSS) — read-only, never compiled in
├── docs/                   # architecture corpus + ITVF + UX + this kit
├── AGENTS.md               # project-wide agent guide
├── BUILD.md                # THIS FILE (load first)
├── TASKS.md                # the ITVF task list (your work queue)
└── LICENSE                 # Apache-2.0, © RAI Labs P. Ltd.
```

**Features:** `default = ["local"]` (no Python, no cloud sandbox) · `full = ["python", "managed"]` · `python` (PyO3 in-process) · `managed` (E2B/Daytona).

**Current state of the crates:** `rai-core` has *real type stubs* (the `StopReason` enum with 11 variants, `Tool` trait with `is_concurrency_safe`, `Permission` + `PermissionMode` Plan/Bypass/Approval/Auto/Bubble, `Hook` trait + 20 `HookEvent`s, `SubAgent`/`Isolation` Worktree/InProcess/Fork, `Compactor` + 9-section `AutoCompactSummary`, `EscalationMode` + `classify_task()`, two-tier `AppState`). The other 10 crates have lib stubs + Cargo.tomls wired into the workspace. Your job: fill them in, task by task.

---

## 5. The architecture spine (the loop you're building)

```
INTAKE  → Hindsight recall (who is this user?) + DCG/Graphiti query (what's this project?)
PLAN    → adaptive (CodePlan for multi-file; Agentless for single-issue)
LOCALIZE→ DCG (call graph, refs) + Graphiti (architectural context)
DECOMPOSE → sub-agents (depth-limited, worktree isolation, summary-only return)
EXECUTE → for EACH sub-task: the ITVF loop (Implement→Test→Verify→Fix until goal matched)
VERIFY  → 11-gate pipeline (Proof-or-Stop; "done" = signed receipt)
HITL    → confidence-triggered review; modes Plan/Bypass/Approval/Auto/Bubble
COMMIT  → conventional commit; emit Graphiti episode; Hindsight retain
CONTEXT → compaction (preserve governance); Graphiti+Hindsight offload; git-as-undo
```

The **ITVF loop is the inner EXECUTE+VERIFY loop per task.** The 11-gate pipeline is the *outer* verification (before a feature is "done-done"). You (the SLM) use ITVF to build RAI Code; RAI Code uses ITVF to build apps. Same loop, two layers.

### The clean-room port map (Claude Code pattern → Rust)

| Claude Code pattern | Rust approach | Key crates |
|---|---|---|
| AsyncGenerator `query()` | `futures::Stream<AgentEvent>` + `async_stream::stream!` | async-stream, futures, tokio |
| Typed Terminal stop-reasons | `enum StopReason` | serde |
| Backpressure | Stream is pull-based | futures |
| `yield*` composability | `StreamExt::flatten`/`then` | futures |
| Cancellation | `CancellationToken` + `tokio::select!` | tokio-util |
| Self-describing Tool | `#[async_trait] trait Tool` | async-trait, serde_json |
| `isConcurrencySafe` | trait method | — |
| StreamingToolExecutor | `tokio::JoinSet` (parallel-safe / sequential-unsafe) | tokio |
| Permission chain | `enum Permission` + most-restrictive-wins; modes Plan/Bypass/Approval/Auto/Bubble | — |
| Hooks (~20, parallel, most-restrictive-wins) | `Vec<Box<dyn Hook>>` via JoinSet | tokio, async-trait |
| Subagents | `tokio::spawn` + fresh state + `git2` worktree isolation | tokio, git2 |
| /bg daemon | separate `rai-daemon` binary (tokio + roster), IPC via Unix socket/JSON-RPC | tokio, serde_json |
| Compaction cascade | state machine + genai `CacheControl` for cache_edits | genai |
| Dynamic workflows (model writes JS) | `rustyscript` (deno_core/V8), `agent()` as a custom Deno op | rustyscript |
| Two-tier state | OnceLock bootstrap + `Arc<RwLock<AppState>>` rebuilt per Ratatui frame | once_cell, parking_lot |
| MCP client/server | `rmcp` TokioChildProcess + ServerHandler + `#[tool]` | rmcp |
| Security boundary | AgentK as MCP stdio proxy (native Rust) | rai-security |
| Browser | chromiumoxide CDP → ratatui-image | chromiumoxide, ratatui-image |
| Code intelligence | tree-house + tree-sitter + async-lsp | tree-sitter, async-lsp |
| Memory (Graphiti/Hindsight) | Python sidecar (JSON-RPC) Phase 1; PyO3 Phase 2 | rai-python |

---

## 6. Conventions (follow strictly)

- **Rust edition 2021, MSRV 1.75.** `#![forbid(unsafe_code)]` in library crates.
- **Async** = tokio. **Streams** = futures + async_stream. **Cancellation** = CancellationToken + tokio::select!.
- **Errors** = anyhow (apps/bins) + thiserror (libs). **Logging** = tracing.
- **Serialization** = serde + serde_json everywhere.
- **Tests** = inline `#[cfg(test)]` modules + `tests/` integration. TUI tests use ratatui's `TestBackend`.
- **Style** = `cargo fmt` + `cargo clippy -- -D warnings` (zero warnings allowed).
- **Read before write** — always `read_file` before `edit_file`.
- **Naming** = RAI Code's own (e.g., `AgentLoop`, `StopReason`, `Tool`, `Permission`), NOT Claude Code's internal identifiers.
- **No unsafe** in lib crates without a SAFETY comment and a review note.
- **Workspace** at repo root; one crate per concern.

### The verify commands (ground truth)

```bash
cargo check -p <crate>                    # compiles?
cargo test -p <crate>                     # tests pass?
cargo clippy -p <crate> -- -D warnings    # zero warnings?
cargo fmt -- --check                      # formatted?
```

A task is "goal matched" iff **all four** pass (plus any task-specific checks in `VERIFY`).

---

## 7. Clean-room rules (critical — read this)

RAI Code's core is a **clean-room reimplementation** of Claude Code's *architectural patterns* (non-copyrightable: methods of operation, interfaces, data flows — 17 USC §102(b); reinforced by *Google v. Oracle*, 2021). The repo includes `references/claude-code/` (per agreement Doc ID SE022KLM454548) AND the other reference harnesses (Codex, Aider, OpenHands, Goose, Continue, Gemini-CLI, Moatless, SWE-agent, AgentK).

**As the SLM developer, you MUST:**
- ✅ Study the *architecture* (what each component does, how it behaves) from `references/` + `docs/architecture/`.
- ✅ Write **fresh Rust** in `crates/` from the spec, using RAI Code's own naming.
- ❌ **NEVER copy literal source code, creative naming, or expressive structure** from Claude Code (or any reference) into `crates/`. Patterns/interfaces/data-flows are non-copyrightable; literal expression is not.
- ✅ For the other Apache/MIT harnesses, studying + adapting patterns is fine; still prefer fresh implementations. `references/agentk/` is MIT Rust — even there, prefer fresh; adapt with attribution if you reuse.
- ✅ Keep a documented independence trail: which reference you studied, which spec section you wrote, which Rust you implemented. Note it in the commit message.
- `references/claude-code/` is **read-only, never compiled into RAI Code.** Removable with zero product impact.
- `references/crush/` is NOT vendored (FSL-1.1-MIT non-compete). Don't add it.

Full methodology: `docs/architecture/CLEAN-ROOM-METHODOLOGY.md`.

> **Belt-and-suspenders:** RAI Code's product code is clean-room regardless of the agreement's scope. For commercial release, RAI Labs should have counsel verify the agreement.

---

## 8. Commit + independence trail

After each goal-matched task:

```bash
cargo fmt
cargo test -p <crate> && cargo clippy -p <crate> -- -D warnings
git add -A
git commit -m "T<id>: <title>

Goal: <one-line acceptance criteria met>
Implemented: <what changed>
Studied-from: <which reference/doc>
No literal code reproduced from references; fresh Rust, RAI Code naming.

Built by RAI Labs P. Ltd. — www.railabs.in"
```

If you hit the iteration cap or circuit-breaker, write `BLOCKED.md` instead:

```markdown
## BLOCKED — T<id>
- Goal: ...
- Iterations used: N/8
- Last failure (exact): <file:line: msg>
- Tried: <list of fixes attempted>
- Likely needs: <human review / stronger model / clarification>
```

---

## 9. Local-model setup (your environment)

You (the SLM) run locally. RAI Code must work the same way. **Hardware tiers:**

| Tier | Hardware | Recommended model | Serving | Notes |
|---|---|---|---|---|
| CPU-only | 16–32 GB RAM | Qwen3-Coder 7B (Q4) or CodeGemma 7B | Ollama | Slow (~5–10 tok/s); keep tasks tiny; aggressive compaction. |
| 16 GB GPU | RTX 4070 / similar | Qwen3-Coder 14B (Q4, ~9 GB VRAM) | Ollama / llama.cpp | ~20–30 tok/s; the practical floor for agentic work. |
| 24 GB GPU | RTX 4090 / 5070 Ti | **Qwen3-Coder 32B (Q4, ~19 GB VRAM)** | Ollama / vLLM | ~30 tok/s; the recommended tier; 62% SWE-bench. |
| 48+ GB GPU | A100 / 2×4090 | Qwen3-Coder 32B (Q8) or DeepSeek-Coder-V4 | vLLM | fastest local; opt-in. |

**Local serving:** `ollama serve` (default port 11434) → genai custom endpoint `http://localhost:11434`. The kit must run **zero external network** in local mode: local model + Kuzu (embedded Graphiti backend) + SQLite (Hindsight) + local subprocess sandbox + local headless Chromium + local verification tools (cargo/pytest/tsc/eslint/semgrep/stryker all run locally).

**Self-contained kit behavior:** when RAI Code launches in `--local` mode, it detects Ollama, recommends a model+tier, and requires **no API keys**. Cloud (Anthropic/OpenAI) + E2B + Browserbase are all opt-in `--full` features.

**Scaffolding that makes you (the SLM) viable** (apply the same to RAI Code's agent):
- Smaller context → aggressive context curation (C4: last 5 tool calls + summarize older; 100-line file viewer; graph offload).
- Weaker reasoning → more structure: simpler tool schemas, schema-enforced outputs (serde + JSON Schema with strict validation), fewer tools in context at once (progressive disclosure).
- **ITVF compensates for one-shot weakness** — small models benefit MOST from fix-until-pass loops.
- Model routing: local-default; opt-in cloud escalation when a circuit-breaker fires.

---

## 10. UX design (full spec in `docs/UX-DESIGN.md`)

RAI Code's UX is specified in detail in **[`docs/UX-DESIGN.md`](./docs/UX-DESIGN.md)** — read it before the `rai-tui` tasks (T49–T57). It's the build-ready spec: 12 cited principles, per-tool adopt/avoid, 4 pane-layout states, the 6-step onboarding-first flow, the **ITVF loop UX** (iteration card, bounded-retry, circuit-breaker prompt — novel), the BrowserPane (4 modes + auto-switch + debug loop), HITL modes + governance visibility, diff review, user-model transparency, keyboard shortcuts, the RAI Labs design language, accessibility (WCAG-cited), and the full Ratatui component tree.

**In one paragraph:** RAI Code feels like a competent pair-programmer who *remembers you*, not a chatbot. Principles: progressive disclosure, keyboard-driven everything, discoverability (`Space`-menu + `?` help), never color-only signaling (symbols + text too — WCAG 1.4.1), streaming without flicker, errors are actionable (structured `file:line` + suggestion). **Onboarding-first** (the signature moment, validated by ToM-SWE 59.7% vs 18.1%): launch → trust screen → Hindsight recall ("I see you're a senior Python dev…") or 3–5 Qs + git/shell-history import → negotiate directives (pinned, survive compaction) → local-model setup wizard → first task. **Layout:** main + popovers (not a fixed grid) — status bar (model/mode/ITVF/ctx/git/sandbox/governance) + Chat (with ITVF iteration cards) + BrowserPane (when active) + Diff (fullscreen takeover) + Plan graph + command palette. **HITL modes:** Plan/Bypass(YOLO)/Approval/Auto — cycled with `Shift+Tab`. **ITVF loop UX (novel):** per-task iteration card N/8, last Verify result, bounded-retry countdown, circuit-breaker escalation prompt when the same failure recurs 3×. **Diff review:** syntax-highlighted (tree-house), hunk accept/reject, cumulative sandbox (Plandex). **Command palette:** `Space` (Helix pattern). **Keyboard:** vim default, emacs opt-in, `.rai/keybindings.toml`. **Accessibility:** screen-reader plain-text mode, no color-only, keyboard-only, reduce-motion. **Design language (RAI Labs):** semantic palette (primary `#7aa2f7`, accent `#bb9af7`, success `#9ece6a`, warning `#e0af68`, error `#f7768e`), respect the terminal theme, rounded Lipgloss borders.

---

## 11. The PoC milestone (your north star for Phase 0–9)

By end of the PoC, RAI Code can do this end-to-end in `--local` mode:
1. **Onboard** a new user (3–5 Qs + git/shell history import → Hindsight profile + directives).
2. **Index** a real repo with the DCG (tree-sitter + LSP) — fast, exact, structural.
3. **Ingest commits** as Graphiti episodes (via the Python sidecar) — slow, semantic, async.
4. **Run a single-loop agent** with grep + DCG + Graphiti tools (via MCP).
5. **Review diffs** (syntax-highlighted, accept/reject hunks) + **git-undo**.
6. **Run tests in the local sandbox** (subprocess + AgentK approval).
7. **Show a basic BrowserPane** (a11y-tree + console/network panes; screenshot on-demand).
8. Use the **ITVF loop** as the per-task execution loop with bounded retry + escalation.

**Defer to post-PoC:** sub-agents, the full 11-gate pipeline, the security kernel hardening, dynamic workflows, the /bg daemon. (Their tasks are in `TASKS.md` but marked `phase: post-poc`.)

---

## 12. Files you have to work with

| File | What it is |
|---|---|
| `BUILD.md` | THIS — load first. |
| `TASKS.md` | Your work queue. Each task = one ITVF unit. |
| `docs/ITVF-LOOP.md` | The ITVF protocol distilled + a worked example. |
| `docs/architecture/CLEAN-ROOM-METHODOLOGY.md` | The legal discipline + two-pass process. |
| `docs/architecture/README.md` | The architecture corpus index (spec files 00–14 to fill). |
| `AGENTS.md` | Project-wide agent guide (the conventions). |
| `README.md` | The repo README (stack + layout). |
| `references/` | 10 harnesses (claude-code + 9 OSS). Read-only. Never compiled in. |
| `crates/rai-core/src/*.rs` | Real type stubs for the core patterns. Start here. |
| `crates/*/src/lib.rs` + `Cargo.toml` | The other 10 crates, stubbed. |

---

## 13. Start here

1. Read `docs/ITVF-LOOP.md` (the protocol + worked example).
2. Read `TASKS.md` (the work queue).
3. Run `cargo check` to confirm the workspace compiles as-scaffolded (it should — the stubs are wired).
4. Pick the first unblocked task (`T00`), run ITVF on it until goal matched, commit, move to `T01`.

**You are RAI Code's first user.** Build it the way it will build apps: task by task, verify before claiming done, escalate when stuck. Welcome to RAI Labs. 🚀

— *RAI Labs P. Ltd. · [www.railabs.in](https://www.railabs.in) · reach@railabs.in*
