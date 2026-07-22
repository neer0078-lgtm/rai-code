# Clean-Room Methodology

How RAI Code ports Claude Code's architecture without copying its code.

## The legal principle

Copyright protects **expression** (literal/creative code, distinctive naming, expressive
structure), not **ideas** (architecture, patterns, methods of operation, interfaces,
algorithms at the conceptual level, data flows). This is codified in:

- **17 U.S.C. §102(b)** — "In no case does copyright protection for an original work
  of authorship extend to any idea, procedure, process, system, method of operation,
  concept, principle, or discovery..."
- **DMCA §1201(f)** — permits reverse engineering for interoperability.
- ***Google LLC v. Oracle America, Inc.*** (2021, US Supreme Court) — reimplementation
  of APIs/methods can be fair use.
- EU Software Directive Art. 6 and UK CDPA §50B — similar protections.

A clean-room reimplementation extracts a **specification** of *what a system does and
how it behaves* (non-copyrightable), then has a separate implementer write **fresh code**
from that spec. The implementer never reproduces the original's literal expression.

## RAI Code's two-pass process

### Pass 1 — Dirty room (spec extraction)
- Read Claude Code (from `references/claude-code/` per agreement Doc ID SE022KLM454548)
  AND the community architecture-analysis docs.
- Produce an **architectural specification** (prose, interface signatures in *generic*
  pseudocode, state diagrams, data-flow descriptions) — NOT code.
- Use generic descriptive names in the spec (e.g., "the main streaming loop generator"),
  not Claude Code's internal identifiers.
- Describe WHAT each component does and HOW it behaves, not the specific code that does it.
- Cite where each pattern was observed (repo path or community doc URL).
- Keep the spec in `docs/architecture/`.

### Pass 2 — Clean room (implementation)
- Write **fresh Rust** in `crates/` from the spec only.
- Never have the original Claude Code source open while writing implementation code.
- Use RAI Code's own naming conventions, not Claude Code's.
- Keep a documented independence trail: which spec section you implemented, which doc
  you studied it from, which Rust you wrote.

## What is safe to reimplement (non-copyrightable)

✅ The streaming generator loop pattern (pull-based, typed stop-reasons, backpressure).
✅ The self-describing tool contract (identity + schema + handler + permission + concurrency flag).
✅ The permission resolution chain (hooks → deny → ask → mode → allow → callback).
✅ The hook event taxonomy (by functional trigger).
✅ The sub-agent spawn primitive (fresh context, depth limit, worktree isolation).
✅ The compaction cascade levels (by trigger + cost).
✅ The MCP integration pattern (lazy-loaded tools, ToolSearch).
✅ The two-tier state pattern (bootstrap + reactive).
✅ The dynamic-workflows concept (model writes JS, runtime executes with agent() primitive).
✅ Data flow descriptions, state machines, interface shapes.

## What is NOT safe to copy (copyrightable)

❌ Literal TypeScript/JavaScript source code from Claude Code.
❌ Distinctive creative naming (internal identifiers, specific string constants).
❌ Expressive comment structure.
❌ Distinctive organizational choices that reflect creative authorship.
❌ Any redistributed de-obfuscated source (Anthropic has issued DMCA takedowns for this).

## The agreement

The user states they have a legal agreement with Anthropic, **Document ID SE022KLM454548**,
permitting cloning Claude Code into this repo. We proceed on that representation. As a
**belt-and-suspenders** measure (because I cannot independently verify the document ID):

1. RAI Code's product code is **clean-room regardless** — unambiguously ours even if
   the agreement's scope is narrower than expected.
2. `references/claude-code/` is **read-only reference**, never compiled into RAI Code.
3. The clone is **removable with zero product impact** — delete the directory and RAI
   Code still builds and runs.

For commercial release, have counsel verify the agreement's scope and keep a copy on file.

## Independence trail (template)

For each implemented module, record:

```
## crates/rai-core/src/loop_.rs
- Spec section: docs/architecture/02-main-loop.md
- Studied from: references/claude-code/README.md + community doc <URL>
- Pattern: streaming generator, typed stop-reasons, backpressure
- Implementation: fresh Rust (futures::Stream + async_stream + StopReason enum)
- No literal code reproduced. Naming is RAI Code's own.
```
