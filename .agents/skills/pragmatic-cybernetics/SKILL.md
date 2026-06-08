---
name: pragmatic-cybernetics
visibility: public
description: "Cybernetic reasoning framework for analyzing Russell's feedback loops, variety engineering, and system homeostasis. Use when diagnosing sentinel timing issues, analyzing feedback loop failures, evaluating proprioception gaps, or reasoning about the system's self-regulation architecture. Pairs with constraint-forces for enforcement-level decisions."
---

# Pragmatic Cybernetics

A framework for reasoning cybernetically about Russell's homeostatic self-regulation system. Cybernetics isn't abstract theory here — the proprioception subsystem *is* a cybernetic system, and understanding its structure helps diagnose failures before they scream.

## Russell as a Cybernetic System

Every cybernetic system has five components. Here is Russell's:

| Component | Russell Implementation | What It Does |
|-----------|----------------------|--------------|
| **Sensor** | Sentinel probes + journal samples | Collects observations — host vitals, process counts, GPU metrics, disk pressure, systemd state |
| **Model** | Journal (SQLite with WAL) + EWMA baselines | Remembers what was seen. Sample rows are the canonical observations. Baselines track statistical norms. |
| **Regulator** | Proprioception subsystem + Nurse (Jack) | Compares current readings to baselines. Proprioceptive alerts when readings diverge. |
| **Actuator** | Skill dispatch + consent-gated interventions | Actions gated by risk bands and consent. The Nurse recommends; the operator consents; the dispatcher executes. |
| **Observer-of-observer** | Proprioception self-checks | "Is Russell regulating itself?" — 8 self-observation points, 2 boolean integrity checks. |

The feedback loop:

```
Host state → Sentinel probes (sensor) → Journal samples (model) → Proprioception (comparator)
    → Nurse/Jack (regulator) → Consent-gated skill dispatch (actuator) → Host state
```

Jack sits in the regulator box. He compares current readings against baselines and recommends. He never bypasses the consent gate — that's the Magna Carta contract (JR-3).

## The Viable System Model of Russell

| VSM System | Russell Component | Function |
|------------|------------------|----------|
| **S1 (Operations)** | Sentinel probes + skill dispatch | Primary activity: observe host, run probes/interventions |
| **S2 (Coordination)** | Journal scheduling + timer cadence | Anti-oscillation: 5-minute cadence prevents thrashing |
| **S3 (Control)** | Proprioception + EWMA baseline comparison | "Is this normal?" Deviation thresholds: 1.5× mild, 3× significant, 10× crisis |
| **S3\* (Audit)** | `russell verify-journal` + `russell self-triage` | Sporadic direct probe, bypassing cached state |
| **S4 (Intelligence)** | Nurse (Jack) via LLM inference | "What could this mean? What's coming?" |
| **S5 (Policy)** | Magna Carta P1–P4 + JR-1 through JR-7 | Identity, constraints, refusal posture |

The recursion principle: every component should be viable at its own level. The Nurse is viable if it can observe, compare, and recommend. A skill is viable if it can execute within its allowed env keys and sandbox. If any component lacks its own feedback loop, it is not viable — flag it.

## Feedback Loop Analysis

When diagnosing a proprioception alert or system issue, analyze the relevant feedback loop on five properties:

| Property | Question | Russell Diagnostic |
|----------|----------|-------------------|
| **Polarity** | Negative (stabilizing) or positive (amplifying)? | Proprioception is negative feedback by design. Positive feedback = runaway — critical. |
| **Delay** | How long between action and feedback? | Sentinel cadence (5 min), LLM latency, journal write stall |
| **Gain** | How strongly does feedback affect the system? | Baseline deviation sensitivity. Too high = missed anomalies. Too low = alert fatigue. |
| **Closure** | Is the loop actually closed? | Proprioceptive alert emitted but operator never sees it = broken closure |
| **Fidelity** | Does the signal accurately represent reality? | Probes only measure what they measure. Unmeasured failure modes = blind spots |

### Spotting Broken Feedback Loops

| Symptom | Cybernetic Diagnosis | What to Check |
|---------|---------------------|---------------|
| Sentinel last run age exceeds 15 min with no alert | Broken feedback closure — signal emitted, never consumed | systemd timer health, journal freshness |
| Baseline never deviates despite known problems | Sensor stall — observation loop broken | Sentinel probe execution, journal write path |
| Proprioceptive alerts fire repeatedly with no change | Positive feedback or gain too high | Check if same alert is re-emitting without new data |
| Journal chain integrity fails | Model-reality divergence | Hash chain verification, journal file corruption |
| LLM p95 latency exceeds threshold | S4 intelligence degradation | Okapi health, fallback adapter |

## Variety Engineering

Ashby's Law of Requisite Variety: the regulator's variety must match the system's disturbance variety. If the host can fail in 100 ways but Russell only monitors 10, that is a variety deficit.

### Russell's Variety Architecture

- **Raw variety:** Host produces many telemetry signals per sentinel cycle (CPU, memory, disk, GPU, systemd, processes)
- **Attenuation layer:** Sentinel probes aggregate raw signals into sample rows per probe type
- **Amplification layer:** When deviation exceeds threshold, the Nurse amplifies by explaining, recommending, and proposing actions

### Variety Analysis Checklist

1. Enumerate system variety: What failure modes, behavioral patterns, and edge cases exist for this host?
2. Enumerate regulator variety: What probes, baselines, and proprioceptive alerts cover them?
3. Is `regulator_variety >= system_variety`? If not, attenuate (add more probes) or amplify (add more Nurse escalation paths).
4. Check for gaps — unmeasured dimensions of host behavior.

## The Good Regulator (Conant-Ashby)

The Good Regulator theorem states: every good regulator of a system must be a model of that system. Applied to Russell:

1. The EWMA baselines are the regulator's model of the host's normal behavior.
2. Where does the model diverge from reality? Check: are there failure modes the baselines don't capture?
3. Is the model updated when the system changes? Stale baselines are worse than no baselines — a 30-day rolling window handles this.
4. Does the model include failure modes, or only success modes? A model that only tracks happy paths is not a Good Regulator.

## When to Use This Skill

- **Proprioceptive alert fires:** Which cybernetic function failed? (Usually sensor, model, or feedback closure.)
- **"Is Russell healthy?":** Check each VSM system. Are all five present and functioning?
- **Alerts fire repeatedly with no change:** Broken feedback loop or positive feedback.
- **Variety deficit is chronic:** Russell is in a rut. The Nurse should propose new probes or the operator should introduce new monitoring.
- **New probe proposed:** Variety analysis. Does this add regulatory burden? Is there requisite variety to handle the new disturbance path?
- **Skill seems stuck:** Check its sandbox and allowed env keys. Is the skill viable within its capability scope?

## Quick Reference Cards

### Feedback Loop Analysis
1. **Polarity:** Negative (stabilizing) or positive (amplifying)?
2. **Delay:** How long between action and feedback?
3. **Gain:** How strongly does feedback affect the system?
4. **Closure:** Is the loop actually closed?
5. **Fidelity:** Does the signal accurately represent what it claims?

### Variety Analysis
1. Enumerate system variety (failure modes, behavioral patterns)
2. Enumerate regulator variety (probes, baselines, Nurse escalation paths)
3. `regulator_variety >= system_variety`? If not, attenuate or amplify.

### Good Regulator Check
1. What is the regulator's model of the system?
2. Where does the model diverge from reality?
3. Is the model updated when the system changes?
4. Does the model include failure modes, or only success modes?