---
title: "ADR-0026: Metacognitive Layer (russell-meta)"
audience: [architects, developers]
last_updated: 2026-05-15
togaf_phase: "H"
version: "1.0.0"
status: "Accepted"
---

<!-- TOGAF_DOMAIN: Application Architecture -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Accepted -->
<!-- LAST_UPDATED: 2026-05-15 -->

# ADR-0026: Metacognitive Layer (`russell-meta`)

- **Status:** Accepted
- **Date:** 2026-05-15
- **Supersedes:** ADR-0016 (doctor-and-llm-router) — naming only; the
  architectural decisions in ADR-0016 remain valid.
- **Principle:** JR-4 (Small but present: the Nurse), JR-5
  (Proprioception)

## Context

The crate formerly named `russell-doctor` has grown beyond the "LLM
consultation" metaphor it was born under. It now performs:

1. **Prompt composition** — template-driven assembly with
   relevance-scored knowledge injection and token budgeting.
2. **LLM client abstraction** — routing, inference hint application,
   model resolution and correction.
3. **Action resolution** — parsing ACTION syntax from LLM output,
   dispatching to skills and Kask tools, consent gating.
4. **Help orchestration** — the full `russell jack` pipeline.
5. **Fallback reasoning** — rule-based logic when no LLM available.
6. **Self-assessment** — quality scoring, outcome tracking, prompt
   regression detection (via `OutcomeAggregator` pattern from Stack).

These are **metacognitive functions** — the system reasoning about
its own reasoning, allocating attention, selecting strategies,
adapting its parameters, and criticizing its own outputs. Calling
this crate "doctor" was a categorization error that:

- Confused the *persona* (Jack the Nurse) with the *mechanism*
  (the metacognitive substrate that enables Jack).
- Obscured the crate's actual architectural role as System 4
  (Intelligence) in VSM terms.
- Made it harder for contributors to understand what code belongs
  in this crate vs. `russell-skills` or `russell-core`.

## Decision

Rename `russell-doctor` to `russell-meta`. The persona is still
Jack. The crate is the metacognitive layer that enables Jack to
function.

## Consequences

### The Metacognitive Responsibilities

`russell-meta` owns all functions that involve the system reasoning
about itself:

| Function | Metacognitive Property |
|---|---|
| Prompt composition | **Attention allocation** — deciding what information to present to the LLM, scored by relevance |
| Inference hints | **Strategy selection** — adjusting temperature, token budget, and reasoning mode per cognitive context |
| Knowledge relevance scoring | **Context filtering** — determining which expertise is salient to the current situation |
| ACTION parsing | **Plan interpretation** — understanding what the reasoner proposes and translating it to executable operations |
| Outcome tracking | **Self-assessment** — measuring whether past recommendations were effective |
| Prompt registry | **Self-modification** — the ability to change one's own reasoning templates without recompilation |
| Fallback reasoning | **Graceful degradation** — maintaining function when the primary reasoning mechanism is unavailable |

### Why Metacognition Enables Adaptation

A system without metacognition cannot:

1. **Assess itself on the fly.** It cannot know whether its last
   recommendation was good or bad, cannot detect prompt regression,
   cannot notice that a knowledge skill has become stale or
   irrelevant.

2. **Evolve its own behavior.** Without template overrides and
   inference hints, changing how the system reasons requires
   recompilation. With metacognitive infrastructure, the operator
   can modify reasoning strategies (temperature, prompt structure,
   knowledge selection) at runtime via disk overrides.

3. **Criticize its own output.** The prompt registry's
   `[inference]` system and the outcome aggregator pattern create
   the foundation for self-critique loops: render → validate →
   score → adjust.

4. **Allocate attention.** Without relevance scoring and token
   budgeting, the system dumps all available knowledge into the
   LLM context regardless of salience. The metacognitive layer
   makes the system *selective* — it chooses what to attend to
   based on the current situation, just as human metacognition
   filters attention.

### The Separation of Concerns

| Crate | VSM System | Function |
|---|---|---|
| `russell-core` | Infrastructure | Journal, events, profile, time — the substrate |
| `russell-sentinel` | System 1 (Operations) | Observation, telemetry collection |
| `russell-skills` | System 1 + 3 | Executable capabilities + IDRS enforcement |
| `russell-proprio` | System 3* (Audit) | Self-observation, proprioceptive vitals |
| **`russell-meta`** | **System 4 (Intelligence)** | **Reasoning about reasoning, strategy, adaptation** |
| `russell-cli` | Interface | Operator-facing commands, presentation |
| `russell-mcp` | Integration | External system communication |
| Operator | System 5 (Policy) | Consent, overrides, final authority |

### Reference Model Implications

For the Kask ecosystem, `russell-meta` establishes that every
cybernetic agent needs a dedicated metacognitive layer that:

1. **Separates the persona from the mechanism.** Jack is a voice;
   `russell-meta` is the engine.
2. **Makes reasoning strategies data-driven.** Templates, not
   code, define how the system thinks.
3. **Closes feedback loops on its own behavior.** Outcome tracking
   enables the system to learn whether its interventions work.
4. **Enables runtime adaptation without redeployment.** Disk
   overrides, inference hints, and relevance scoring are all
   adjustable without recompiling.

### Migration

- All `use russell_doctor::` imports become `use russell_meta::`.
- The `DoctorError` type retains its name for now (it describes
  errors from this crate; renaming to `MetaError` is optional
  future work).
- No behavioral change. The rename is purely a categorization fix.

## Notes

The name "meta" is chosen over:
- "nurse" — too tied to the persona; conflates role and mechanism
- "cognition" — implies the LLM itself, not the metacognitive
  envelope
- "intelligence" — overloaded with AI connotations
- "reasoning" — too narrow; misses attention allocation and
  self-assessment

"Meta" in this context means specifically **metacognition** — the
capacity to reason about one's own cognitive processes, monitor
their effectiveness, and modify them adaptively.
