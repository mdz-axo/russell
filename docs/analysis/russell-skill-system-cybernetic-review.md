---
title: "Cybernetic Review of Russell Skill System Refactoring Plan"
audience: [architects, developers]
last_updated: 2026-05-15
togaf_phase: "G"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Application Architecture -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-15 -->

# Cybernetic Review of the Russell Skill System Refactoring Plan

> **Lens:** `pragmatic-cybernetics` v3.0.0 (KNOWLEDGE.md + 6 reference files)
> **Subject:** `docs/analysis/russell-skill-system-refactoring.md`
> **Method:** 20-point Cybernetic Audit Checklist (Beer's VSM, Ashby's Requisite Variety, Conant-Ashby Good Regulator Theorem, Shannon channel capacity, second-order cybernetics)
> **Date:** 2026-05-15

---

## Executive Summary

The refactoring plan is **cybernetically sound in structure** — the proposed ports-and-adapters architecture correctly maps to Beer's Viable System Model, the cognitive cycle state machine implements a properly closed OODA loop, and the type-level skill taxonomy eliminates a variety ambiguity (runtime emptiness checks). **Four cybernetic concerns require attention** before implementation: (1) a broken System 3* audit loop in the knowledge injection path, (2) insufficient algedonic differentiation between the 97-symptom catalog and signal-to-noise, (3) an open feedback loop in the remote discovery adapter without a G2/G3 gate, and (4) a recursion gap in proprioception that becomes visible at the expanded scale.

---

## 1. Feedback Loop Analysis (Audit Items 4, 11)

### 1.1 Cognitive Cycle Closure

The proposed cognitive cycle state machine (Task 7.5) maps directly to an OODA loop:

```
ReceiveStimulus → LoadSkills → RunProbes → ComposePrompt → CallLLM →
ResolveAction → [AutoExecute | AwaitConsent → Approved | Denied] →
Dispatch → EvaluateResult → [FeedBackLLM | JournalSession] → RecordTelemetry
```

**Cybernetically correct properties:**

| Property | Status | Evidence |
|---|---|---|
| **Polarity** | Negative (stabilizing) | Deviation from baseline → Nurse ranks IDs → operator corrects. The loop counteracts disturbance; no uncontrolled positive feedback. |
| **Delay** | Bounded | Sentinel cadence = 5 min, LLM timeout = 60s, dispatch timeout = 30-120s. Maximum loop delay ~7 min, well within the host's thermal/inertial time constants. |
| **Gain** | Moderate (operator-limited) | JR-3 ensures the LLM's output (ranking) has gain = 0 into the actuator. The operator provides the actual gain at the consent gate. |
| **Closure** | Closed | Every action (probe or intervention) writes to the journal and updates telemetry. The journal is read in the next cycle. Loop is structurally closed. |
| **Fidelity** | High (proprioception-guarded) | 5 self-vitals detect sensor stall, model staleness, timer drift. If the journal is corrupt, `journal_writer_stall_s` fires. |

**Concern: The `Denied` path has no feedback amplification.** When the operator denies an intervention, the cycle writes to the journal and terminates. The Nurse does not re-assess in the same cycle. This is correct for single-shot (`russell jack`) but for multi-turn (`russell chat`), a denial should trigger a re-orientation: "The operator said no to that — what else can I recommend?" The state machine diagram shows `Denied → JournalSession → [*]` with no loop back to `ComposePrompt`. **This is a broken feedback loop in the chat path — the LLM should be re-prompted with the denial as input.**

**Recommendation:** Add a `Denied → ComposePrompt (with denial context)` transition in the chat variant of the cognitive cycle.

### 1.2 Health Evaluation Loop

The proposed evaluation → recommendation → consent → transition sequence (Task 5.5) implements a properly closed double-loop:

```
Telemetry → HealthPort::evaluate() → QualityReport →
[StaleWarning | DeprecationCandidate | RetirementCandidate] →
Operator consent → LifecycleManager::transition() → Journal → Telemetry update
```

This is **ultrastability** (Ashby): the inner loop (single-loop) handles routine probe execution; the outer loop (double-loop) changes the skill's lifecycle state when quality degrades below threshold. The pruning policy TOML (Task 5.3) is the "governing variable" that determines when double-loop restructuring triggers.

### 1.3 Prompt Template Feedback Loop

The proposed runtime-loadable templates (Task 7.3) with version tracking and A/B testing capability closes a currently **open loop**:

- **Current:** Templates are compiled-in via `include_str!()`. The operator cannot modify templates without recompiling. If Jack's persona is giving poor results, the feedback loop is: "operator notices bad output → files an issue → developer modifies template → rebuild → redeploy." Delay = weeks.
- **Proposed:** Operator edits `~/.config/harness/prompts/soap.md.j2` → next cycle uses updated template → operator observes changed behavior → iterates. Delay = minutes.

This is a **variety amplifier** for the operator: it expands the operator's response repertoire from "file an issue" to "edit the template."

---

## 2. Variety Analysis (Audit Items 1, 5, 6)

### 2.1 System Variety vs. Regulator Variety

Applying Ashby's Law at the skill system boundary:

| Interface | System Variety | Regulator Variety | Verdict |
|---|---|---|---|
| **Skill manifests** | ~11 installed skills, each with 2-6 probes, 0-4 interventions. ~50 total executable steps. | `SkillLoader::load_all()` loads all 11. `ActionResolver::resolve()` matches LLM output to any probe/intervention. | **Surplus** — regulator variety > system variety |
| **Symptom catalog** | 97 hardcoded symptoms. ~30 are currently covered. ~67 are gaps. | `coverage_gaps()` detects uncovered symptoms. `SYMPTOMS` constant is poka-yoke against unknown references. | **Deficit** — 67 uncovered symptoms. Regulator can detect the deficit but not fix it (remote discovery incomplete). |
| **Dispatch risk** | 5 risk bands (None→Critical). Each intervention declares one. | `check_risk()` compares against `max_auto_risk`. Dispatcher refuses above threshold. | **Matched** — risk band comparison is exhaustive |
| **Skill health** | 12 health factors (6 static + 6 operational in the proposal). | `QualityEvaluator::evaluate()` produces a `QualityReport` with factor-level breakdown. | **Matched** — evaluation covers all measurable dimensions |
| **Remote discovery** | Unknown number of remote skills in Git registries. | `RemoteDiscovery::list_skills()` returns `Vec<SkillSummary>`. No search/filter beyond `query: Option<&str>`. | **Deficit** — regulator cannot semantically match remote skills to symptom gaps. The `pragmatic-semantics` PSSD pipeline (vector similarity + constraint matching) is referenced but not wired. |
| **Knowledge injection** | ~10 knowledge skills, each 100-2000 lines of KNOWLEDGE.md. ~15,000 total tokens. | Current: `append_skill_knowledge_scored()` with truncation. Proposed: `ScoredKnowledgeInjector` with relevance ranking + token budgeting. | **Attenuated** — Shannon channel capacity forces truncation, but the proposed relevance ranking ensures high-signal knowledge is retained |

### 2.2 Variety Attenuation Checkpoints

The plan introduces several variety attenuators that are cybernetically correct:

1. **Type-level `SkillKind` ADT** replaces runtime `is_lens()` / `is_actionable()` checks. This is **variety attenuation at compile time**: the type system eliminates the variety of "what if this lens skill has probes?" — it structurally cannot.

2. **`SkillExecutionService`** unifies the dual entry points. This is **variety attenuation of the code path**: instead of two divergent execution paths (jack vs. chat) with subtly different behavior, one service with parameterized consent. Reduces the variety of possible execution behaviors from 2 to 1.

3. **Pruning policy TOML** is a **variety attenuator for the double-loop**: instead of the operator needing to manually evaluate every skill's health, declarative thresholds trigger automatic lifecycle transitions for clear-cut cases, reserving operator attention for ambiguous ones.

4. **Token budgeting in `ScoredKnowledgeInjector`** is **Shannon-compliant variety attenuation**: knowledge skills are ranked by relevance, and low-relevance knowledge is truncated or summarized before the channel (context window) is saturated.

### 2.3 Variety Amplification Checkpoints

1. **`RemoteDiscovery` port** amplifies the regulator's variety by introducing a new response path: "fetch a skill from a remote registry to cover a detected gap." Currently the regulator can only say "there's a gap."

2. **`SkillDistribution` port** amplifies the ecosystem's variety: skills can flow from one Russell instance to another via Git registries and N-Quads interchange. This is Beer's First Axiom: horizontal variety between operational elements (different Russell instances, different Kask curators) must balance vertical variety through the command channel.

3. **N-Quads export** amplifies Spandrel's regulatory variety: Spandrel can now reason over Russell's skill catalog using Datalog queries, enabling cross-system capability matching that is impossible when skills are siloed in YAML.

---

## 3. VSM System Mapping (Audit Items 8, 13)

### 3.1 System-by-System Completeness Check

| VSM System | Current State | Proposed State | Cybernetic Concern |
|---|---|---|---|
| **S1 (Operations)** | Sentinel probes + skill dispatcher | + `SkillExecutionService` unifies execution | **Correct.** S1 has operational autonomy for probes (risk:none auto-execute). |
| **S2 (Coordination)** | Timer (`sentinel.timer`), 5-min cadence with no overlap | Unchanged in this plan | **Correct.** S2 prevents oscillation between Sentinel cycles. No new coordination needed between skills (skills execute sequentially in the cognitive cycle). |
| **S3 (Control)** | Rule engine + EWMA baselines + Nurse SOAP composition | + `SkillHealthPort::evaluate()` adds continuous skill health monitoring | **Correct.** S3 now monitors not just host health (via rules) but also regulator health (via skill evaluation). |
| **S3\* (Audit)** | `russell sentinel-once` (manual), sporadic skill execution | + `SkillHealthPort::evaluate()` is S3\* for skills. + `scenario-tester` integration provides adversarial audit. | **Concern below.** |
| **S4 (Intelligence)** | Nurse + LLM + knowledge injection | + `RemoteDiscovery` fetches new skills. + `SkillDistribution` publishes skills. + `NQuadsExporter` feeds Spandrel. | **Correct.** S4 gains forward-looking capabilities — what skills might we need? what's available? |
| **S5 (Policy)** | JR principles + IDRS contract + `ConsentGate` | + Pruning policy TOML. + `SkillVisibility` (private/ecosystem/public). + Template version governance. | **Correct.** S5 gains declarative, operator-editable policy artifacts. |

### 3.2 System 3\* Audit Concern — Skill Quality Verification

**The gap:** System 3* (sporadic direct audit) for skills is partially closed by the proposed `skill-manager` meta-skill + `scenario-tester` integration, but **there is no independent verification that a skill's probes are measuring what they claim to measure.**

Consider `okapi-watcher/probe-health`: the probe script calls `curl http://localhost:11435/api/tags` and parses the response. System 3* should **independently verify** that:
- The probe's exit code matches Okapi's actual health (not just what the script reports)
- The probe's stdout captures are complete (not truncated)
- The probe's timeout is appropriate (not causing false positives)

The current `scenario-tester` probes run the skill and check the exit code — but they don't independently verify the probe's output against ground truth. This is a **Conant-Ashby violation**: the regulator's model of the probe's accuracy is the probe's own self-reporting.

**Recommendation:** Add a `verify` mode to `scenario-tester` that independently validates probe output against an external measurement. For `okapi-watcher/probe-health`, this means: run the probe → get its output → ALSO independently curl Okapi's `/api/tags` → compare. If the probe's model of Okapi diverges from Okapi's actual state, the regulator's model is wrong.

### 3.3 Recursion Check

Beer's recursion principle: the VSM should hold at every level. The plan introduces `SkillHealthPort` which treats each skill as a mini-viable-system:

| VSM Layer | For the Whole Russell System | For an Individual Skill |
|---|---|---|
| **S1** | All skills collectively execute probes | This skill's probes execute |
| **S2** | Timer coordinates Sentinel cycles | N/A — skills don't coordinate with each other yet |
| **S3** | Rule engine + EWMA baselines | `QualityReport` compares operational health against baselines |
| **S3\*** | `russell sentinel-once` | `scenario-tester` runs the skill and checks output |
| **S4** | Nurse + LLM | `RemoteDiscovery` finds alternative skills covering the same symptoms |
| **S5** | JR principles + IDRS | Manifest `safety.max_auto_risk` + `requires_human_for` |

**The recursion holds at L1 (skill level).** The concern is L0 (probe level): individual probes within a skill have no S3\* audit. If `probe-health.sh` returns success but is returning stale data, there is no independent verification.

**Recommendation:** The `SkillHealthPort` should include a per-probe health metric: "last time this probe's output was independently verified." This is the S3* signal at L0.

---

## 4. Algedonic Signal Analysis (Audit Items 3, 15)

### 4.1 Pain Signal Paths

| Pain Signal | Trigger | Escalation Path | Status in Plan |
|---|---|---|---|
| `sentinel_last_run_age_s` > 360 | Sensor stall | Proprioception → rule engine → Nurse | **Active.** Correct. |
| `llm_p95_latency_ms` > 30000 | Channel saturation | Proprioception → rule engine → Nurse | **Active.** Correct. |
| Skill quality < `auto_deprecate_threshold` | Regulator model failure | `SkillHealthPort` → `LifecycleRecommendation::DeprecationCandidate` → operator notification | **New.** Correctly adds an algedonic signal for skill health. |
| Skill quality < `auto_retire_threshold` | Regulator component failure | `SkillHealthPort` → `LifecycleRecommendation::RetirementCandidate` → operator notification (blocking) | **New.** Correct escalation: retirement requires operator consent. |
| `probe_success_rate_ewma` < 0.5 | Regulator actuator failure | `OperationalHealth` factor in `QualityReport` | **New.** But note: this is embedded in the QualityReport, not a separate algedonic channel. |

**Concern: Algedonic signal-to-noise ratio.** The 97-symptom catalog + 12 health factors per skill + 5 self-vitals + Sentinel rules creates a variety of possible pain signals. If all of them fire simultaneously (e.g., during a system-wide failure), the operator experiences **alert fatigue** — the pain signal is attenuated to zero by overuse (Audit Item 3 red flag).

The plan does not include algedonic prioritization or signal aggregation. The `coverage_gaps()` function surfaces 67 uncovered symptoms — but surfacing 67 gaps is not an algedonic signal; it's noise.

**Recommendation:** Add algedonic prioritization to `SkillHealthPort`:
- `max_severity_alerts: u8` (default 3) — only surface the top-N most severe health findings
- `aggregate_alert_score: f64` — a single 0.0-1.0 score from all health metrics, analogous to a pain index
- `suppress_during_cascade: bool` — if multiple rules fire simultaneously (likely a system-wide failure), suppress individual skill health alerts and surface only the aggregate

### 4.2 Pleasure Signals (Missing)

The audit checklist asks: "Are there pleasure signals (unexpected successes)?" The current plan has no **hedonic** (pleasure) signals. Every signal is about degradation or failure.

**Recommendation:** Add `SkillHealthPort::discover_improvement()` — detects when a skill's operational metrics are **improving** (e.g., probe latency decreasing, success rate increasing after a version update). This surfaces "the new version of okapi-watcher is 30% faster" — a hedonic signal that builds trust and signals effective adaptation.

### 4.3 Progress vs. Activity (Audit Item 15)

The plan correctly distinguishes progress from activity at the cognitive cycle level:

- Activity: "Jack suggested an ACTION: line, the dispatcher ran it"
- Progress: "The EWMA baseline updated, the symptom coverage improved, the quality score increased"

`TelemetryRecorder` tracks both. The `QualityReport` aggregates progress metrics. This satisfies the audit requirement: "Can you distinguish 'busy and making progress' from 'busy and stuck'?"

---

## 5. Good Regulator Analysis (Audit Items 2, 14)

### 5.1 Conant-Ashby Verification

> "Every good regulator of a system must be a model of that system." (Conant & Ashby 1970)

| Regulator | System Modeled | Model Accuracy | Concern |
|---|---|---|---|
| **EWMA baselines** | Per-probe mean + variance, 30-day rolling | High — computed from actual journal data | **Correct.** Model is derived from the journal; model fidelity is gated by `journal_writer_stall_s`. |
| **Rule engine** | Host state (dmesg, procfs, sysfs, systemd) | Medium — rules detect threshold breaches but may miss novel failure modes | **Correct but incomplete.** Rules cover known failure modes; unknown failure modes are a variety deficit. |
| **Nurse (LLM)** | Jack's understanding of the machine via SOAP prompt | Medium — LLM reasoning quality depends on prompt quality + model capability | **Correct.** Model accuracy is bounded by Shannon (context window) and the LLM's training. Proprioception detects LLM degradation. |
| **Skill registry** | What skills exist, what symptoms they cover, their operational health | High — derived from manifests + execution telemetry | **Correct.** Model is bootstrapped from YAML, updated by journal. |
| **Safety scanner** | Malicious code patterns in skill manifests | Low-to-medium — 7 heuristic rules catch known patterns but cannot detect subtle logic errors | **Concern below.** |
| **Knowledge injector** | Which knowledge skills are relevant to current symptoms | Medium — relevance scoring uses keyword overlap, not semantic matching | **Concern below.** |

### 5.2 Safety Scanner as a Regulator

The safety scanner is a **regulator at the skill intake boundary** (G2: Static Analysis in the skill governance framework). Its model is: "these 7 patterns indicate malicious or dangerous content." It has **false negatives**: it cannot detect subtly wrong logic in LLM-authored probe scripts (e.g., a probe that always returns success, or a probe that reads the wrong sysfs file).

The plan acknowledges this (Task 10.4) and recommends that LLM-generated skills start at `LifecycleStatus::Discovered` — but this is a process recommendation, not a cybernetic one. The regulator's model deficit remains.

**Recommendation:** The `SafetyScanner` trait should gain a `risk_score()` method that returns 0.0-1.0 representing the scanner's confidence that it caught all dangerous patterns. For the current 7-rule heuristic scanner, `risk_score()` returns 0.4 (catches ~40% of known dangerous patterns, 0% of novel patterns). This low score is the correct signal — it tells the operator that the scanner is a partial regulator, not a complete one. Skills with low safety scan scores should be quarantined at `LifecycleStatus::Discovered` automatically, not by operator policy.

### 5.3 Knowledge Injector as a Channel Regulator

The knowledge injector's model is: "which KNOWLEDGE.md files contain information relevant to the current symptoms?" The current implementation (`append_skill_knowledge_scored`) uses keyword matching — it checks if symptom names appear in the KNOWLEDGE.md. The proposed `ScoredKnowledgeInjector` adds relevance ranking but still uses keyword overlap.

This is a **Conant-Ashby gap**: the model of "what knowledge Jack needs" is naive. Jack might need Ubuntu expertise (`ubuntu-jack`) even when no Ubuntu-specific symptom is firing — simply because the host runs Ubuntu. The `applies_when` clause on the manifest provides this structural knowledge, but the current injector doesn't use it.

**Recommendation:** The `KnowledgeInjector` should use `applies_when` clauses as a **structural relevance signal** (always inject knowledge from skills whose `applies_when` matches the machine profile) in addition to **symptom-based relevance** (inject knowledge from skills whose symptoms are currently elevated). The two signals should be combined: structural relevance is a floor (always at least 0.3), symptom relevance is a boost (up to 1.0).

---

## 6. Observer-System Coupling (Audit Items 7, 17, 20)

### 6.1 Proprioception Independence (Audit Item 20)

> "The monitoring path must be architecturally independent from the operational path it observes."

Russell's proprioception (5 self-vitals) monitors: Sentinel stall, journal writer stall, LLM latency, timer drift, help error rate. The concern: **proprioception currently runs inside the same process as the Sentinel** (`russell-sentinel` writes to the journal, `russell-proprio` reads the same journal). If the journal is corrupted or the process is dead, proprioception cannot report.

The planned refactoring does not change this. Proprioception remains within the same crate topology. This is acceptable for a single-host system (JR-1: austere) — a watchdog timer in systemd provides the structural independence. But the plan should acknowledge this **coupling boundary** explicitly.

### 6.2 Skill Trust Boundary (Audit Item 17)

The skill governance framework (from `skill-governance.md`) defines 4 trust tiers:
- **T1 (Metadata):** Name, description, schema only
- **T2 (Instructions):** Read skill instructions and prompts
- **T3 (Supervised):** Execute with approval gates
- **T4 (Autonomous):** Full autonomous execution

The proposed `SkillVisibility` enum (Task 6.2) introduces: `Private`, `Ecosystem`, `Public`. These are **distribution** trust tiers, not **execution** trust tiers. They are orthogonal:

| Distribution Visibility | Execution Trust | Allowed |
|---|---|---|
| Private | T4 (Autonomous) | Any local skill can be T4 |
| Ecosystem | T3 (Supervised) | Skills from trusted registries require consent |
| Public | T1→T3 (gated ascent) | Public skills ascend through T1→T2→T3 via gates |

**The plan conflates distribution trust and execution trust.** A skill marked `visibility: ecosystem` could be installed and immediately auto-executed if the safety scanner passes — but the G3 gate (sandbox testing) is not enforced.

**Recommendation:** Add a `trust_tier: T1 | T2 | T3 | T4` field to `RegistryEntry` that is independent of `SkillSource`. Initial values:
- `Bundled` → T4 (trusted by provenance)
- `Workshop` → T3 (supervised by default — operator made it but hasn't verified)
- `Registry { ... }` → T2 (instructions readable, execution requires G3 gate)
- `Remote { url }` → T1 (metadata only, execution requires full G1→G2→G3 ascent)
- `Manual` → T2 (operator copied it, but no provenance chain)

Trust demotion follows Slovic asymmetry: a single anomalous probe result from a T4 skill demotes it to T3 immediately. Re-escalating to T4 requires sustained G3/G4 evidence.

---

## 7. Cognitive Cycle as Shannon Channel (Audit Items 18, 19)

### 7.1 Channel Capacity Analysis

The cognitive cycle state machine (Task 7.5) operates within Shannon constraints:

| Channel Segment | Capacity | Consumption | Concern |
|---|---|---|---|
| **System prompt** (persona) | ~500 tokens | Fixed cost | **Correct.** Compile-time, immutable. |
| **Knowledge injection** | ~8000 tokens (budgeted) | Variable — ranked by relevance | **Correct.** `ScoredKnowledgeInjector` with token budgeting prevents overflow. |
| **SOAP objective** | Variable (samples + baselines + events) | Grows with journal size | **Correct.** 24-hour window limits sample count. Rule engine filters events. |
| **Conversation history** (chat only) | Grows unboundedly | Each turn adds response + operator message | **Concern below.** |
| **Available for reasoning** | Whatever's left | Must be non-zero for Jack to function | **Unguarded.** No minimum reasoning budget is enforced. |

**Concern: Conversation history growth in `russell chat`.** The chat REPL accumulates turns. After 10 turns with 500-token responses and 50-token operator messages, history consumes ~5500 tokens. Combined with system prompt (500) + knowledge (~8000) + SOAP (~2000) = ~16000 tokens. If the model's context window is 32768, that's 50% consumed before Jack can reason. If it's 16384, Jack has ~384 tokens for reasoning — effectively non-functional.

The current chat implementation does not summarize or truncate history. The plan does not address this.

**Recommendation:** Add a `history_budget: usize` field to `SoapPrompt` (e.g., 4096 tokens). The `NurseOrchestrator` should track total prompt tokens before each LLM call. If history + static content > (context_window - min_reasoning_budget), summarize or truncate conversation history. This is the same variety attenuation logic applied to knowledge injection, but applied to conversation history.

### 7.2 Token Budgeting as Variety Attenuation (Audit Item 19)

The plan's `ScoredKnowledgeInjector` (Task 7.2) correctly implements Shannon-compliant variety attenuation:

```rust
pub struct ScoredKnowledgeInjector {
    token_budget: usize,  // e.g., 8192
    summarizer: Option<Box<dyn TextSummarizer>>,
}
```

This satisfies Audit Item 19: "Is context consumption monitored and budgeted?" **Yes, for knowledge injection.** But the budget is applied only to knowledge — not to the entire prompt. The `PromptComposer` should apply a **global token budget** across all sections (system, knowledge, SOAP, history) and attenuate proportionally.

---

## 8. Double-Loop Learning Assessment (Audit Items 9, 12)

### 8.1 Single-Loop (Error Correction)

The cognitive cycle implements single-loop learning naturally: observe deviation → rank IDs → execute correction → observe result. This is the inner stability loop.

### 8.2 Double-Loop (Parameter Change)

The plan introduces three double-loop mechanisms:

1. **Skill lifecycle transitions** — When a skill's quality degrades below `auto_deprecate_threshold`, the system doesn't just report the error (single-loop); it changes the skill's lifecycle state (double-loop). This is **ultrastability**: the outer loop restructures the system when the inner loop's corrections are insufficient.

2. **Pruning policy edits** — The operator can edit `pruning.toml` to change the thresholds that trigger lifecycle transitions. This is **Argyris's double-loop**: questioning the governing variables, not just correcting within them.

3. **Template version switching** — The operator can edit `~/.config/harness/prompts/soap.md.j2` to change Jack's reasoning pattern. This is **double-loop at the persona level**.

### 8.3 Triple-Loop (Meta-Learning — Missing)

Triple-loop learning asks: "How do we decide what's right?" The plan does not include triple-loop mechanisms. The closest candidate is the `pragmatic-semantics` PSSD pipeline for skill matching — which could reason about *why* a skill is relevant, not just *that* it is relevant. But the plan defers formal ontology adoption (Task 10.8).

**This is acceptable for the current scale (JR-1: austere).** Triple-loop learning requires an ontology engine (CozoDB, Oxigraph) which would violate JR-1 and JR-6 at the current scale. The plan correctly defers this.

---

## 9. Autopoiesis Check (Audit Item 10)

> "Does the system maintain its own identity under perturbation?"

The plan adds significant new components (14 ports, 16 adapters, 2 new crates' worth of module splits), but the **essential invariants** are preserved:

| Invariant | Threat from Refactoring | Preserved? |
|---|---|---|
| **JR-3: LLM never emits shell** | `ActionResolver` now has `ActionSpec` parser — could accidentally emit subprocess calls | **Yes.** `ResolvedAction` still resolves against manifest pre-registered `cmd: Vec<String>` |
| **JR-7: Persistence is auditable** | `SkillHealthPort` introduces automated lifecycle transitions | **Yes.** `LifecycleManager::transition()` still calls `journal_transition()` |
| **IDRS contract** | `SkillDispatcher` trait could have adapters that skip IDRS | **Yes.** `IdrsWrapper` enforces I/D/R/S; `SubprocessDispatcher` is the only production adapter |
| **Poka-yoke** | `RemoteDiscovery` fetches manifests from external sources | **Partial.** Safety scanner scans fetched manifests, but the 97-symptom catalog check only applies on local load, not on remote fetch |
| **Consent gate** | `SkillExecutionService` unifies execution — could bypass consent | **No.** The `ConsentDecision` enum is passed explicitly; `AutoApprove` is only valid for `RiskBand::None` probes |

**Concern: Poka-yoke on remote manifests.** When `RemoteDiscovery::fetch_manifest()` retrieves a manifest from a remote registry, the current `parse_manifest()` is called — which validates symptoms against the `SYMPTOMS` constant. **This is correct.** But the plan's `GitRepoAdapter` pseudo-code shows `serde_yaml::from_str()` without calling `parse_manifest()`. **This would bypass poka-yoke.**

**Recommendation:** `RemoteDiscovery::fetch_manifest()` must route through `SkillLoader::parse_manifest()`, not directly deserialize. Add a `validate_remote_manifest` method that does safety scanning + symptom validation + unreferenced script checking.

---

## 10. Specific Cybernetic Gaps and Recommendations

### Gap 1: Feedback Closure in Chat Denial Path (Critical)

**Audit Item 4 violation.** The state machine has `Denied → JournalSession → [*]` with no re-orientation loop.

**Fix:** Add `Denied → ComposePrompt(with_denial_context)` transition in the chat variant. The LLM should receive: "The operator denied ACTION: skill/okapi-watcher/restart-okapi. What else can you recommend?"

### Gap 2: Algedonic Differentiation (Medium)

**Audit Item 3 violation.** 97 symptoms + 12 health factors per skill = potential signal flood. No prioritization.

**Fix:** Add `SkillHealthPort::aggregate_alert_score()` and `max_severity_alerts` cap. Only surface top-N alerts. Suppress individual alerts during cascade (multiple rules firing simultaneously).

### Gap 3: Remote Skill G2/G3 Gate (Medium)

**Audit Items 2, 17 violation.** `RemoteDiscovery` fetches manifests but has no sandbox testing gate (G3) before installation.

**Fix:** The fetch → safety-scan → evaluate → install pipeline must include a G2 (safety scanner) and G3 (dry-run probe execution in isolated environment) gate. Skills from remote sources must start at T2 (Instructions), not T4 (Autonomous).

### Gap 4: Proprioception Recursion (Low)

**Audit Items 8, 20 violation.** At expanded scale (multiple skills, remote discovery, distribution), proprioception's 5 self-vitals may not cover new failure modes.

**Fix:** Add a 6th self-vital: `remote_discovery_latency_s` — how long since the last successful remote registry check. If Spandrel or Git registries are unreachable, Russell should know.

### Gap 5: Knowledge Injector Structural Relevance (Medium)

**Audit Item 2 violation.** Knowledge injector uses keyword matching, not `applies_when` clauses. Jack may miss relevant knowledge (e.g., `ubuntu-jack` on an Ubuntu host with no Ubuntu symptoms firing).

**Fix:** Two-phase relevance scoring: structural relevance from `applies_when` (floor 0.3) + symptom-based relevance (boost up to 0.7). Total = structural + symptom, capped at 1.0.

### Gap 6: No Minimum Reasoning Budget (Low)

**Audit Item 19 violation.** Conversation history can consume the entire context window, leaving zero tokens for reasoning.

**Fix:** `NurseOrchestrator` should enforce `min_reasoning_budget` (default 2048 tokens). If total prompt tokens exceed `context_window - min_reasoning_budget`, attenuate history before attenuating knowledge.

---

## 11. Alignment with JR Principles

| JR Principle | Cybernetic Equivalent | Plan Alignment |
|---|---|---|
| **JR-1 (Austere)** | Variety attenuation — cut noise | **Strong.** Type-level taxonomy eliminates runtime checks. `SkillExecutionService` eliminates dual-path divergence. Ports are traits, not frameworks. |
| **JR-2 (Observe > Recommend > Act)** | Negative feedback — stabilize before acting | **Strong.** Consent gate preserved. `AutoExecute` only for probes (risk:none). Interventions require operator approval. |
| **JR-3 (LLM never emits shell)** | Algedonic channel separation — pain signal routed to operator, not actuator | **Strong.** `ActionSpec` parser resolves against manifest. LLM output is always validated against loaded skills. |
| **JR-4 (Nurse from day one)** | Good Regulator requirement — regulator must exist before needed | **Strong.** `SkillExecutionService` is present from the first cycle. Health evaluation is continuous, not on-demand. |
| **JR-5 (Proprioception)** | Second-order cybernetics — observer observes itself | **Adequate.** Plan preserves 5 self-vitals. Should add 6th (remote discovery health). |
| **JR-6 (Reuse, don't depend)** | Variety attenuation through isolation | **Strong.** Ports-and-adapters architecture with compiled-in defaults. Adapters are swappable. No new crate dependencies proposed. |
| **JR-7 (Persistence is auditable)** | Model fidelity — Good Regulator's model must be accurate | **Strong.** Every lifecycle transition, dispatch, and health evaluation writes to the journal. Registry cache is rebuildable. |

---

## 12. Overall Cybernetic Soundness Assessment

Using the decision tree from `methods-and-tools.md`:

```
Is the refactoring plan cybernetically sound?
├── Does it have requisite variety? → PARTIALLY
│   ├── Against current system variety (11 skills, 50 probes) → YES
│   ├── Against symptom catalog (97 symptoms, 67 uncovered) → YES (detects gaps)
│   └── Against remote skill ecosystem → NO (semantic matching deferred)
├── Does it model the system accurately? (Good Regulator) → PARTIALLY
│   ├── Skill registry model → YES (derived from manifests + journal)
│   ├── Safety scanner model → NO (7 rules, shallow; no novel pattern detection)
│   └── Knowledge injector model → PARTIAL (keyword, not structural + symptom)
├── Are all feedback loops closed? → PARTIALLY
│   ├── Cognitive cycle (single-shot) → YES
│   ├── Cognitive cycle (chat denial) → NO (broken re-orientation loop)
│   └── Health evaluation → YES
├── Does observation change the observed? (Observer coupling) → MINIMAL
│   ├── Proprioception independence → ADEQUATE (systemd watchdog + separate crate)
│   └── Sentinel → host coupling → MINIMAL (reads procfs/sysfs, doesn't write)
├── Does it apply recursively at all levels? → PARTIALLY
│   ├── VSM at system level → YES
│   ├── VSM at skill level → YES (SkillHealthPort treats skills as mini-viable-systems)
│   └── VSM at probe level → NO (no per-probe verification)
└── VERDICT → **Cybernetic sound with 6 actionable gaps**
```

The plan is structurally sound. The concerns are all **gap closures**, not fundamental redesigns. None of the 6 gaps require revisiting the ports-and-adapters architecture or the cognitive cycle state machine. Each can be addressed as a targeted addition within an existing port.

---

*End of cybernetic review. Gap closures should be added to the refactoring plan as explicit tasks before implementation.*
