---
name: pragmatic-semantics
visibility: public
description: "Epistemic discipline for classifying statements by certainty level and constraint force. Distinguish IS from OUGHT, declarative from probabilistic from subjunctive. Classify provenance of facts. Use when communicating about the system, justifying decisions, or when the user asks 'how do you know that?' or 'how certain are you?'"
---

# Pragmatic Semantics

A discipline for making honest statements about the system. "Pragmatic" means: prefer actionable consequences over abstract correctness. When you cannot satisfy every guideline, relax them in epistemic-strength order — but never relax a Prohibition or Guardrail. That is the IS/OUGHT distinction: guardrails are inviolable; guidelines are negotiable.

See `constraint-forces` for the enforcement-level classification. This skill covers the full epistemic framework: how to classify what you know, how you know it, and how to communicate it honestly.

## The Two Axes

Every statement about the system exists on two axes:

### Axis 1: Ontological Mode (IS vs. OUGHT)

| Mode | Meaning | Example |
|------|---------|---------|
| **Descriptive (IS)** | What is — a measurement or observation | "Proprioception shows sentinel_last_run_age_s = 320" |
| **Prescriptive (OUGHT)** | What should be — a rule, principle, or requirement | "Sentinel run age must not exceed 900s" (JR-5) |

Never present an OUGHT statement as an IS statement. "The sentinel should run more often" is prescriptive, not descriptive. Say which it is.

### Axis 2: Epistemic Mode (How Certain)

| Mode | Meaning | Example |
|------|---------|---------|
| **Declarative** | Direct measurement or self-evident fact | "This test passes" — verified by running it |
| **Probabilistic** | Statistical inference from data | "Based on 30 sentinel cycles, p95 duration is 2.3s" |
| **Subjunctive** | What-if projection, speculation | "If this trend continues, the journal will exceed 100MB in ~4 hours" |

Never present a subjunctive statement as declarative. If you are guessing, say you are guessing. If you are extrapolating, show the trend. If you do not know, say "I don't know." Pretending to certainty you don't have is dishonesty — and dishonesty breaks the Good Regulator contract.

### Cross-Axis Classification → Constraint Forces

The two axes cross to produce the five constraint forces (see `constraint-forces` for enforcement detail):

| Force | Ontology | Epistemic | Example |
|-------|----------|-----------|---------|
| **Prohibition** | OUGHT | Declarative | "Sovereign data must not be exposed without consent" (P1) |
| **Guardrail** | IS | Declarative | "Sentinel run age > 900s triggers proprioceptive alert" |
| **Guideline** | OUGHT | Probabilistic | "Prefer local models for sovereign data" |
| **Evidence** | IS | Probabilistic | "Three sentinel cycles show rising duration" |
| **Hypothesis** | IS | Subjunctive | "Duration increase may be due to disk I/O pressure" |

## Provenance of Facts

Every claim should carry provenance — where it came from, and how confident you should be.

| Provenance | Russell Source | Confidence |
|-----------|---------------|-----------|
| **Directly Stated** | Journal event, sample row, test result | High — verified observation |
| **Implicit** | Inferred from pattern (e.g., "inference is slow" from latency + GPU pressure) | Medium — inference, not measurement |
| **Inherited** | Derived from EWMA baseline (inherits confidence from its window) | Decays with window staleness |
| **Relation-Derived** | "If journal write stall > 5s AND disk pressure is high, then I/O is the bottleneck" | Low-medium — depends on relation validity |
| **LLM-Assessed** | Nurse (Jack) opinion — always flagged as assessment, not diagnosis | Variable — mark with epistemic mode |

When unsure about a fact's provenance, say so. A directly stated measurement outweighs an LLM-assessed inference, and you must tell the reader which is which.

## Temporal Semantics

Russell's journal has time at multiple granularities:

| Temporal Concept | Russell Implementation | Semantic Meaning |
|-----------------|----------------------|-----------------|
| **Valid from** | Sample/event timestamp | When the observation was made |
| **Valid to** | Until superseded by newer observation of same probe | The fact's validity window |
| **Supersession** | Newer sample replaces older | New fact replaces old; old fact is historical |
| **Retention** | Journal retention policy | Facts outside retention may be pruned |
| **Memory export** | Daily digest (`russell digest --format daily-log`) | Journal entries → narrative summary |

When comparing "now" to "baseline," you are doing a temporal join — current readings against the EWMA baseline's rolling window. The baseline is only as valid as its most recent refresh. A stale baseline is not a valid comparator.

## Semantic Architecture of Russell's Data

Russell stores information at four semantic layers:

| Layer | Store | Semantic Role | Example |
|-------|-------|-------------|---------|
| **Raw facts** | Journal samples + events | Uninterpreted observations | "GPU temperature = 72°C at T+0" |
| **Derived facts** | EWMA baselines | Aggregated meaning from raw facts | "p50 GPU temp = 65°C, p95 = 78°C" |
| **Assessment** | Nurse (Jack) output | Expert judgment constrained by epistemic markers | "GPU temperature is mildly elevated (72°C vs p95 78°C)" |
| **Memory** | Markdown exports (`memory/`) | Rebuildable narrative derived from journal | "Pattern: GPU temp spikes correlate with training jobs" |

The journal is the sole canonical source. Memory exports are derived — they can be rebuilt from the journal. If journal and memory disagree, the journal wins. This is a semantic invariant.

## Constraint Hierarchy

Russell operates under a constraint hierarchy from strongest to weakest:

| Rank | Constraint Type | Russell Example | Relaxable? |
|------|----------------|---------------|------------|
| 1 | **Prohibition** | P1: Sovereign data never exposed without consent | Never |
| 2 | **Guardrail** | Sentinel run age > 900s → Proprioceptive alert | Only via operator affirmative consent |
| 3 | **Guideline** | Prefer local models for sovereign data | Yes, with reason stated |
| 4 | **Evidence** | "Three sentinel cycles show rising duration" | Always informational |
| 5 | **Hypothesis** | "Duration increase may be due to disk I/O" | Always tentative |

This is an Optimality Theory ranking: higher-ranked constraints dominate lower-ranked ones. When constraints conflict, the higher rank wins. Never silently relax Rank 1 or 2.

## When to Use This Skill

- **"How do you know that?":** Trace provenance. Is it Directly Stated, Implicit, Inherited, or LLM-Assessed?
- **A constraint is violated:** Which rank? Is it a Prohibition (must fix) or a Guideline (should fix)?
- **Journal and memory exports disagree:** Journal is canonical. Regenerate the memory export.
- **A baseline seems wrong:** Check temporal freshness. Stale data is worse than no data.
- **"What should I do?":** Distinguish Prohibition from Guideline. Prohibitions demand action; guidelines suggest action.
- **About to state something as fact:** Check epistemic mode. Are you measuring, inferring, or projecting? Say which.

## Quick Reference

### Classification Decision Tree
```
Statement about the system?
├── Direct measurement or test result → Declarative + Descriptive → Evidence
├── Baseline deviation check → Declarative + Prescriptive → Guardrail
├── Statistical inference from journal → Probabilistic + Descriptive → Evidence
├── Trend extrapolation → Subjunctive + Descriptive → Hypothesis
├── Magna Carta principle application → Declarative + Prescriptive → Prohibition
└── Best practice suggestion → Probabilistic + Prescriptive → Guideline
```

### Provenance Check (before stating a fact)
1. Where did this fact come from?
2. Is the source direct measurement, inference, or inherited?
3. How confident should I be?
4. Am I stating it at the right epistemic level?

### Constraint Conflict Resolution
1. Identify the conflicting constraints
2. Check their ranks (Prohibition > Guardrail > Guideline > Evidence > Hypothesis)
3. Higher rank wins
4. State the conflict and resolution explicitly
5. Never silently relax a Prohibition or Guardrail