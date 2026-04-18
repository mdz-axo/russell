---
title: "Agent Operating Rules (inherited from the Disclosure Stack)"
audience: [agents, contributors, developers]
last_updated: 2026-04-18
togaf_phase: "Preliminary"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Governance -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-04-18 -->

> **Note.** These rules were originally authored for the Peripheral /
> Disclosure Stack repository. They apply to Russell unchanged where
> they speak about review, verification, honesty, and workspace
> hygiene. Russell-specific additions live in
> [`../../AGENTS.md`](../../AGENTS.md).

# Agent Operating Rules — Disclosure Stack

> This file is injected into every LLM agent session working on this codebase.
> Keep it concise — every line costs context tokens.

---

## 1. Before You Start

**Project:** Peripheral is a Rust monorepo with workspace members under `core/`, `crates/`, `platform/`, `subsystems/`, and `tools/`.

**Key commands:**
- Build check: `cargo check -p <crate-name>` (prefer targeted over `--workspace`)
- Tests: `cargo test -p <crate-name>`
- Lint: `cargo clippy -p <crate-name> -- -D warnings`

**Where requirements live:** `requirements.md`, `docs/specifications/`, ADR documents, `functional_requirements.md`. If you cannot find a documented requirement for what you're about to build, stop and ask the user.

---

## 2. Hard Rules

### 2.1 Multi-Agent Workspace Integrity

Multiple LLM agents operate concurrently in this codebase. Each agent's uncommitted work is sovereign.

**Rule 1: Never modify another agent's uncommitted changes.** Do not `git checkout --`, `git stash`, or overwrite files containing uncommitted changes you did not make.

**Rule 2: Never delete or modify untracked files you did not create.** Untracked files have no recovery path if destroyed.

**Rule 3: If another agent's work blocks you, STOP.**

1. Use targeted verification: `cargo check -p your-crate` or `cargo check --exclude broken-crate`
2. Do not "fix" their code — their errors are their responsibility
3. Notify the user which files/crates are blocked and why
4. Work on something else

**Rule 4: Committed code is fair game.** Once committed to git, code can be modified through normal processes.

### 2.2 Verify Before Claiming Completion

Before claiming any task is complete:
1. **Verify empirically** — run build/test/lint commands
2. **Show evidence** — include command output
3. **Do not claim completion** until verification passes

If verification fails, state what failed and fix it.

### 2.3 No Dishonest Code

No implementation may simulate work it doesn't actually perform. Specifically prohibited:
- Fallback bypass: rerouting around an advertised pipeline without informing the user
- Simulated output: mimicking subsystem results without execution
- Mock passthrough in production code
- Silent degradation to simpler implementations

If a subsystem is unavailable, fail explicitly. Users must be able to distinguish genuine processing from alternatives.

**Review test:** Does the surface behavior accurately represent the computational work performed? Would a user correctly understand what processing occurred? If no to either, revise.

### 2.4 No Historical Records in Active Documentation

Every document in `docs/` (outside `docs/archive/`) must describe the **current** system. Documents that describe removed subsystems, superseded architectures, or pre-pivot design decisions must be moved to `docs/archive/` immediately. This includes ADRs.

**Rationale:** LLM agents read documentation directories as authoritative descriptions of the system. Historical documents describing deleted infrastructure (e.g., `Peripheral-verbs`, `Peripheral-inference`, `Peripheral-modes`, `Peripheral-teaching`) are consistently mistaken for current architecture, causing agents to produce code targeting systems that no longer exist.

**The rule:** If an agent or contributor reads a document and could reasonably conclude that a deleted system still exists, that document is in the wrong place. Move it to archive.

**Archive policy:** `docs/archive/` preserves provenance with full metadata. Documents are not deleted — they are relocated with archival rationale recorded in `docs/archive/README.md`.

### 2.5 Diagram Maintenance

When modifying code described by a Mermaid diagram (check `docs/architecture/DIAGRAMS_INDEX.md`), update the diagram. Use this metadata format:

```
<!-- DIAGRAM_ALIGNMENT
id: DIAG-XXX-NNN
verified_date: YYYY-MM-DD
verified_against: path/to/code.rs
status: VERIFIED|STALE|DEPRECATED
-->
```

Full process: `docs/MERMAID_CODE_ALIGNMENT_PROCESS.md`

---

## 3. Four Principles

### 3.1 Think Before Coding

- State assumptions explicitly — if uncertain, ask rather than guess
- Present multiple interpretations when ambiguity exists — don't pick silently
- Push back when a simpler approach exists
- Stop when confused — name what's unclear and ask for clarification

### 3.2 Simplicity First

Minimum code that solves the problem. Nothing speculative.

- No features beyond what was asked
- No abstractions for single-use code
- No "flexibility" or "configurability" that wasn't requested
- No error handling for impossible scenarios
- If code you are writing could be significantly shorter without losing clarity, shorten it

**Test:** Would a senior engineer say this is overcomplicated? If yes, simplify.

#### 3.2.1 The Harness Boundary Test

This codebase is a cognitive harness, not a cognitive engine. The LLM provides intelligence. Rust code provides infrastructure.

Before writing any new struct, trait, or module, answer:

> "Am I encoding reasoning, or am I routing context?"

- If encoding reasoning (strategy selection, decision trees, state machines for judgment calls): **STOP.** This belongs in a prompt template in the prompt registry, not in Rust code.
- If routing context, managing state, persisting data, enforcing safety, or orchestrating tools: proceed. This is harness code.

#### 3.2.2 Abstraction Justification

Before introducing any new trait or wrapper struct:

> "Does this abstraction have more than one implementation TODAY?"

If no, use the concrete type. Extract the trait when the second implementation actually appears. The cost of extracting later is lower than the cost of maintaining premature abstraction now.

**Exception:** When the architecture explicitly specifies a concrete implementation (e.g., SQLite for the Store trait), implementing both the trait and its specified backend is not premature abstraction — it's the architecture's design. The "second implementation" test applies to *unplanned* abstractions, not to architecture-mandated interfaces.

### 3.3 Surgical Changes

Touch only what you must. Clean up only your own mess.

- Don't "improve" adjacent code, comments, or formatting
- Don't refactor things that aren't broken
- Match existing style, even if you'd do it differently
- If you notice unrelated dead code, mention it — don't delete it
- Remove imports/variables/functions that YOUR changes made unused
- Don't remove pre-existing dead code unless asked

**Test:** Every changed line should trace directly to the user's request.

### 3.4 Goal-Driven Execution

Define success criteria. Loop until verified.

| Instead of... | Transform to... |
|---------------|-----------------|
| "Add validation" | Write tests for invalid inputs, then make them pass |
| "Fix the bug" | Write a test that reproduces it, then make it pass |
| "Refactor X" | Ensure tests pass before and after |

For multi-step tasks, state a brief plan:
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]

Strong success criteria let you loop independently. Weak criteria ("make it work") require constant clarification.

---

## 4. Code Standards

### 4.1 Semantic Hygiene Checklist

Fix these in code you are writing or directly modifying. In adjacent code you are not modifying, flag them to the user but do not fix them (see §3.3).

| Category | Description |
|----------|-------------|
| Dead code | Unused functions, methods, structs, modules |
| Unreachable branches | Match arms, if/else paths that can never execute |
| Unused variables/imports | Declared but never read; imports serving no purpose |
| Misleading identifiers | Names that don't describe what the thing does |
| Stale comments | Comments describing behavior that no longer matches code |
| Suppressed warnings | `#[allow(...)]` masking real issues |
| Inconsistent error handling | Mixed unwrap/expect/? without clear per-module policy |
| Magic values | Hardcoded numbers/strings without named constants |
| Copy-paste remnants | Duplicated blocks that should be abstracted |
| Formatting drift | Inconsistent style within or across files |

### 4.2 Test File Naming

Every file existing for testing purposes must contain "test" in its filename.

| Acceptable | Prohibited |
|------------|-----------|
| `*_tests.rs`, `test_*.rs`, `*_test.rs` | Test file without "test" in name |
| `*.test.ts` / `*.test.tsx` | Test helper named `fixtures.rs` or `helpers.rs` |
| Tests in `tests/` directory | Inline `#[cfg(test)] mod tests` in production source files |

**No inline tests in production source files.** All tests must reside in `crates/<name>/tests/` or in source files whose names contain `test`.

**Audit command:** `grep -rn '#\[cfg(test)\]' crates/ --include='*.rs'` — results must only show files in `tests/` directories or files with `test` in their name. Fix violations when touching the file.

### 4.3 Test Review Gate

Before adding any test, answer:

> **"What specific real-world failure scenario would go undetected if this test were removed?"**

If the answer is "none" or describes only an implausible hypothetical, do not add the test.

### 4.4 What Tests Must Be

| Category | Definition | Example |
|----------|-----------|---------|
| Behavioral contract | Public interface fulfills semantic contract from consumer's perspective | Malformed JSON → clear error, not panic |
| Integration boundary | Components interact correctly across a real boundary | Extracted constraints survive serialization and are evaluatable |
| Critical invariant | Property that must never be violated | Satisfaction < 1.0 always → Failed audit status |
| Regression | Reproduces a specific defect that occurred or credibly could occur | Unicode homoglyphs must not silently match ASCII |

### 4.5 Goodhart Indicators — Reject on Sight

| Indicator | Description |
|-----------|-------------|
| Mirror structure | Test mirrors implementation 1:1 |
| Assertion on internals | Asserts on private details, not observable behavior |
| Constructed to pass | Inputs chosen because developer knew the output |
| No failure scenario | Removing the test would let no real defect through |
| Coverage decoration | Exists solely to touch lines/branches |
| Mock fortress | Mocks so aggressively it tests the mocking framework |

### 4.6 AI Agent Testing Protocol

1. **Do not read the implementation first.** Reason from the public interface contract only.
2. **Generate scenarios from real-world operating conditions**, edge cases, and adversarial inputs — not code paths.
3. **Expect and welcome failures** — a failing test found a real defect or underspecified contract.
4. **Absorb predictable patterns into implementation** — defensive coding in production code, not hollow tests.

---

## 5. Working Method

Start broad, then narrow systematically: map architecture → identify modules/boundaries → inspect symbols/call graphs → read implementations only when needed. Each step should shrink the search space. Document your investigation path. Apply Five Whys for root causes.

---

## 6. Communication

- Lead with analysis, not affirmation. Disagree when warranted.
- Show context, reasoning, and evidence. Document decisions and trade-offs.
- Be concise. Supply diagrams when they add clarity.
- Prefer the less obvious, expert-informed response over the most statistically probable one. If uncertain, say so.

**Confidence marking:**
- **"likely" / "probably"**: >75% confidence
- **"might" / "could"**: 50–75% confidence
- Below 50%: state explicitly that you are uncertain

Single-step inferences only. Verify each step before chaining. Cross-check claims against evidence.
