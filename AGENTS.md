# RAI Code — Agent Guide

> **Built by [RAI Labs P. Ltd.](https://www.railabs.in)** · reach@railabs.in

This file is the project-wide guide for RAI Code's own builder agent (and any
contributing agent, including a small local model developing the repo). It is
read at session start and injected into the system prompt.

> 🤖 **If you are an SLM developing this repo:** read **[BUILD.md](./BUILD.md)** and **[TASKS.md](./TASKS.md)** first, then work task-by-task with the **[ITVF loop](./docs/ITVF-LOOP.md)**.

## What RAI Code is

A Rust + Ratatui agentic software engineering harness. The core is a clean-room
reimplementation of Claude Code's architectural patterns (non-copyrightable) in
fresh Rust, extended with: a temporal code brain (deterministic code graph +
Graphiti), a behavioral user model (Hindsight), an embedded browser (chromiumoxide
+ ratatui-image), an 11-gate evidence-gated production pipeline, execution-layer
security (AgentK pattern), and token-saving as the north star.

## License & legal discipline (READ FIRST)

- RAI Code is **Apache-2.0**.
- The core is **clean-room**: study Claude Code's *architecture* (from
  `references/claude-code/` per agreement Doc ID SE022KLM454548 AND from community
  architecture-analysis docs), extract a specification of *what each component does*,
  then write **fresh Rust** from that spec.
- **NEVER copy literal source code, creative naming, or expressive structure** from
  Claude Code into RAI Code's `crates/`. Patterns/interfaces/data-flows are
  non-copyrightable (17 USC §102(b)); literal expression is not.
- `references/claude-code/` is **read-only reference**. It is never compiled into
  RAI Code. Keep a documented independence trail (which doc you studied, which
  spec you wrote, which Rust you implemented).
- The other reference harnesses (Codex, Aider, OpenHands, Goose, Continue,
  Gemini-CLI, Moatless, SWE-agent) are Apache-2.0/MIT — study their patterns
  freely; still prefer fresh implementations over copy.
- `references/crush/` is NOT vendored (FSL-1.1-MIT non-compete); if added it's
  submodule-only, reference, no derivative code.

## Conventions

- **Rust edition 2021, MSRV 1.75.** `#![forbid(unsafe_code)]` in library crates.
- **Workspace** at the repo root; one crate per concern (see README).
- **Async** = tokio. **Streams** = futures + async_stream. **Cancellation** =
  CancellationToken + tokio::select!.
- **Errors** = anyhow (apps) + thiserror (libs). **Logging** = tracing.
- **Serialization** = serde + serde_json everywhere.
- **Tests** = inline `#[cfg(test)]` modules + `tests/` integration; Vitest-equivalent
  is `cargo test`. TUI tests use ratatui's TestBackend.
- **Style** = `cargo fmt` + `cargo clippy -- -D warnings`.

## Architecture spine (the loop)

```
INTAKE  -> Hindsight recall (who is this user?) + DCG/Graphiti query (what's this project?)
PLAN    -> adaptive (CodePlan for multi-file; Agentless for single-issue bugs)
LOCALIZE-> DCG (call graph, refs) + Graphiti (architectural context)
DECOMPOSE -> sub-agents (depth-limited, worktree isolation, summary-only return)
EXECUTE -> single while(tool_call) loop; CodeAct where possible; checkpoint before risky edits;
           compaction co-designed with caching (checkpoint = new cacheable prefix);
           governance directives NEVER summarized away (Hindsight directives in cached prefix)
VERIFY  -> 11-gate pipeline (Proof-or-Stop; "done" = signed receipt, not claim);
           R2E-Gym hybrid verification (execution + LLM judge)
HITL    -> confidence-triggered review; diff review; checkpoints; git-as-undo;
           modes: Plan, Bypass (YOLO), Approval (HITL), Auto (classifier), Bubble (subagent)
COMMIT  -> conventional commit; emit Graphiti episode (what + why); Hindsight retain (delegated/caught)
CONTEXT -> compaction (preserve governance); Graphiti+Hindsight offload; git-as-undo always
```

## The clean-room port map (Claude Code pattern -> Rust)

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
| Security boundary | AgentK as MCP stdio proxy (native Rust) | agentk-pattern in rai-security |
| Browser | chromiumoxide CDP -> ratatui-image | chromiumoxide, ratatui-image |
| Code intelligence | tree-house + tree-sitter + async-lsp | tree-sitter, async-lsp |
| Memory (Graphiti/Hindsight) | Python sidecar (JSON-RPC) Phase 1; PyO3 Phase 2 | rai-python |

## When you're working in this repo

- Read the relevant `references/<harness>/` to understand a pattern, then write
  fresh Rust in the appropriate `crates/<crate>/src/`. Don't copy.
- Run `cargo check -p <crate>` after edits; `cargo fmt && cargo clippy` before commit.
- Update `docs/architecture/` when you change a core pattern.
- Keep `crates/rai-core` free of TUI/IO concerns — it's the pure loop + types.
- Token-saving is a first-class concern: prompt caching (stable prefix, no dynamic
  values in cached portion), tool-result clearing, 100-line file viewer, sub-agent
  summary-only contracts, model routing (cheap for extraction, frontier for reasoning).
