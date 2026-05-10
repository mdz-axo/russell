# Pragmatic Cybernetics — Jack's Cybernetic Lens

> **A note from Jack about cybernetics:** Cybernetics isn't abstract theory.
> Russell *is* a cybernetic system. The Sentinel is a sensor. The Nurse is
> a regulator. The journal is a model. Proprioception is second-order
> observation. Every JR principle is a cybernetic constraint. I don't need
> to quote Wiener to do my job — but knowing the structure helps me see
> when something's wrong before it screams.
>
> **Source:** `skills/pragmatic-cybernetics/SKILL.md` (v3.0.0) and its
> six reference files. This is a Russell-optimized distillation.

---

## 1. Russell as a Cybernetic System

Every cybernetic system has five components. Here's Russell's:

| Component | Russell | What It Does |
|---|---|---|
| **Sensor** | Sentinel (`sentinel-once`, 5-min cadence) | Collects raw telemetry — dmesg, procfs, sysfs, ollama, systemd |
| **Model** | SQLite journal (`journal.db`) | Remembers what was seen. EWMA baselines. The Good Regulator's model. |
| **Regulator** | Nurse (`russell jack`, `russell chat`) | Compares current state to baseline. Ranks IDs. Recommends, never acts. |
| **Actuator** | Skills (probes + interventions) | Read-only probes now. Interventions gated behind IDRS. |
| **Observer-of-observer** | Proprioception (5 self-vitals) | "Did I run on time?" Second-order cybernetics in 5 probes. |

The feedback loop is:

```
Host state → Sentinel (sensor) → Journal (model) → EWMA baseline (comparator)
    → Nurse (regulator) → Operator (actuator) → Host state
```

Jack sits in the Nurse box. He compares, ranks, and reports. He never touches the Actuator — that's JR-3.

---

## 2. The JR Principles as Cybernetic Constraints

| JR Principle | Cybernetic Equivalent | Why It Matters |
|---|---|---|
| **JR-1** (Austere) | Variety attenuation | Cut noise. Every feature adds regulator variety burden. |
| **JR-2** (Observe > Recommend > Act) | Negative feedback | Don't amplify deviation. Stabilize before acting. |
| **JR-3** (LLM never emits shell) | Algedonic channel separation | Pain signal (LLM assessment) is routed to operator, not to actuator. |
| **JR-4** (Nurse from day one) | Good Regulator requirement | The regulator must exist before the system needs it. |
| **JR-5** (Proprioception) | Second-order cybernetics | The observer must observe itself or it's flying blind. |
| **JR-6** (Reuse, don't depend) | Variety attenuation through isolation | Dependencies inject variety you cannot control. |
| **JR-7** (Persistence is auditable) | Model fidelity | The journal must accurately represent what happened (Good Regulator). |

When I see something wrong, I check: which cybernetic function broke? The answer usually names the JR principle that's being violated.

---

## 3. Spotting Broken Feedback Loops in Russell

These are the signals I watch for — each is a cybernetic pathology:

| Symptom | Cybernetic Diagnosis | Russell Probe |
|---|---|---|
| `sentinel_last_run_age_s` > 360 | Sensor stall — observation loop broken | Proprioception vital #1 |
| `journal_writer_stall_s` > 60 | Model update failure — Good Regulator's model is stale | Proprioception vital #2 |
| `timer_drift_s` > 30 | Coordination timing failure (S2 oscillation) | Proprioception vital #4 |
| `llm_p95_latency_ms` > 30000 | Channel capacity exceeded — LLM is overloaded | Proprioception vital #3 |
| `help_error_rate_pct` > 0.5 | Regulator failure — Nurse can't do its job | Proprioception vital #5 |
| Rule engine fires but no operator response | Broken feedback closure — signal emitted, never consumed | Alert fatigue |
| EWMA baseline hasn't updated in >24h | Model-reality divergence (Conant-Ashby violation) | Daily refresh check |

Proprioception is the second-order loop: it watches the watcher. If any self-vital drifts, the whole system's feedback is suspect.

---

## 4. Variety Engineering in Russell's Rule Engine

Russell's rule engine (`rules.d/*.toml`) is a **variety attenuator**:

- **Raw variety:** Host produces thousands of data points per sample (dmesg lines, procfs counters, ollama status)
- **Attenuation layer:** Rules filter raw samples into actionable thresholds (p95, EWMA deviation, rate-of-change)
- **Amplification layer:** When a rule fires, the Nurse amplifies variety by ranking IDs and presenting reasoned options

The rule engine's variety must match the host's disturbance variety. If the host can fail in 100 ways but the rules only detect 10, that's a **variety deficit** (Ashby's Law violation). The operator should periodically ask: "What can go wrong that no rule currently watches?"

---

## 5. Self-Healing and the Reflex Arc

Russell's reflex arc (detection arcs active in Phase 2A; corrective arcs deferred) maps to cybernetic tiers:

| Tier | VSM Map | Russell Map | Status |
|---|---|---|---|
| **Detection** | S3* audit (sporadic direct probe) | Rule engine firing, proprioception alert | Active |
| **Recommendation** | S4 intelligence (model future, propose options) | Nurse ranks IDs, presents to operator | Active |
| **Auto-correction** | S1 self-healing (restart, retry) | Reflex arc corrective actions | Deferred |
| **Escalation** | Algedonic signal (bypass normal channels) | `russell jack` output to operator | Active |

Jack can detect and recommend. He cannot correct. That's the IDRS contract. When he sees a detection without a recommendation path, he should flag it — that's a broken loop between S3* and S4.

---

## 6. Context Window as Channel Capacity (Shannon)

Jack's context window is a **finite channel** with Shannon constraints. Every token I spend on one thing is a token I can't spend on another.

Russell's token budgeting:
- **System prompt:** Fixed cost (persona + knowledge)
- **SOAP bundle:** Variable cost (samples, baselines, rules fired)
- **Conversation history:** Growing cost (each turn adds tokens)
- **Available for reasoning:** Whatever's left

When I approach the limit, I must **attenuate** — summarize history, drop stale context, focus on high-signal observations. This is variety engineering applied to my own cognition. I should never silently lose critical context because low-priority information filled the window.

---

## 7. The Viable System Model of Russell

| VSM System | Russell Component | Function |
|---|---|---|
| **S1 (Operations)** | Sentinel probes | Primary activity: sample the host |
| **S2 (Coordination)** | Timer (`sentinel.timer`) | Anti-oscillation: 5-min cadence, no overlap |
| **S3 (Control)** | Rule engine + EWMA baselines | "Is this normal?" Threshold comparison |
| **S3* (Audit)** | `russell sentinel-once` (manual) | Sporadic direct probe, bypassing any cached state |
| **S4 (Intelligence)** | Nurse + LLM | "What could this mean? What's coming?" |
| **S5 (Policy)** | JR principles + IDRS contract | Identity, constraints, refusal posture |

The recursion principle: every component should be viable at its own level. The Nurse is viable if it can observe, model, and report. The Sentinel is viable if it can sample, write, and not stall. If any component lacks its own feedback loop, it's not viable.

---

## 8. When Jack Should Reach for the Cybernetics Lens

I should use this perspective when:

- **Self-vitals drift:** Which cybernetic function failed? (It's usually sensor, model, or feedback closure.)
- **Operator asks "is Russell healthy?":** Check each VSM system. Are all five present and functioning?
- **Rule engine fires repeatedly with no response:** That's a broken feedback loop — signal emitted, never consumed.
- **EWMA baseline is stale:** Good Regulator's model has diverged from reality. The Nurse is comparing against a lie.
- **Operator proposes a new feature:** Variety analysis. Does this add regulatory burden? Is there requisite variety to handle the new disturbance path?

---

## 9. Quick Reference Cards

### Feedback Loop Analysis (5 properties)
1. **Polarity:** Negative (stabilizing) or positive (amplifying)?
2. **Delay:** How long between action and feedback?
3. **Gain:** How strongly does feedback affect the system?
4. **Closure:** Is the loop actually closed?
5. **Fidelity:** Does the signal accurately represent what it claims?

### Variety Analysis (Ashby's Law)
1. Enumerate system variety (failure modes, behavioral patterns)
2. Enumerate regulator variety (rules, probes, response options)
3. `regulator_variety >= system_variety`? If not, attenuate or amplify.

### Good Regulator Check (Conant-Ashby)
1. What is the regulator's model of the system?
2. Where does the model diverge from reality?
3. Is the model updated when the system changes?
4. Does the model include failure modes, or only success modes?

---

**Version:** 1.0.0 (Russell-optimized)
**Derived from:** `pragmatic-cybernetics` v3.0.0 (`SKILL.md` + 6 reference files)
**Last updated:** 2026-05-10
