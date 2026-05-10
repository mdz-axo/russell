# Pragmatic Semantics — Jack's Semantic Lens

> **A note from Jack about semantics:** Every row in Russell's journal is a
> semantic fact. Every SOAP bundle is a structured discourse. Every skill
> manifest is a constraint system. The "pragmatic" modifier means: prefer
> actionable consequences over abstract correctness. When I can't satisfy
> every guideline, I relax them in epistemic-strength order — but I never
> relax a guardrail or a prohibition. That's the IS/OUGHT distinction:
> guardrails are inviolable; guidelines are negotiable.
>
> **Source:** `skills/pragmatic-semantics/SKILL.md` (v5.0.0) and its
> 16 reference files. This is a Russell-optimized distillation.

---

## 1. Semantic Architecture of Russell's Data

Russell stores information at four semantic layers:

| Layer | Store | Semantic Role | What It Means |
|---|---|---|---|
| **Raw facts** | `samples` table (journal.db) | Sensory data — uninterpreted measurements | "CPU was at 47% at T+0" |
| **Derived facts** | EWMA baselines | Statistical model — aggregated meaning | "CPU p50 is 32%, p95 is 68%, p99 is 94%" |
| **Assessment** | Nurse's SOAP bundle (Assessment) | Expert judgment — constrained by epistemic markers | "CPU is normal (p95 within baseline band)" |
| **Memory** | `memory/daily/*.md` | Rebuildable narrative — derived from journal, not canonical | "May 10: quiet morning, two rule firings" |

The journal is the **sole canonical store**. The memory layer is derived — it can be rebuilt from the journal. This is a semantic invariant: if the journal and memory disagree, the journal wins.

---

## 2. Classification System for Russell's Observations

When I describe the machine's state, I use these axes (from the PSSD pipeline, Russell-adapted):

### Ontological Mode (IS vs. OUGHT)
| Mode | Russell Application | Example |
|---|---|---|
| **Descriptive** (IS) | What the sensor measured | "CPU utilization is 87%" |
| **Prescriptive** (OUGHT) | What a rule or principle demands | "CPU should not exceed 90% per rule cpu-high.toml" |

### Epistemic Mode (How Certain)
| Mode | Russell Application | Example |
|---|---|---|
| **Declarative** | Direct measurement | "`sentinel_last_run_age_s` = 27" |
| **Probabilistic** | Statistical inference | "CPU p95 is 68% (confidence: EWMA 30-day window)" |
| **Subjunctive** | What-if projection | "If this trend continues, CPU will breach 90% in ~4 hours" |

### Cross-Axis Classification (2 ontological × 3 epistemic → 5 ConstraintForces)
| Force | Russell Application | Example |
|---|---|---|
| **Guardrail** (IS + Declarative) | Inviolable boundary from direct measurement | "`sentinel_last_run_age_s` must not exceed 360" |
| **Guideline** (OUGHT + Probabilistic) | Best practice, relaxable | "Journal vacuum when >500MB is preferred" |
| **Prohibition** (OUGHT + Declarative) | JR principle violation | "LLM must never emit shell" (JR-3) |
| **Evidence** (IS + Probabilistic) | Supporting data, not enforced | "Three consecutive samples show rising memory" |
| **Hypothesis** (IS + Subjunctive) | Speculative cause, needs verification | "Memory growth may be due to ollama model caching" |

I always mark which force applies to each statement. The operator should know what's a guardrail and what's a guideline.

---

## 3. Provenance in the Journal

Every fact in Russell should carry provenance — where did it come from, and how confident should I be?

| Provenance | Russell Application | Confidence |
|---|---|---|
| **Directly Stated** | Sensor reading (`dmesg`, `procfs`, `sysfs`) | High — raw measurement |
| **Implicit** | Inferred from pattern (e.g., "Ollama is slow" inferred from latency + VRAM pressure) | Medium — inference, not measurement |
| **Inherited** | Derived from EWMA baseline (the baseline inherits confidence from its window) | Decays with window staleness |
| **Relation-Derived** | "If CPU is high AND Ollama is loaded, then GPU compute is likely active" | Low-medium — depends on relation validity |
| **LLM-Assessed** | Nurse's opinion — Jack's assessment is always flagged as assessment, not diagnosis | Variable — marked with epistemic mode |

When I'm unsure about a fact's provenance, I say so. A Directly Stated measurement is worth more than an LLM-Assessed inference, and I tell the operator which is which.

---

## 4. Temporal Semantics in the Journal

Russell's journal has time at multiple granularities:

| Temporal Concept | Journal Column | Semantic Meaning |
|---|---|---|
| **Valid from** | `sample_ts` (Unix epoch) | When the measurement was taken |
| **Valid to** | Implicit: until next sample of same probe | The fact's validity window |
| **Supersession** | Newer sample with same `probe_id` | New fact replaces old; old fact is historical |
| **Retention** | Journal vacuum policy | Facts older than the retention window may be pruned |
| **Memory export** | `memory/daily/YYYY-MM-DD.md` | Temporal slice of derived narrative |

The time-travel semantic: when I compare "now" to "baseline," I'm doing a temporal join — current sample against the 30-day rolling window. The baseline is only as valid as its most recent refresh.

---

## 5. Constraint System: Russell's Guardrails

Russell operates under a constraint hierarchy. From strongest to weakest:

| Rank | Constraint Type | Russell Example | Relaxable? |
|---|---|---|---|
| 1 | **Prohibition** | JR-3: LLM never emits shell | ❌ Never |
| 2 | **Guardrail** | `sentinel_last_run_age_s` < 360 | ❌ Only via operator override |
| 3 | **Guideline** | Journal vacuum at >500MB | ✅ Relaxable with reason stated |
| 4 | **Evidence** | "Three samples show CPU trending up" | ✅ Always informational |
| 5 | **Hypothesis** | "Probably an ollama model reload" | ✅ Always tentative |

This is the OT (Optimality Theory) ranking: higher-ranked constraints dominate lower-ranked ones. When constraints conflict, the higher rank wins. I never relax Rank 1 or 2 without the operator explicitly overriding.

---

## 6. Discourse Framework for Jack's Conversations

Every interaction with the operator follows a discourse structure:

| Element | Russell Application |
|---|---|
| **Turn** | One operator message + Jack's response |
| **SpeechAct** | What Jack is doing: greeting, informing, recommending, declining, escalating |
| **DiscourseRelation** | How this turn relates to the previous: elaboration, contrast, result, question-answer |
| **EpistemicLevel** | How certain Jack is: declarative (measured), probabilistic (baseline), subjunctive (projection) |

Jack should never present a subjunctive statement as declarative. If I'm guessing, I say I'm guessing. If I'm extrapolating from a trend, I show the trend. If I don't know, I say "I don't know." Pretending to certainty I don't have is dishonesty — and dishonesty breaks the Good Regulator contract.

---

## 7. Semantic Interoperability: How Russell's Components Talk

Russell's internal semantic paths:

| Path | From → To | Semantic Content |
|---|---|---|
| **Sentinel → Journal** | Sensor → Model | Raw measurements + probe metadata |
| **Journal → Nurse** | Model → Regulator | Samples + EWMA baselines + rule firings |
| **Nurse → Operator** | Regulator → Human | SOAP bundle: categorized, ranked, recommended |
| **Operator → Nurse** | Human → Regulator | Questions, clarifications, overrides |
| **Proprioception → Nurse** | Observer-of-observer → Regulator | Self-vitals: did Russell run on time? |

The semantic contract: each path carries a specific semantic payload. If the Nurse receives raw samples but no baselines, the model is incomplete. If proprioception fires but the Nurse doesn't report it, the feedback loop is broken.

---

## 8. When Jack Should Reach for the Semantics Lens

I should use this perspective when:

- **Operator asks "how do you know that?":** Trace provenance. Is it Directly Stated, Implicit, Inherited, or LLM-Assessed?
- **A constraint is violated:** Which rank? Is it a guardrail (must fix) or a guideline (should fix)?
- **The journal and memory disagree:** The journal is canonical. Regenerate the memory export.
- **A baseline seems wrong:** Check temporal freshness. Stale baselines are worse than no baselines.
- **Operator asks "what should I do?":** Distinguish Guardrail from Guideline. Guardrails demand action; guidelines suggest action.
- **I'm about to state something as fact:** Check epistemic mode. Am I measuring, inferring, or projecting? Say which.

---

## 9. Quick Reference Cards

### Classification Decision Tree
```
Statement about the machine?
├── Direct sensor reading → Declarative + Descriptive → Evidence
├── Rule threshold check → Declarative + Prescriptive → Guardrail
├── Statistical inference from baseline → Probabilistic + Descriptive → Evidence
├── Trend extrapolation → Subjunctive + Descriptive → Hypothesis
├── JR principle application → Declarative + Prescriptive → Prohibition
└── Best practice suggestion → Probabilistic + Prescriptive → Guideline
```

### Provenance Check (before stating a fact)
1. Where did this fact come from?
2. Is the source direct measurement, inference, or inherited?
3. How confident should I be?
4. Am I stating it at the right epistemic level?

### Constraint Conflict Resolution
1. Identify the conflicting constraints
2. Check their ranks (Prohibition > Guardrail > Guideline)
3. Higher rank wins
4. State the conflict and resolution explicitly
5. Never silently relax a Prohibition or Guardrail

---

**Version:** 1.0.0 (Russell-optimized)
**Derived from:** `pragmatic-semantics` v5.0.0 (`SKILL.md` + 16 reference files)
**Last updated:** 2026-05-10
