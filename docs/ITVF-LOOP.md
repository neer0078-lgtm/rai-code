# The ITVF Loop Protocol

> **ITVF** = **I**mplement → **T**est → **V**erify → **F**ix, repeat **until GOAL MATCHED**, then next task.
>
> This is how you (the SLM) develop RAI Code. It is also the loop RAI Code uses per task when it builds apps. Same loop, two layers.

---

## 1. Why ITVF (and why it suits a small local model)

Small models can't one-shot complex changes. But they *can* make a small change, run the tests, read the failure, and fix it — repeatedly, with a clear exit criterion. ITVF turns a weak one-shot model into a capable iterative one by:

- **Bounding each iteration** to a small, verifiable change.
- **Grounding verification in real toolchain output** (cargo/clippy/tests), not the model's self-assessment — this avoids the Reflexion "memory confabulation" failure (RRR=0.64 when feedback is binary/verbal).
- **Providing a clear exit criterion** ("goal matched" = all `VERIFY` commands pass + spec met) — so you never claim done prematurely.
- **Circuit-breaking** when you're stuck, instead of burning tokens forever.

Prior art: Agentless (localize→repair→validate), SWE-agent (ReAct + ACI), VeriHarness (structured feedback from deterministic validators), R2E-Gym (hybrid execution+LLM verification, 51% vs 42–43%), ORPS (execution-grounded self-critique, +26.9%), Proof-or-Stop (evidence-gated lifecycle, false-done 31/1800 → 2/1800). ITVF is the structured, bounded, evidence-gated version of "fix until it works."

---

## 2. The loop (state machine)

```
                 ┌──────────────────────────────────────────────┐
                 │                                              ▼
  START ──▶ IMPLEMENT ──▶ TEST ──▶ VERIFY ──▶ GOAL_MATCHED? ──YES──▶ COMMIT ──▶ NEXT TASK
                ▲                              │                       │
                │                              NO                      │
                │                              ▼                       │
                │                            FIX ──▶ (extract specific failure)
                │                              │
                │                          iteration < 8?
                │                              │ YES → loop back to IMPLEMENT
                │                              │ NO  → BLOCKED (write BLOCKED.md, escalate)
                │
                └── same failure 3×? → CIRCUIT BREAKER → BLOCKED (escalate)
```

**States:**
- **IMPLEMENT** — write the code for the task in the listed files, following conventions. Small, atomic.
- **TEST** — write the tests (inline `#[cfg(test)]` or `tests/`).
- **VERIFY** — run the exact `VERIFY` commands. **Read the actual output.** Don't summarize it as "passed" if you didn't read it.
- **GOAL_MATCHED?** — YES iff every `VERIFY` command exits 0 AND the code meets the task's acceptance spec. If YES → COMMIT → NEXT TASK.
- **FIX** — extract the **specific** failure (file, line, message — from the real tool output, not a guess), apply a **targeted** fix, loop back to TEST. Not a wholesale rewrite.
- **BLOCKED** — iteration cap (8) hit OR circuit-breaker (same failure 3×) OR budget exhausted. Write `BLOCKED.md`, escalate. Do NOT keep iterating.

---

## 3. The rules (strict)

1. **Max 8 iterations per task.** Count them. At 8, stop.
2. **Circuit-breaker:** the *same* failure message 3 times → stop. You're in a loop; a different fix is needed (or human input).
3. **Never claim done.** "Goal matched" is the `VERIFY` commands passing, not your assessment. If `cargo test` shows a failing assertion, the task is not done — period.
4. **Read the real output.** Don't pattern-match "looks like it passed." Read the test names, the assertion messages, the compiler errors. Extract the exact failure.
5. **Small, atomic changes per iteration.** One fix at a time. Don't bundle three fixes — if one is wrong, you can't tell which.
6. **Read before write.** `read_file` before `edit_file`. Always.
7. **Verify with the real toolchain.** The four commands in §6 of `BUILD.md` are ground truth. Plus any task-specific `VERIFY`.
8. **No silent assumptions.** If a task is ambiguous, write `BLOCKED.md` with the question — don't guess and burn iterations.

---

## 4. A worked example

**Task T03 (from `TASKS.md`):** *Implement `StopReason` exhaustiveness test.*

```
GOAL:    A unit test that compiles AND asserts that `StopReason` has exactly the 11
         variants listed in crates/rai-core/src/loop_.rs, failing if any are added/removed.
FILES:   crates/rai-core/src/loop_.rs (add the test inline)
TESTS:   #[test] fn stop_reason_has_eleven_variants()
VERIFY:  cargo test -p rai-core stop_reason  &&  cargo clippy -p rai-core -- -D warnings
DEPS:    T01, T02
```

**Iteration 1:**
- IMPLEMENT: read `loop_.rs`, see the 11-variant enum. Add:
  ```rust
  #[cfg(test)]
  mod tests {
      use super::StopReason;
      #[test]
      fn stop_reason_has_eleven_variants() {
          let all = [
              StopReason::EndTurn, StopReason::UserAbort, StopReason::BudgetExhausted,
              StopReason::StopHookPrevented("x".into()), StopReason::MaxTurns(1),
              StopReason::Unrecoverable("x".into()), StopReason::BlockingLimit("x".into()),
              StopReason::StreamAborted, StopReason::ModelError("x".into()),
              StopReason::PromptTooLong, StopReason::ImageError("x".into()),
          ];
          assert_eq!(all.len(), 11);
      }
  }
  ```
- TEST: the test above.
- VERIFY: run `cargo test -p rai-core stop_reason`.
  - **Output:** `running 1 test ... test result: ok. 1 passed` ✓
  - run `cargo clippy -p rai-core -- -D warnings`.
  - **Output:** `Finished` ✓ (zero warnings)
- GOAL_MATCHED? → YES (both VERIFY commands pass, spec met).
- COMMIT: `T03: StopReason exhaustiveness test. Goal: 11-variant assertion compiles + passes.`
- NEXT TASK → T04.

**Iteration 1 with a failure (hypothetical):**
- VERIFY: `cargo test` → `error[E0599]: no variant named 'StreamAborted' found`.
- FIX (extract specific failure): variant is actually named `StreamingAborted` in `loop_.rs`. Update the test array. Loop back to TEST.
- (Don't rewrite the whole test — just the one wrong variant name.)

**Circuit-breaker (hypothetical):**
- If `cargo test` fails with `no variant 'StreamAborted'` 3 times in a row (because you keep typo-fixing the wrong thing), STOP → `BLOCKED.md`: "T03: StopReason variant name mismatch, tried [StreamAborted, StreamingAborted, Stream Abort], need confirmation of the exact variant name in loop_.rs." Escalate.

---

## 5. When to escalate (BLOCKED.md)

Write `BLOCKED.md` and stop when:
- **Iteration cap (8) hit** — you've tried 8 times and it's not matched.
- **Circuit-breaker** — the same failure recurred 3 times.
- **Ambiguity** — the task spec is unclear and you'd be guessing.
- **Missing dependency** — a `DEPS` task isn't actually done.
- **Toolchain/environment failure** — `cargo` itself is broken, not your code.

`BLOCKED.md` format:
```markdown
## BLOCKED — T<id> (as of <date>)
- Goal: <copy from TASKS.md>
- Iterations used: N/8
- Last failure (exact): <file:line: message> — <command that produced it>
- Tried: <bullet list of fixes attempted, in order>
- Likely needs: <human review / stronger model / clarification / dependency fix>
- Suggested next: <your best guess at the right direction>
```

A `BLOCKED.md` is **not a failure** — it's the honest signal that the task needs a different kind of help. The alternative (claiming done with failing tests) is worse.

---

## 6. Goal-matching vs "tests pass"

"Tests pass" is necessary but not sufficient. **Goal matched** = the `VERIFY` commands pass **AND** the code actually meets the task's acceptance spec. Examples where tests pass but the goal isn't matched:
- The test is trivial / doesn't exercise the spec (e.g., `assert_eq!(1,1)`).
- The implementation does the wrong thing that happens to pass a weak test.
- The code compiles and tests pass but doesn't handle an edge case the spec named.

For each task, ask: *"Does this actually do what the GOAL says, not just pass the listed tests?"* If the tests are weak, write stronger ones (the `TESTS` field is a floor, not a ceiling).

---

## 7. Per-task hygiene

- **One task per branch of thought.** Don't do T05's work while on T03.
- **Commit only when goal matched.** No "WIP" commits in the kit flow (the SLM doesn't need them — each task is atomic).
- **Note what you studied.** In the commit message, name the reference/doc you studied the pattern from (the independence trail).
- **Keep `crates/rai-core` pure.** No TUI, no IO, no network in the core crate. The loop + types only.
- **Run all four verify commands**, not just `cargo test`. `clippy -D warnings` and `fmt --check` catch real issues.

---

## 8. The ITVF loop IS RAI Code's loop (meta)

The same loop you use to build RAI Code is the loop RAI Code uses per task to build apps. When you implement `crates/rai-core/src/escalation.rs` (the ITVF state machine), you're formalizing your own working method. Build it well — you're its first user.

— *RAI Labs P. Ltd. · [www.railabs.in](https://www.railabs.in) · reach@railabs.in*
