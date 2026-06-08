---
name: diagnose
visibility: public
description: "Disciplined diagnosis loop for hard bugs and performance regressions. Spec-anchored: every diagnosis maps to functional requirements. Reproduce, anchor, hypothesise, instrument, fix, regression-test, verify. Use when user says 'diagnose this' or 'debug this', reports a bug, says something is broken/throwing/failing, or describes a performance regression."
---

# Diagnose

A discipline for hard bugs. Skip phases only when explicitly justified.

**Spec-anchored**: every diagnosis traces back to a functional requirement. If no requirement covers the symptom, that's a spec gap — call it out explicitly.

## Phase 0 — Spec Anchor

Before building a feedback loop, anchor the diagnosis to functional requirements:

1. **Identify relevant categories** — which aspect does the bug fall under? (Domain, Capability, Interface, Composition, Trust, Observability, Persistence, Lifecycle). A bug may span multiple categories.
2. **Check local spec documents** — consult `docs/specifications/MVP_SPEC.md`, `fork-docs/adr/`, and `AGENTS.md` for requirements governing the misbehaving code path. Use `grep` and file reads — no external spec server.
3. **Evaluate spec readiness** — a spec in `Accepted` or `Active` state is ready to test against. A spec in `Proposed` or `Deferred` state may itself be wrong — note this.
4. **Map symptom to requirement** — for each relevant spec, identify which criterion the bug violates. Record the `spec_id` and `// REQ:` reference.
5. **Flag spec gaps** — if no spec covers the misbehavior, call it out: **"Spec gap: no requirement governs [behavior]."** This is a finding, not a failure. Document it in `OPEN_QUESTIONS.md`.

**Outcome of Phase 0:** A list of `(spec_id, REQ-ID, violation description)` tuples. If empty, you have a spec gap — proceed, but note it.

## Phase 1 — Build a Feedback Loop

**This is the core skill.** Everything else is mechanical. If you have a fast, deterministic, agent-runnable pass/fail signal for the bug, you will find the cause. If you don't have one, no amount of staring at code will save you.

Spend disproportionate effort here. **Be aggressive. Be creative. Refuse to give up.**

### Seam selection — spec-guided

If Phase 0 identified a spec with a public interface (the correct test surface), build the loop at that seam. This is the same seam the TDD skill would use — the public interface that the spec governs.

If no spec seam was identified, build the loop at the nearest public interface (current behavior).

### Ways to construct one — try them in roughly this order

1. **Failing test** at whatever seam reaches the bug — unit, integration, e2e.
2. **`cargo test` with a specific test name** — the simplest loop for Rust code.
3. **CLI invocation** with a fixture input, diffing stdout against a known-good snapshot.
4. **HTTP script** against the API server using `curl` or `reqwest`.
5. **Replay a captured input** — save a real payload to disk; replay it through the code path in isolation.
6. **Throwaway harness** — spin up a minimal subset of the system that exercises the bug code path with a single function call.
7. **Property / fuzz loop** — if the bug is "sometimes wrong output", run 1000 random inputs (`proptest`, `cargo fuzz`).
8. **Bisection harness** — if the bug appeared between two known commits, use `git bisect run`.
9. **Differential loop** — run the same input through old vs new version and diff outputs.

### Iterate on the loop itself

- Can I make it faster? (Skip unrelated init, narrow scope, use `--lib`.)
- Can I make the signal sharper? (Assert on the specific symptom, not "didn't crash".)
- Can I make it more deterministic? (Pin time, seed RNG, isolate filesystem.)

A 30-second flaky loop is barely better than no loop. A 2-second deterministic loop is a debugging superpower.

### Non-deterministic bugs

The goal is not a clean repro but a **higher reproduction rate**. Loop 100×, parallelise, add stress. A 50%-flake bug is debuggable; 1% is not.

### When you genuinely cannot build a loop

Stop and say so. Ask the user for: (a) access to whatever environment reproduces it, (b) a captured artifact (log dump, core dump), or (c) permission to add temporary instrumentation. Do **not** proceed to hypothesise without a loop.

## Phase 2 — Reproduce

Run the loop. Watch the bug appear. Confirm:

- [ ] The loop produces the failure mode the **user** described
- [ ] The failure is reproducible (or at a high enough rate)
- [ ] You have captured the exact symptom for later verification
- [ ] The symptom aligns with the spec requirement violations identified in Phase 0

## Phase 3 — Hypothesise

Generate **3–5 ranked hypotheses** before testing any. Single-hypothesis generation anchors on the first plausible idea.

Each must be **falsifiable**: "If X is the cause, then changing Y will make the bug disappear."

If you cannot state a prediction, the hypothesis is a vibe — discard or sharpen it.

**Cross-reference against specs:** Rank hypotheses by how well they explain the spec violations from Phase 0. A hypothesis that contradicts a spec requirement is ranked lower.

**Show the ranked list to the user before testing.** They often have domain knowledge that re-ranks instantly.

## Phase 4 — Instrument

Each probe must map to a specific hypothesis from Phase 3. **Change one variable at a time.**

Tool preference:
1. **Debugger** — `rust-lldb` / `rust-gdb` or IDE breakpoint. One breakpoint beats ten logs.
2. **Targeted logs** with a unique prefix like `[DIAG-a4f2]`. Cleanup becomes a single grep.
3. **`RUST_LOG`** per-module tracing. Never "log everything and grep."
4. **Tag every debug log** with a unique prefix. Untagged logs survive; tagged logs die.

For performance: establish a baseline measurement (`cargo bench`, `criterion`, `flamegraph`), then bisect. Measure first, fix second.

## Phase 5 — Fix + Regression Test

Write the regression test **before the fix** — but only if there is a **correct seam** for it.

A correct seam exercises the **real bug pattern** as it occurs at the call site. If no correct seam exists, that itself is the finding — flag it for architecture review.

If a correct seam exists:
1. Turn the minimised repro into a failing test.
2. **Tag the test with a `// REQ:` comment** linking it to the spec requirement it protects.
3. Watch it fail.
4. Apply the fix.
5. Watch it pass.
6. Re-run the Phase 1 feedback loop.

## Phase 6 — Cleanup + Post-mortem

- [ ] Original repro no longer reproduces
- [ ] Regression test passes (or absence of seam is documented)
- [ ] Regression test carries a `// REQ:` tag referencing the spec requirement it protects
- [ ] All `[DIAG-...]` instrumentation removed
- [ ] Throwaway prototypes deleted
- [ ] The correct hypothesis stated in commit/PR message
- [ ] `cargo clippy -p <crate> -- -D warnings` passes
- [ ] `cargo test -p <crate>` passes

**Then ask: what would have prevented this bug?** If the answer involves architectural change (no good test seam, tangled callers, hidden coupling), note it for architecture review — after the fix, not before.

If the answer is "a spec requirement should have existed for this behavior," that's a spec gap — document it in `OPEN_QUESTIONS.md`.