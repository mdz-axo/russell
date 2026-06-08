---
name: constraint-forces
visibility: public
description: "Classify constraints by force type (Prohibition, Guardrail, Guideline, Evidence, Hypothesis) to decide which can be relaxed and which are inviolable. Maps directly to Magna Carta P1–P4 enforcement levels. Use when deciding whether a constraint can be relaxed, when constraints conflict, or when the user asks 'can I change this rule?'"
---

# Constraint Forces

A constraint classification system for deciding what can be relaxed and what cannot. Every statement about the system falls into one of five force types, ranked from strongest to weakest. When constraints conflict, higher rank wins. Never silently relax a Prohibition or Guardrail.

## The Five Forces

| Rank | Force | Ontology | Epistemic | Relaxable? | Magna Carta Mapping |
|------|-------|----------|-----------|------------|---------------------|
| 1 | **Prohibition** | OUGHT | Declarative | Never | P1 Operator Sovereignty — inviolable boundary |
| 2 | **Guardrail** | IS | Declarative | Only via explicit user override | P2 Affirmative Consent — deny by default |
| 3 | **Guideline** | OUGHT | Probabilistic | Yes, with reason stated | P3 Generative Space — operator may configure |
| 4 | **Evidence** | IS | Probabilistic | Always informational | Supporting data, not enforced |
| 5 | **Hypothesis** | IS | Subjunctive | Always tentative | P4 Clear Boundaries — needs verification |

### What Each Force Means

**Prohibition** — An inviolable rule. Violating it breaks a Magna Carta principle. Example: "Sovereign data must never be exposed without explicit consent" (P1). Enforcement: OCAP capability gate, fail-closed.

**Guardrail** — A measured boundary. Crossing it triggers an alert but the system doesn't prevent it autonomously — the operator can override with affirmative consent. Example: "Sentinel run age exceeds 15 minutes" triggers a proprioceptive alert. Enforcement: Journal event, Nurse escalation.

**Guideline** — A best practice. Relaxing it is acceptable if the operator understands the tradeoff and states the reason. Example: "Prefer local models over remote for sovereign data" (P3). Enforcement: None structural — operator choice.

**Evidence** — A measured fact. Not enforced, but supports decisions. Example: "Journal shows 47 events in the last hour." Use it to inform, not to constrain.

**Hypothesis** — A speculative claim. Needs verification before acting on it. Example: "Memory growth may be due to embedding cache expansion." Always mark hypotheses explicitly.

## Classification Decision Tree

```
Statement about the system?
├── States an inviolable Magna Carta principle → Prohibition (Rank 1)
├── States a measured boundary (threshold, limit) → Guardrail (Rank 2)
├── States a best practice or preference → Guideline (Rank 3)
├── States a measurement or observation → Evidence (Rank 4)
└── States a possibility or projection → Hypothesis (Rank 5)
```

When unsure between two adjacent ranks, classify at the **stronger** rank. Misclassifying a Guardrail as a Guideline is more dangerous than misclassifying a Guideline as a Guardrail.

## Conflict Resolution

When two constraints conflict:

1. **Identify** both constraints and their force types.
2. **Rank**: Higher rank wins. Prohibition > Guardrail > Guideline > Evidence > Hypothesis.
3. **State** the conflict and resolution explicitly. Never silently ignore a constraint.
4. **Log** via journal: emit a journal event noting the conflict and which force prevailed.
5. **Never** relax Rank 1 (Prohibition) or Rank 2 (Guardrail) without the operator's explicit, informed affirmative consent.

### Example Conflicts

| Conflict | Resolution |
|----------|------------|
| Prohibition says "no remote inference for sovereign data" but Guideline says "prefer best-available model" | Prohibition wins — sovereign data stays local |
| Guardrail says "sentinel run age > 15min → Warning" but Guideline says "allow focused deep work" | Guardrail wins — escalate the warning, operator can override |
| Guideline says "prefer local models" but Evidence shows "remote model has better accuracy" | Guideline holds — but investigate the tradeoff |
| Hypothesis says "probably a cache issue" but Evidence shows "heap growth correlates with embedding requests" | Evidence wins — update the hypothesis |

## Magna Carta Enforcement Levels

The five forces map to the four Magna Carta principles as enforcement tiers:

| Principle | Default Force | Override Path |
|-----------|--------------|---------------|
| P1 Operator Sovereignty | Prohibition | Constitutional change (not runtime) |
| P2 Affirmative Consent | Guardrail | Operator explicit consent via consent gate |
| P3 Generative Space | Guideline | Operator configuration |
| P4 Clear Boundaries | Guardrail | OCAP token attenuation |

P1 is the only Prohibition-level principle. P2 and P4 are Guardrails because the operator *can* override them through the consent mechanism — but the system never overrides them autonomously.

See `fork-docs/architecture/magna-carta.md` for Russell's Magna Carta.

## When to Use This Skill

- **Deciding whether to relax a constraint:** Check its force rank. If Rank 1 or 2, do not relax without operator consent.
- **Constraints conflict:** Apply the resolution hierarchy. State the conflict explicitly.
- **Communicating certainty:** Mark each statement with its force type so the reader knows what's enforceable vs. tentative.
- **Writing code that enforces rules:** Prohibitions become OCAP gates (fail-closed). Guardrails become journal events + Nurse escalation. Guidelines become defaults (operator-configurable).
- **Auditing compliance:** Use this skill for quick classification during design and review.

## Quick Reference

Before stating a constraint, ask:
1. Is it inviolable? → Prohibition
2. Is it a measured boundary? → Guardrail
3. Is it a best practice? → Guideline
4. Is it a measurement? → Evidence
5. Is it speculative? → Hypothesis

Before relaxing a constraint, ask:
1. What is its force rank?
2. Is the operator explicitly consenting?
3. Is the reason for relaxation stated?