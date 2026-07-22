# RAI Code

> **The agent that knows you and your codebase — and actually tests what it builds.**
>
> **Built by [RAI Labs P. Ltd.](https://www.railabs.in)** · [www.railabs.in](https://www.railabs.in) · reach@railabs.in

A modern **Rust + Ratatui** agentic software engineering harness for long-horizon, production-ready, secure, comprehensively-tested, verified software. RAI Code combines Claude Code's proven harness architecture (clean-room reimplemented in Rust) with a **temporal code brain** (deterministic code graph + Graphiti), a **behavioral user model** (Hindsight), an **embedded browser** that tests and debugs the apps it builds, an **11-gate evidence-gated production pipeline**, **execution-layer security**, and **token-saving as the north star**.

> 🤖 **Building RAI Code with a small local model?** Read **[BUILD.md](./BUILD.md)** first, then **[TASKS.md](./TASKS.md)**. The ITVF loop (Implement → Test → Verify → Fix until goal matched) is how an SLM develops this repo task-by-task, fully self-contained, offline.

## Status

🚧 **Phase 1 (research + brainstorming) complete.** Phase 2 (technical spec) and Phase 3 (PoC scaffold) in progress. This repo currently contains the workspace scaffold + reference corpus; the crate bodies are stubs awaiting implementation.

## Why RAI Code

Existing coding agents are either (a) capable but blind to *who* they're working for and *why* a project is the way it is, or (b) memory-rich but architecturally weak for long-horizon production work. RAI Code's bet: combine a **temporal code knowledge graph** (deterministic code graph + Graphiti) so the agent understands a project end-to-end and over time, with a **behavioral user model** (Hindsight) so it knows who it's talking to and what the human will handle vs. delegate — all inside a Rust TUI with an embedded browser, production-grade HITL, sub-agents, sandboxing, and security at the execution layer.

## The stack

| Layer | Choice |
|---|---|
| Language | **Rust** (edition 2021, MSRV 1.75) |
| TUI | **Ratatui** + crossterm + **ratatui-image** (Kitty/iTerm2/Sixel/half-blocks for the BrowserPane) |
| Agent loop | Clean-room reimplementation of Claude Code's AsyncGenerator `query()` pattern — `futures::Stream<AgentEvent>` + typed `StopReason` enum + `CancellationToken` |
| LLM | **genai** (26+ providers, native protocols, Anthropic `CacheControl` prompt caching, custom endpoints, local) + optional LiteLLM proxy |
| Temporal code KG | **Graphiti** (Python, via `rai-python` sidecar — Phase 1; PyO3 in-process — Phase 2) |
| User model | **Hindsight** (Python, via `rai-python` sidecar — Phase 1; PyO3 — Phase 2) |
| Deterministic code graph | tree-sitter (native Rust) + async-lsp + SCIP + tree-house highlighting |
| Browser | **chromiumoxide** (pure-Rust CDP) → ratatui-image; a11y-tree default (~300 tokens), screenshot on-demand |
| Sandbox | **E2B** (Firecracker microVM, runs app + headless Chromium together) / Daytona / local subprocess+approval |
| MCP | **rmcp** (official Rust SDK) — client + server, lazy-loaded via ToolSearch |
| Security | **AgentK** pattern (native Rust — no port needed): typed syscalls, taint, capability receipts, flight recorder, MCP proxy mediation |
| Dynamic workflows | **rustyscript** (deno_core/V8) — sandboxed JS, `agent()` as a custom Deno op |
| Persistence | Neo4j (Graphiti, managed) / Kuzu (embedded, local) + PostgreSQL+pgvector (Hindsight, managed) / SQLite (local) |
| License | **Apache-2.0** |

## The workspace

```
rai-code/
├── Cargo.toml              # workspace
├── crates/
│   ├── rai-core/           # agent loop, tools, permissions, hooks, sub-agents, compaction, escalation
│   ├── rai-tui/            # Ratatui app: chat/diff/plan/file-tree/BrowserPane/command palette
│   ├── rai-llm/            # multi-provider LLM (genai)
│   ├── rai-mcp/            # MCP client + server (rmcp)
│   ├── rai-sandbox/        # E2B / Daytona / local subprocess
│   ├── rai-browser/        # chromiumoxide + ratatui-image (BrowserPane)
│   ├── rai-security/       # AgentK execution-layer security kernel
│   ├── rai-codegen/        # tree-sitter DCG + async-lsp + tree-house
│   ├── rai-python/         # Graphiti + Hindsight bridge (sidecar — Phase 1; PyO3 — Phase 2)
│   ├── rai-daemon/         # /bg per-user supervisor binary
│   └── rai-cli/            # the rai binary
├── references/             # reference corpus (see references/README.md)
└── docs/                   # architecture corpus + dossier
```

**Features:** `default = ["local"]` (no Python, no cloud sandbox) · `full = ["python", "managed"]` · `python` (PyO3 in-process) · `managed` (E2B/Daytona).

## The clean-room methodology

RAI Code's core is a **clean-room reimplementation** of Claude Code's *architectural patterns* (non-copyrightable: methods of operation, interfaces, data flows — 17 USC §102(b); reinforced by *Google v. Oracle*, 2021) in fresh Rust. We study Claude Code (cloned in `references/claude-code/` per agreement Doc ID SE022KLM454548) and the community architecture-analysis docs, extract a specification of *what each component does and how it behaves*, then write fresh Rust from that spec — never reproducing literal code, creative naming, or expressive structure. See `docs/architecture/CLEAN-ROOM-METHODOLOGY.md`.

> **Legal note:** For commercial release, have counsel verify the agreement's scope. RAI Code's product code is clean-room regardless, so the product is unambiguously ours even if the agreement's scope is narrower than expected. The `references/claude-code/` clone is read-only (never compiled into RAI Code) and removable with zero product impact.

## Build

```bash
# Local mode (default — no Python, no cloud)
cargo build --release

# Full mode (Python sidecar + cloud sandbox)
cargo build --release --features full

# Run
cargo run --release --bin rai
```

## Documentation

- `BUILD.md` — **the self-contained build kit for a small local model (SLM) developer. Load this first.**
- `TASKS.md` — the ITVF-sized work queue (65 tasks across 14 phases).
- `docs/ITVF-LOOP.md` — the ITVF protocol (the SLM's working method) + a worked example.
- `docs/RAI-Code-Brainstorming-Dossier.md` — the full v3 research dossier (18 sections)
- `docs/architecture/` — the clean-room architectural spec + methodology
- `references/README.md` — the reference corpus guide + license summary

## License

Apache-2.0. See `LICENSE`.

## References (the building blocks)

Graphiti (Apache-2.0) · Hindsight (MIT) · Playwright (Apache-2.0) · chromiumoxide (MIT) · Ratatui (MIT) · ratatui-image (MIT) · rmcp (MIT) · genai (MIT) · tree-sitter (MIT) · async-lsp (MIT) · AgentK (MIT) · E2B (Apache-2.0) · rustyscript (MIT).
