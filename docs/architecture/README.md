# Architecture Corpus

The clean-room architectural specification for RAI Code — extracted from Claude Code
(non-copyrightable patterns) + community analysis docs. Each file describes *what a
component does and how it behaves*; the Rust implementation in `../../crates/` is
written fresh from these specs.

## Index

- `00-OVERVIEW.md` — system overview: process model, entry points, two-layer (session + per-message), language/runtime.
- `01-CLEAN-ROOM-METHODOLOGY.md` — the legal discipline + two-pass process + independence trail.
- `02-main-loop.md` — the streaming agent loop: phases, typed stop-reasons, backpressure, state, cancellation.
- `03-tool-system.md` — the self-describing Tool contract + StreamingToolExecutor.
- `04-permission-model.md` — the resolution chain + modes (Plan/Bypass/Approval/Auto/Bubble).
- `05-hook-system.md` — ~20 lifecycle events, parallel + most-restrictive-wins.
- `06-subagents.md` — spawn primitive, depth limit, worktree isolation, sidechain transcripts.
- `07-background-daemon.md` — the /bg supervisor: roster, idle eviction, rolling upgrades.
- `08-compaction-cascade.md` — the 6 levels, cache-aware, autocompact summary structure.
- `09-mcp-integration.md` — client-side ToolSearch, deferred tools, in-process SDK MCP.
- `10-context-files.md` — AGENTS.md hierarchical discovery, skills, slash commands, plugins.
- `11-dynamic-workflows.md` — model writes JS, agent() primitive, journaling, schema enforcement.
- `12-state-management.md` — two-tier (bootstrap + reactive), DeepImmutable-equivalent.
- `13-model-layer.md` — multi-provider (genai), prompt caching, custom endpoints, LiteLLM proxy.
- `14-sdk-surface.md` — the public API for embedding/extension.

## Status

The spec extraction (Pass 1) is in progress; files will be filled in from
`references/claude-code/` + community docs. The Rust implementation (Pass 2) in
`../../crates/` is written from these specs, never from Claude Code's source.

## Where the patterns come from

- `references/claude-code/` — the public repo (README + plugins + docs). The CLI source
  ships as obfuscated npm/binary and is NOT in the public repo.
- Community architecture analysis (original works *about* the patterns):
  - github.com/lai3d/claude-code-architecture
  - github.com/alejandrobalderas/claude-code-from-source
  - github.com/Windy3f3f3f3f/how-claude-code-works
  - github.com/openedclaude/claude-reviews-claude
  - harrisonsec.com
  - akshayparkhi.net
- The other reference harnesses in `../../references/` (Codex, Aider, OpenHands, Goose,
  Crush, Gemini-CLI, Moatless, SWE-agent) for cross-cutting patterns.

## RAI Code's extensions (beyond Claude Code's patterns)

- **Temporal code brain**: deterministic code graph (tree-sitter + LSP + SCIP) + Graphiti
  semantic/temporal layer (via `rai-python`).
- **Behavioral user model**: Hindsight (via `rai-python`) + onboarding-before-driving.
- **TUI-embedded browser**: chromiumoxide + ratatui-image; a11y-tree default.
- **11-gate production pipeline**: evidence-gated "done = receipt".
- **Execution-layer security**: AgentK pattern (native Rust).
- **Token-saving north star**: caching + clearing + 100-line viewer + graph offload +
  sub-agent isolation + model routing.
- **Adaptive loop escalation**: Agentless / PlanAndExecute / TreeSearch / DynamicWorkflow /
  Explore / ContextFolding (per the 2025-2026 agentic-loops research).
