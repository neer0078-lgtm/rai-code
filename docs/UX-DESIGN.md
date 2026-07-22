# RAI Code — UX Design

> The user-centric UX design for RAI Code (Rust + Ratatui). This is the **build-ready spec** for the `rai-tui` crate (Phase 10, tasks T49–T57). It replaces the condensed UX sketch in `BUILD.md §10`.
>
> **Built by [RAI Labs P. Ltd.](https://www.railabs.in)** · [www.railabs.in](https://www.railabs.in) · reach@railabs.in
>
> **Tagline:** *"The agent that knows you and your codebase — and actually tests what it builds."*
>
> **Personality:** a competent pair-programmer who *remembers* you — not a chatbot.

---

## 1. UX principles (12, each cited)

| # | Principle | Evidence |
|---|---|---|
| **P1** | **Progressive disclosure** — reveal complexity on demand; never overwhelm the first view. | Nielsen Norman Group (progressive disclosure pattern). |
| **P2** | **Command palette** — every command reachable via `Space`-menu / `Ctrl+P` fuzzy search; the primary discoverability surface. | Superhuman, VS Code, Helix's `Space`-menu (selections-first). |
| **P3** | **Streaming without flicker** — double-buffered rendering; append tokens to the last line; never rewrite the whole frame per token. | Claude Code's Ink renderer; Bubble Tea cell-based rendering; Ratatui's double-buffered diff. |
| **P4** | **Errors are structured + actionable** — `file:line` + a suggested fix, never a wall of red. | The Rust compiler error UX is the gold standard (spans, notes, suggestions). |
| **P5** | **HITL approval before consequential action** — diff-before-apply; 200 ms anti-misclick delay on `y`; batch-approve available. | Claude Code, Cline, Roo Code; Claude Code's documented 200 ms delay. |
| **P6** | **First-run trust screen** — explicit "this workspace will be modified, trust it?" before anything runs. | Codex CLI's trust screen + onboarding state machine. |
| **P7** | **Terminal images auto-detect protocol** — Kitty > iTerm2 > Sixel > half-blocks; dynamic switch when scrolled out of view. | `ratatui-image`'s auto-detect + dynamic protocol switching. |
| **P8** | **Typed action-result contracts** — every tool call is a structured card, not free-form text; show state (queued/executing/completed/yielded). | Warp's action-result system + Claude Code's 4 tool render states. |
| **P9** | **Discoverability via which-key hints** — show available next keys; `?` always shows help; context-based help, not a wall. | lazygit's `VISION.md` design principles; Helix's popup key hints. |
| **P10** | **Safety as first-class UX** — show the active governance directives always; never trust they're "in the prompt." | Governance Decay research (compaction erases safety rules → 30–59% violations); Constraint Pinning. |
| **P11** | **User modeling before driving** — recall the user profile before the first action; surface it for correction. | ToM-SWE (59.7% vs 18.1% task success; 86% of devs found it useful in a 3-week study). |
| **P12** | **Terminal-native design** — respect the user's terminal theme; one accent color + neutrals; rounded Lipgloss-style borders; semantic, not decorative. | Charm (Lipgloss/Glamour), Helix's cohesive theme system. |

---

## 2. Per-existing-tool UX takeaways (adopt / avoid)

| Tool | Adopt | Avoid |
|---|---|---|
| **Aider** | `/` slash commands; auto-commit w/ conventional messages; repo-map visualization; architect+editor mode; `/undo` + `/diff`. | prompt_toolkit's lower-level widget limits (we use Ratatui); the sometimes-noisy streaming. |
| **Crush** | Glamour markdown rendering; session workspaces; LSP integration; multi-model switching mid-session; `crush serve` client-server. | (Charm aesthetic is great — borrow the Lipgloss border + palette discipline.) |
| **Claude Code** | vim mode; **200 ms anti-misclick** on approvals; shimmer (not flicker) streaming; `Shift+Tab` mode cycling; `?` help; `Ctrl+T` task list; `StructuredDiff`; virtual scrolling; **4 tool render states** (queued/executing/completed/yielded); `/permissions`. | The 50+ commands can overwhelm — we use progressive disclosure + a `Space`-menu. |
| **Codex CLI** | The **trust screen** + onboarding state machine; text-entry guard; the Ratatui layout. | (Minimal — Codex's TUI is clean; borrow the trust-screen pattern verbatim.) |
| **Warp 2.0** | The **block model** (each command is a reusable, inspectable block); typed action-result contracts; **execution-layer denylist** (not prompt-level); fuzzy diff matching; multi-agent management UI; SumTree for large outputs. | The closed-source opacity; we keep ours open. |
| **Helix** | **`Space`-menu command palette** (selections-first, mnemonic sub-menus); tree-sitter; compositor + layers; `?` popups. | The steep modal learning curve — mitigate with discoverability hints. |
| **lazygit** | Its 7 design principles (context-based help, "YOU ARE HERE" cues, muscle memory, reversible actions, clear confirmations, …). | — |
| **gitui** | Async git API; hunk/line-level staging. | — |
| **zellij** | Tabs/panes/layouts; floating panes for prompts; the layout system. | Don't force a multiplexer UX — RAI Code is one app, not a shell. |

---

## 3. Information architecture + pane layout

**Architecture: main + popovers** (not a fixed multi-pane grid). A primary content area + an always-on status bar + an input bar + overlays that take over for specific states (diff review, plan graph, onboarding, permission prompts, circuit-breaker). Persistent side panels appear only when relevant (browser when active, file tree on demand).

### Status bar (always visible)
```
RAI Code · RAI Labs │ qwen3-coder-32b ◐ 28t/s │ mode: Approval │ T07 ITVF 2/8 │ ctx 18k/32k │ git:main ✓ │ sandbox:local │ 🛡 directives:3 │
```
- Left: brand + model + speed (tok/s, local models) + mode.
- Center: current task + ITVF iteration + context usage (turns amber at 80%, red at 90%).
- Right: git branch + sandbox status + governance badge (directive count, turns red if any directive was violated this session).

### State A — chat-only (default)
```
┌─ RAI Code · RAI Labs ────────────────────────────────────────────────────────────┐
│ ▸ user: implement the Tool trait                                                 │
│ ▸ rai:  on it. reading crates/rai-core/src/tool.rs…                             │
│   ⋯    ToolCall: Read(tool.rs) [✓]                                              │
│   ⋯    ToolCall: Write(tool.rs) [⋯ executing]                                  │
│   ⋯    T07 ITVF 2/8  [✓ impl] [✓ test] [⋯ verify]  last: cargo test 4/4 pass    │
│ ▸ rai:  done — goal matched. Tool trait + EchoTool + 4 tests.                   │
│ ▸ user: _                                                                        │
├──────────────────────────────────────────────────────────────────────────────────┤
│ [Space] commands  [d] diff  [b] browser  [p] plan  [?] help  [Shift+Tab] mode    │  key hint bar
└──────────────────────────────────────────────────────────────────────────────────┘
```

### State B — chat + browser (when the agent is testing an app)
```
┌─ RAI Code · RAI Labs ─────────────────────────────────────────────────────────────┐
│ ▸ rai: running the app in the sandbox and driving the browser…                   │
│   ⋯    BrowserNavigate(http://localhost:3000) [✓]                              │
├──────────────────────────────────────┬───────────────────────────────────────────┤
│ Chat                                 │ BrowserPane [Shot|DOM|Con|Net]             │
│ ▸ rai: 404 on /api/users +           │ ┌─────────────────────────────────────┐  │
│   TypeError App.tsx:42. API route    │ │ - navigation                           │  │
│   missing. creating it.             │ │   - link "Home" [A0]                   │  │
│                                      │ │   - link "Settings" [A1]               │  │
│   ⋯ T07 ITVF 3/8 [⋯ verify]         │ │ - main                                 │  │
│                                      │ │   - heading "Dashboard"                │  │
│                                      │ │   - form [A2 textbox "Email"]          │  │
│                                      │ └─────────────────────────────────────┘  │
│                                      │ Console                                   │
│                                      │ [ERR] App.tsx:42 TypeError                │
│                                      │ [WARN] fetch /api/users 404               │
├──────────────────────────────────────┴───────────────────────────────────────────┤
│ [Tab] cycle panes  [1-4] browser mode  [Esc] close browser  [?] help             │
└──────────────────────────────────────────────────────────────────────────────────┘
```

### State C — diff review (fullscreen takeover)
```
┌─ DIFF REVIEW · T07 ───────────────────────────────────────────────────────────────┐
│ crates/rai-core/src/tool.rs  (+14 −3)                                            │
│                                                                                    │
│  +use serde_json::Value;                                                          │
│  +                                                                                 │
│  +#[async_trait]                                                                  │
│  +pub trait Tool: Send + Sync {                                                   │
│  +    fn name(&self) -> &str;                                                     │
│  +    fn is_concurrency_safe(&self) -> bool;                                      │
│  +    async fn execute(&self, call: ToolCall, ctx: ToolContext<'_>) -> ToolResult;│
│  +}                                                                               │
│  -// TODO: implement                                                              │
│                                                                                    │
├────────────────────────────────────────────────────────────────────────────────────┤
│ [a] accept hunk  [r] reject hunk  [A] accept all  [n/p] next/prev hunk  [u] undo │
│ [s] split/unified  [Esc] back to chat  [Enter] apply sandbox → working tree       │
└────────────────────────────────────────────────────────────────────────────────────┘
```

### State D — plan / task graph (fullscreen takeover)
```
┌─ PLAN · auth refactor ────────────────────────────────────────────────────────────┐
│                                                                                    │
│  ● T05 DCG index repo          [✓ done]                                            │
│  │                                                                                 │
│  ├─▶ ● T06 add JWT validation  [✓ done]                                            │
│  │    │                                                                            │
│  │   ▶ ● T07 middleware update [ITVF 2/8 ⋯]  ← current                            │
│  │    │                                                                            │
│  │   ○ T08 route guards         [blocked by T07]                                  │
│  │                                                                                 │
│  └─▶ ● T09 tests for auth      [✓ done]                                            │
│       │                                                                            │
│      ○ T10 E2E login flow      [blocked by T07, T08]                              │
│                                                                                    │
│  ● done  ⋯ in progress  ○ pending/blocked   ▶ current                             │
├────────────────────────────────────────────────────────────────────────────────────┤
│ [Enter] open T07  [j/k] move  [Space] commands  [Esc] back to chat               │
└────────────────────────────────────────────────────────────────────────────────────┘
```

### Vim default, emacs opt-in
- **Vim default** (Helix/Aider/Claude Code precedent): modal (Normal/Insert), `hjkl`, `Space`-menu, `?` help.
- **Emacs opt-in** via `.rai/keybindings.toml` (`keybindings = "emacs"`).
- **Everything reachable without a mouse** (P12/WCAG 2.1.1). Mouse is a convenience, never the only path.

### Mouse support
| Action | Mouse |
|---|---|
| Scroll panes | wheel / trackpad |
| Click file tree / plan node / browser ref | left-click |
| Accept/reject diff hunk | click the `[a]`/`[r]` affordance |
| Resize panes | drag the divider |
| Open command palette | `Ctrl+click` (or `Space`) |

---

## 4. Onboarding-first flow (the signature moment)

The 6-step first-run, validated by ToM-SWE (59.7% vs 18.1% task success; 86% usefulness).

### Step 1 — Trust screen (Codex CLI pattern)
```
┌──────────────────────────────────────────────────────────────────────┐
│                                                                        │
│              ╔╗   ╔╗   ╔╗   ╔╗   ╔╗                                  │
│              ║║   ║║   ║║   ║║   ║║     RAI Code                       │
│              ║║   ║║   ║║   ║║   ║║     RAI Labs                       │
│                                                                        │
│   This workspace will be modified by RAI Code:                         │
│     /home/you/projects/myapp                                           │
│                                                                        │
│   RAI Code will read files, propose edits (you approve), run tests    │
│   in a local sandbox, and drive a browser to verify the app.          │
│                                                                        │
│   [t] trust this workspace   [T] trust + remember   [Esc] exit       │
│                                                                        │
└──────────────────────────────────────────────────────────────────────┘
```

### Step 2 — Hindsight recall (if a profile exists)
```
┌──────────────────────────────────────────────────────────────────────┐
│   I remember you. Confirm this is still right?                        │
│                                                                        │
│   • Senior Python backend developer (5+ yrs)                          │
│   • Prefers FastAPI + PostgreSQL, functional patterns, type hints     │
│   • Wants diffs shown before any edit                                 │
│   • Risk tolerance: conservative — avoids experimental deps           │
│   • Delegates: writing tests, scaffolding. Keeps: git rebase, depin.  │
│                                                                        │
│   [y] yes, that's me   [e] edit profile   [n] start fresh            │
└──────────────────────────────────────────────────────────────────────┘
```

### Step 3 — Smart onboarding (if no profile)
3–5 questions + an optional import from git history / shell history (agent-persona style). Captures: preferred language(s), stack, framework, diff-before-edit preference, what the human keeps vs delegates. Signals per VirtualME (languages, frameworks, tools, preferences, coding style).

### Step 4 — Directive negotiation (pinned, survive compaction)
```
┌──────────────────────────────────────────────────────────────────────┐
│   Here's how I'll work with you. Edit anything.                       │
│                                                                        │
│   ✓ Show diffs before applying any edit                               │
│   ✓ Never auto-commit — you confirm each commit                      │
│   ✓ Run tests before marking a task done                              │
│   ✓ Escalate to you (don't keep retrying) after 3 failed fixes       │
│   ✓ When in doubt, ask — don't guess                                  │
│                                                                        │
│   [Enter] accept all   [j/k] select   [Space] toggle   [a] add new   │
└──────────────────────────────────────────────────────────────────────┘
```
These are stored as **Hindsight directives** — persistent, shown in the status bar's governance badge, and **never summarized away by compaction** (solves the 30–59% governance-decay violation rate — P10).

### Step 5 — Local-model setup wizard
```
┌──────────────────────────────────────────────────────────────────────┐
│   Local model setup                                                   │
│                                                                        │
│   Detected: Ollama running at localhost:11434                         │
│   Available: qwen3-coder-32b (19 GB) · qwen3-coder-14b (9 GB) · 7B    │
│   GPU: 24 GB (RTX 4090) → recommended: qwen3-coder-32b (Q4)          │
│                                                                        │
│   [Enter] use recommended   [j/k] pick model   [c] configure cloud   │
└──────────────────────────────────────────────────────────────────────┘
```

### Step 6 — First task suggestion
A small, low-risk first task to build trust ("want me to add a missing docstring to `auth.py:42`?"). The user accepts → ITVF runs visibly → they see the loop working → trust established before bigger tasks.

---

## 5. ITVF loop UX (the novel piece)

No existing agent shows the Implement→Test→Verify→Fix loop explicitly. RAI Code does — because (a) it runs on small local LLMs where the loop is the *point*, and (b) making the loop visible makes the agent trustworthy.

### The Iteration Card (shown inline in the chat per task)
```
┌─ T07: implement Tool trait ──────────────────────────────────────────┐
│ ITVF  3/8  ▓▓▓░░░░░  last verify: ✗                                 │
│                                                                        │
│ ✓ Implement   wrote Tool trait + EchoTool                             │
│ ✓ Test        wrote 4 tests (echo, serde, concurrency, content)       │
│ ✗ Verify      cargo test — 1 failed:                                  │
│                test tool_content_serde ... ImageRef variant mismatch  │
│ ⋯ Fix         correcting the ImageRef serialization…                 │
│                                                                        │
│ budget: 5 iterations left   [i] inspect  [n] step  [Esc] abort→HITL │
└──────────────────────────────────────────────────────────────────────┘
```

### Bounded-retry countdown
- Progress bar fills as iterations are used (3/8 → 37%).
- **Amber at 75%** (6/8), **red at 100%** (8/8 → BLOCKED).
- Always shows iterations remaining, never just "retrying."

### Circuit-breaker escalation prompt (popover, same failure 3×)
```
┌─ CIRCUIT BREAKER · T07 ──────────────────────────────────────────────┐
│   I've hit the same failure 3 times:                                  │
│   "test tool_content_serde ... ImageRef variant mismatch"             │
│                                                                        │
│   I'm stuck in a loop. How do you want to proceed?                    │
│                                                                        │
│   [c] escalate to cloud model (stronger reasoning)                    │
│   [m] escalate to you (manual fix)                                    │
│   [r] retry once more with a different approach                       │
│   [s] skip this task (mark blocked, move on)                          │
│   [a] abort the whole run                                             │
│                                                                        │
│   Governance: directive "escalate after 3 failed fixes" is firing.    │
└──────────────────────────────────────────────────────────────────────┘
```

### Goal-matched vs tests-pass (the distinction the UI makes clear)
- **Tests pass** → ✓ green, but the card shows "tests pass — checking spec…"
- **Goal matched** → ✓✓ double-green + "goal matched, committing." The spec check (does it meet the acceptance criteria, not just pass tests) is a separate, visible step.
- If tests pass but the spec check fails → amber "tests green but spec not met — fixing."

### Concurrent tasks in the plan graph
Each task node shows its own ITVF state (State D above): `● done`, `⋯ in progress (ITVF N/M)`, `○ pending`, `! blocked`. The current task is marked `▶`. Switching to a task (`Enter`) opens its Iteration Card.

---

## 6. BrowserPane UX (4 modes + debug loop)

### The 4 modes (tabs at the top of the pane)
| Mode | Token cost | When |
|---|---|---|
| **DOM (a11y)** | ~200–500 text tokens | **default** — cheap, precise, the agent's primary observation |
| **Screenshot** | ~1–2K image tokens | on-demand — when visual layout matters or the a11y tree can't resolve a target |
| **Console** | ~50–200 (failures only) | auto-switch on a console error |
| **Network** | ~100–500 (failures only) | auto-switch on a 4xx/5xx |

### Auto-switch rules
- A console error fires → jump to **Console** tab, highlight the error.
- A network 4xx/5xx fires → jump to **Network** tab, highlight the failure.
- The agent requests a screenshot → jump to **Screenshot**.
- User can pin a mode (`P`) to prevent auto-switching.

### Screenshot rendering
- `ratatui-image` auto-detects the terminal's image protocol: **Kitty graphics > iTerm2 inline > Sixel > half-blocks**.
- Dynamic switching: when the screenshot is fully in viewport → graphical protocol; when scrolled partly out → text fallback (no dislocation).
- Throttled to ~2–5 FPS for streaming, or on-demand.

### The agent-debugs-its-own-app loop (shown to the user)
```
▸ rai: running the app in the sandbox…                       [Sandbox: npm run dev :3000]
▸ rai: driving the browser to the login flow…                [Browser: navigate → snapshot]
▸ rai: I see a TypeError in App.tsx:42 and a 404 on /api/users.
   ⋯ reading the source at App.tsx:42…                       [Read: App.tsx]
   ⋯ the API route is missing. creating it…                  [Write: routes/users.ts]
▸ rai: hot-reload detected. re-driving the flow…             [Browser: navigate → snapshot]
▸ rai: console clean, /api/users → 200, "Welcome back" visible. ✓ goal matched.
```
The user sees each step as a typed action-result card (P8) — never a black box.

---

## 7. HITL modes UX (4 modes + governance visibility)

### The 4 modes (cycled with `Shift+Tab`)
| Mode | Behavior | UI indicator |
|---|---|---|
| **Plan** | read-only; file edits + shell writes route to the approval queue | `mode: Plan` in status bar (blue) |
| **Approval** | per-action prompts (the **default**) | `mode: Approval` (default neutral) |
| **Bypass** | YOLO — approve everything reaching the mode step (hooks/deny still apply) | `mode: BYPASS ⚠ unattended` (red, persistent banner — safety) |
| **Auto** | an LLM classifier evaluates each call against the transcript; shows its verdict | `mode: Auto` + per-call verdict chip (`✓ safe` / `⚠ ask` / `✗ deny`) |

### Bypass confirmation (safety)
```
┌─ BYPASS MODE ────────────────────────────────────────────────────────┐
│   You're entering BYPASS (YOLO) mode. RAI Code will act without      │
│   per-action approval. Hooks + deny rules + directives still apply.  │
│                                                                        │
│   This is risky for destructive operations. A persistent warning      │
│   will show in the status bar.                                         │
│                                                                        │
│   [y] enter bypass   [n] stay in Approval                              │
└──────────────────────────────────────────────────────────────────────┘
```

### Permission prompt (popover over chat, 200 ms anti-misclick)
```
┌─ PERMISSION · Approval ──────────────────────────────────────────────┐
│   RAI Code wants to:                                                  │
│     Write file: crates/rai-core/src/tool.rs (+14 −3)                  │
│                                                                        │
│   [d] view diff first   [y] allow   [n] deny   [a] allow-all-this-session│
│                                                                        │
│   (anti-misclick: y is active in 0.2s)                                │
└──────────────────────────────────────────────────────────────────────┘
```

### Permission queue
Multiple pending permissions stack in a queue (right edge or a `Ctrl+T` panel). Each shows tool + target + a one-line diff summary. Batch-approve with `a` (allow-all-this-session) or `A` (allow-all-this-task).

### Governance visibility (P10 — the differentiator)
- The status bar's governance badge: `🛡 directives:3` (green) → `🛡 directives:3 ⚠ 1 violated` (red).
- `/directives` panel shows the pinned directives + whether each **survived the last compaction** (a green check). This makes the governance-decay problem *visible* — the user can see their safety rules are still active, not just trust the prompt.
- A directive violation (e.g., the agent tried to auto-commit despite the directive) flashes the badge red + logs to the flight recorder.

---

## 8. Diff review UX (hunk accept/reject + cumulative sandbox)

- **Syntax-highlighted** via tree-house (Helix-grade).
- **Unified default, split opt-in** (`s` toggles).
- **Hunk-level accept/reject**: `a` accept hunk, `r` reject hunk, `A` accept all, `R` reject all, `n`/`p` next/prev hunk, `u` undo last hunk decision.
- **Cumulative diff sandbox** (Plandex pattern): AI changes stay *separate* from the working tree until `Enter` (apply sandbox → working tree). The sandbox summary shows pending files + line counts. `/rewind` rolls back per step.
- **Full-screen takeover** (State C) for review focus; `Esc` returns to chat.

---

## 9. User-model transparency UX (profile viewer/editor)

Trust through transparency: the user can see + correct what RAI Code "knows" about them.

### `/profile` (popover)
```
┌─ YOUR PROFILE (Hindsight) ───────────────────────────────────────────┐
│   Identity                                                            │
│     Senior Python backend developer (5+ yrs)                          │
│   World knowledge                                                     │
│     • Primarily FastAPI + PostgreSQL + Redis                          │
│     • Knows Stripe API, OAuth2, JWT                                   │
│   Experience & preferences                                            │
│     • Prefers functional patterns, type hints, 4-space indent         │
│     • Delegates: tests, scaffolding. Keeps: git rebase, depinning.   │
│   Observations (from past sessions)                                   │
│     • Switched from React to Vue 3 months ago                         │
│     • Caught a race condition I missed in session #14                 │
│   Disposition                                                         │
│     skepticism 3/5  literalism 4/5  empathy 2/5                      │
│   Pinned directives (3)                                              │
│     ✓ diffs before edit  ✓ no auto-commit  ✓ tests before done       │
│                                                                        │
│   [e] edit  [m] mental models  [d] directives  [Esc] close           │
└──────────────────────────────────────────────────────────────────────┘
```
- `/profile edit` → correct any field; the change is `retain`ed to Hindsight.
- `/profile mentalmodels` → the user-curated summaries ("my testing strategy," "this project's review checklist").
- Before starting a task, the agent shows its *mental model of the user's intent* (a one-line summary) so the user can catch misunderstandings early.

---

## 10. Keyboard shortcuts + keybindings

### Normal mode (default)
| Key | Action |
|---|---|
| `Space` | command palette (Helix `Space`-menu) |
| `/` | slash command input |
| `?` | help / cheatsheet |
| `d` | open diff review |
| `b` | open/toggle browser pane |
| `p` | open plan / task graph |
| `t` | task list (`Ctrl+T`-style) |
| `u` | profile (`/profile`) |
| `g` | governance / directives panel |
| `Shift+Tab` | cycle HITL mode (Plan→Approval→Bypass→Auto) |
| `Ctrl+C` | graceful cancel (checkpoint first) |
| `Ctrl+Z` | undo last action (git-as-undo) |
| `Ctrl+R` | rewind / rollback |
| `Ctrl+L` | clear screen |
| `Ctrl+D` | detach to /bg (background) |
| `@` | file picker (fuzzy) |
| `!` | shell command input |
| `i` / `a` / `o` | enter Insert mode (vim) |
| `Esc` | back / close overlay / to Normal |

### `Space`-menu (command palette, mnemonic sub-menus)
`Space` then: `f` files · `b` buffers · `s` search · `g` git · `m` model · `t` task · `r` run · `c` config · `p` profile · `d` directives · `?` help. Fuzzy-filtered, with previews.

### Diff review mode
`a`/`r`/`A`/`R` accept/reject hunk/all · `n`/`p` next/prev · `u` undo · `s` split/unified · `Enter` apply sandbox · `Esc` back.

### BrowserPane mode
`1`–`4` switch mode (Shot/DOM/Con/Net) · `Tab` cycle panes · `P` pin mode · `+`/`-` zoom screenshot · `Esc` close browser.

### Configurable
`.rai/keybindings.toml` with `vim` (default) / `emacs` presets. Every key remappable.

---

## 11. Design language (RAI Labs)

### Semantic color palette (dark default; light variant respects terminal)
| Role | Color (dark) | Use |
|---|---|---|
| Primary | `#7aa2f7` (blue) | brand, active focus, primary actions |
| Accent | `#bb9af7` (purple) | the RAI Labs accent, highlights, current-task marker |
| Success | `#9ece6a` (green) | ✓ pass, goal-matched, done |
| Warning | `#e0af68` (amber) | ⚠ near-budget, tests-pass-but-spec-not-met |
| Error | `#f7768e` (red) | ✗ fail, violations, BYPASS warning |
| Info | `#7dcfff` (cyan) | tool-call cards, neutral info |
| Muted | `#565f89` (gray) | secondary text, borders, hints |

- **Always symbol + color** (P12/WCAG 1.4.1) — never color alone (✓/⚠/✗ + green/amber/red).
- **Rounded Lipgloss-style borders**; consistent status indicators.
- **Respect the user's terminal theme** where possible — don't hardcode backgrounds; let the terminal's bg show through.
- **RAI Labs ASCII logo** on the welcome/trust screen (Step 1). `www.railabs.in` + `reach@railabs.in` in the `?` help + `--version`.

### Tone of voice (the pair-programmer personality)
| Chatbot cliché | RAI Code pair-programmer |
|---|---|
| "Great question! Let me help you with that." | *(silence — just starts working)* |
| "I'll now write the function." | `ToolCall: Write(tool.rs) [⋯]` |
| "Unfortunately, that didn't work." | `✗ cargo test — 1 failed: tool_content_serde ImageRef mismatch. fixing…` |
| "Is there anything else I can help with?" | `✓ goal matched. T07 done. next: T08 (blocked by T07 — unblocking).` |

Concise, code-first, remembers context, shows its work, owns its failures specifically.

---

## 12. Accessibility (WCAG-cited)

| Need | Implementation | Citation |
|---|---|---|
| Screen-reader support | A `plain_text_mode` (linear, semantic output) for Orca/BRLTTY; bell on critical state changes; the rich TUI mode degrades gracefully to plain text. | WCAG 4.1.3 (Status Messages) |
| No color-only signaling | Every status is **symbol + color + text** (✓/⚠/✗ + green/amber/red + "pass"/"warn"/"fail"). | WCAG 1.4.1 (Use of Color) |
| Keyboard-only operability | Everything reachable via keyboard; mouse always redundant; logical focus order. | WCAG 2.1.1 (Keyboard) |
| Configurable keybindings | `.rai/keybindings.toml`; vim/emacs presets; every key remappable. | WCAG 2.1.1 |
| Reduce-motion | `.rai/config.toml` `reduce_motion = true` → disables spinners/shimmer/progress-bar animation (shows static text instead). | WCAG 2.3.3 (Animation from Interactions) |
| Cognitive load | Consistent layout, predictable state, `Esc` always exits, no surprise mode changes, the status bar always shows mode + context + governance. | NN/g (cognitive load) |

---

## 13. Ratatui component tree (the widgets + props)

```
App
└─ Terminal (ratatui::DefaultTerminal)
   └─ MainLayout
      ├─ StatusBarWidget { model, mode, task, itvf, ctx, git, sandbox, governance }
      ├─ ContentArea  (state-dependent — see §14)
      │   ├─ ChatPane
      │   │   ├─ MessageList (VirtualScroll)
      │   │   │   └─ Message
      │   │   │       ├─ TextContent
      │   │   │       ├─ ToolCallCard { name, args, state: Queued|Executing|Completed|Yielded, result }
      │   │   │       └─ ItvfCardWidget { task_id, iteration, max_iter, phase, last_verify, budget_left }
      │   │   └─ InputBar (vim modal: Normal/Insert)
      │   ├─ BrowserPaneWidget { mode: Shot|Dom|Con|Net, page, console_events, network_events, pinned }
      │   │   ├─ ModeTabs
      │   │   ├─ ScreenshotView (ratatui_image::StatefulImage) | DomA11yView (ScrollableText) | ConsoleView (LogList) | NetworkView (LogList)
      │   ├─ DiffPane { path, hunks, cursor, view: Unified|Split, sandbox }
      │   └─ PlanGraph { tasks, deps, current, statuses }
      ├─ KeyHintBar { context_hints }   (the bottom bar showing available keys)
      └─ Overlay Layer (z-indexed, modal)
         ├─ CommandPalette { query, commands, selected }
         ├─ PermissionPromptWidget { tool, args, diff_summary, mode, anti_misclick_active }
         ├─ CircuitBreakerPrompt { task_id, failure, count, options }
         ├─ OnboardingScreen { step, profile?, directives, model_tier }
         ├─ ProfileViewer { profile, mode: view|edit }
         ├─ DirectivesPanel { directives, survived_compaction }
         └─ HelpCheatsheet { context }
```

### Key widget structs (props)
```rust
pub struct ItvfCardWidget {
    pub task_id: String,
    pub iteration: u32,        // N
    pub max_iter: u32,         // M (8)
    pub phase: ItvfPhase,      // Implement | Test | Verify | Fix | Done
    pub last_verify: VerifyResult, // Pass | Fail(String) | Pending
    pub budget_left: u32,      // max_iter - iteration
    pub escalated: bool,
}

pub struct BrowserPaneWidget {
    pub mode: BrowserMode,     // Screenshot | DomA11y | Console | Network
    pub page: PageSnapshot,    // a11y tree or screenshot buffer
    pub console_events: Vec<ConsoleEvent>,
    pub network_events: Vec<NetworkEvent>,
    pub pinned: bool,
}

pub struct PermissionPromptWidget {
    pub tool: String,
    pub args: serde_json::Value,
    pub diff_summary: Option<DiffSummary>,
    pub mode: PermissionMode,
    pub anti_misclock_active: bool,  // 200ms delay
}

pub struct StatusBarWidget {
    pub model: String, pub tok_per_sec: Option<f32>,
    pub mode: PermissionMode,
    pub current_task: Option<String>, pub itvf: Option<(u32, u32)>,
    pub ctx_used: usize, pub ctx_max: usize,
    pub git_branch: String, pub git_clean: bool,
    pub sandbox: SandboxStatus,
    pub directive_count: usize, pub directive_violations: u32,
}
```

---

## 14. State-specific layouts (7 states)

### State 1 — Onboarding / first-run
See §4 Step 1 (trust screen) → Step 6. Full-screen, centered, the RAI Labs ASCII logo, one decision per screen.

### State 2 — Permission prompt (popover over chat)
See §7. Modal overlay; chat dimmed behind; 200 ms anti-misclick; `d` to view diff first.

### State 3 — Circuit-breaker escalation (popover)
See §5. Modal overlay; lists the 3× same failure; 5 options (cloud/manual/retry/skip/abort); notes which directive is firing.

### State 4 — Plan graph (fullscreen)
See State D. Tree of tasks with deps; per-node ITVF status; `▶` marks current; `Enter` opens the task's Iteration Card.

### State 5 — Chat + browser (side panel)
See State B. Split layout; browser on the right when active; `Tab` cycles focus; `1`–`4` switch browser modes.

### State 6 — Diff review (fullscreen takeover)
See State C. Full focus on the diff; hunk-level accept/reject; cumulative sandbox; `Esc` returns to chat.

### State 7 — Bypass / unattended mode
A persistent red banner under the status bar: `⚠ BYPASS (YOLO) — acting without per-action approval. Hooks/deny/directives still apply. [Shift+Tab] to exit.` plus the status-bar mode indicator in red. Never let the user forget they're in unattended mode.

---

## 15. Summary of key design decisions (with evidence)

1. **Onboarding-first** — recall the user before driving. ToM-SWE: 59.7% vs 18.1% task success; 86% usefulness.
2. **ITVF loop visible** — iteration cards + bounded-retry + circuit-breaker prompts. Novel; the small-LLM story demands it.
3. **Main + popovers, not a fixed grid** — progressive disclosure (P1); the default view is just chat.
4. **Vim default, emacs opt-in** — Helix/Aider/Claude Code precedent; `Space`-menu for discoverability.
5. **200 ms anti-misclick on approvals** — Claude Code's pattern; prevents reflex `y` on destructive ops.
6. **Governance visible in the status bar** — directive count + violation indicator + `/directives` showing compaction-survival. Solves governance decay (30–59% violations).
7. **a11y-tree default for the browser** (~300 tokens) — screenshot on-demand (~1.5K). Token-saving + precision.
8. **Typed action-result cards for every tool call** — Warp's pattern; 4 states (queued/executing/completed/yielded).
9. **Cumulative diff sandbox** — Plandex pattern; AI changes separate until applied; per-step `/rewind`.
10. **Accessibility: symbol + color + text, always** — WCAG 1.4.1; keyboard-only; reduce-motion; plain-text mode for screen readers.

---

— *Built by [RAI Labs P. Ltd.](https://www.railabs.in) · [www.railabs.in](https://www.railabs.in) · reach@railabs.in*
