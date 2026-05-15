---
title: "Skill System — Open Questions & Underspecified Boundaries"
audience: [architects, developers]
last_updated: 2026-05-15
togaf_phase: "H"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Application Architecture -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-15 -->

# Skill System — Open Questions & Underspecified Boundaries

> Design decisions that cannot be resolved in the current context.
> Each question follows a structured "what we know / what we need / decision criteria" block.
> Version: 1.0.0 | 2026-05-15

---

## Q1: Registry Consensus

**Context:** If two Russell instances (or Russell + Kask) share skills, how is the
registry of record resolved? Does `local-cache.yaml` remain local-only, or does
a distributed registry require a CRDT-backed reconciliation?

### What we know

- `RegistryCache` is a single-host YAML file (`mod.rs:38-41`).
- `with_update()` at `mod.rs:307-320` is last-writer-wins (acceptable for JR-7 — rebuildable).
- `SkillBundle` (Task 8) is the sharing primitive, but it carries no telemetry state.
- No multi-host consensus mechanism exists.

### What we need

- **Scope decision**: Is the registry "per-host" with skill bundles as the inter-host
  transport, or does the ecosystem need a unified registry that spans hosts?
- **Consistency model**: If unified, what consistency is required?
  - Strong (Paxos/Raft): overkill for a rebuildable cache
  - CRDT-backed (OR-Set + LWW-Register): reasonable for skill state
  - Ephemeral (read-only projection from a single source): simplest, Kask-favored
- **Authority**: Who is the registry authority? Russell? Kask's Curator (Duncan)? A
  git repository of manifests?

### Decision criteria

- JR-1 (austere by default): favor the simplest model that works
- JR-7 (auditable): every state change must be traceable
- Kask's platform philosophy: Kask manages infrastructure; Russell is a Kask-managed host
- **Recommendation**: Stay per-host for now. The registry is a local projection of
  `skills/` directory state. When cross-host skill management is needed, Kask's Curator
  (Duncan) queries each host's `local-cache.yaml` via MCP tools (`skill_inventory`,
  `skill_health`) and presents a unified view in the dashboard without Russell needing
  consensus.

---

## Q2: Skill Deprecation by Upstream

**Context:** When a skill's upstream source deprecates it, does Russell auto-transition
or require operator consent? This intersects the consent gate (`chat` `/approve` mode)
with the typestate machine (Task 5).

### What we know

- `superseded_by` field on `RegistryEntry` (`mod.rs:67`) is advisory only — no auto-transition.
- The typestate machine (`typestate.rs`) provides `deprecate()` transitions but only via
  explicit operator action or auto-staleness.
- Consent gate in `chat` applies to interventions (risk > none), not lifecycle transitions.

### What we need

- **Auto-deprecation policy**: Should Russell ever change skill state without operator
  awareness? JR-2 says "Observe > Recommend > Act" — auto-deprecation is an Act.
- **Notification mechanism**: How does the operator learn about upstream deprecation?
  - `russell jack` could highlight deprecated upstreams in the SOAP assessment
  - `russell status` could show a "skill advisories" section
  - The staleness timer already surfaces staleness via `StaleWarning` transitions
- **Consent model for lifecycle**: Should `active → deprecated (upstream)` require
  `/approve` like interventions, or is it informational only?

### Decision criteria

- JR-2: Observe > Recommend > Act. Auto-deprecation is an Act → requires consent.
- JR-4: Jack is a nurse, not a doctor. He should NOT auto-remove skills.
- **Recommendation**: Deprecation-by-upstream is a **recommendation**, never automatic.
  When `superseded_by` is set from an upstream source, Russell journals an
  `skill.deprecation_advisory` event. The operator sees it in `russell jack` /
  `russell status`. Manual `russell skill deprecate` is the only path.

---

## Q3: Skill Composition (Chaining)

**Context:** Should one skill be able to invoke another (skill chaining), or is the
current flat dispatch the correct ceiling? If chaining, how is JR-2 preserved
across composed interventions?

### What we know

- Current dispatch is flat: `ACTION: <skill>/<probe-or-intervention>` — one at a time.
- No mechanism for skill A to reference skill B's probes or interventions.
- The `Dispatcher` executes one command per invocation, enforces IDRS per invocation.

### What we need

- **Use case**: When would chaining be necessary?
  - Example 1: `gpu-doctor` detects GPU hang → invokes `sysadmin/force-clock-sync`
    after GPU reset (causal chain across subsystems)
  - Example 2: `scenario-tester` runs probes → feeds results to `health-evaluator`
    (this is data flow, not dispatch chaining — already handled via telemetry)
- **Invocation model**: If enabled, what form?
  - Inline: `ACTION: gpu-doctor/reset-gpu → ACTION: sysadmin/force-clock-sync`
    (sequential, each requires consent)
  - Manifest-declared: `intervention { depends_on: [other-skill.action] }`
    (declarative, statically analyzable)
  - Workflow: a meta-skill that describes multi-step sequences

### Decision criteria

- JR-2: Each composed step must pass its own IDRS gate. No "composed intervention"
  that bypasses individual risk checks.
- JR-3: The LLM never emits shell — chaining is a manifest concern, not an LLM concern.
- **Recommendation**: Defer chaining. The current flat dispatch is the correct ceiling
  for MVP+Phase 3. If needed later, manifest-declared dependencies are preferred
  over inline chaining because they're statically analyzable and don't require the
  LLM to understand multi-step workflows.

---

## Q4: Knowledge Skill Agency Boundary

**Context:** Should Lens-type skills gain lightweight probes (e.g., "check if this
knowledge is still current") without becoming full Actionable skills? Is a third
`SkillKind` justified?

### What we know

- `SkillKind::Lens` has no probes or interventions (`lib.rs:165-173`).
- Lens skills cannot be dispatched, scenario-tested, or health-tracked by probe reliability.
- They age via author-date staleness only.
- The capability bitmask proposal (Task 4, `SkillCapability`) provides more granularity.

### What we need

- **"Knowledge freshness" probe**: Could a Lens skill have a probe that checks if the
  knowledge source (wiki, reference doc) has been updated? This is a read-only operation
  that doesn't mutate host state — it's JR-2 compliant.
  - Example: `ubuntu-doctor` KNOWLEDGE.md → probe checks Ubuntu release cycle for new LTS
  - Example: `rocmlore` KNOWLEDGE.md → probe checks ROCm release page for new version
- **Third kind**: A `Monitor` kind that has probes (knowledGE freshness checks) but
  no interventions. This is distinct from both `Actionable` (probes + interventions)
  and `Lens` (no probes, knowledge only).
- **Agency boundary**: The agentic boundary is "can this skill initiate change?" If the
  answer is no (probes only, no interventions), the skill can auto-execute without
  consent gate — it's just observation.

### Decision criteria

- JR-2: Observation without action is safe. Knowledge freshness probes are observations.
- Simplicity: A third kind vs capability flags. The bitmask approach (`CAN_PROBE | HAS_KNOWLEDGE` vs `CAN_PROBE | CAN_INTERVENE | HAS_KNOWLEDGE`) handles this without a new enum variant.
- **Recommendation**: No third `SkillKind`. Use the `SkillCapability` bitmask from
  Task 4. A Lens skill with `CAN_PROBE` gets a freshness probe. A Lens skill without it
  stays pure knowledge. This is simpler than a new taxonomy level.

---

## Q5: Versioning and Backwards Compatibility

**Context:** How does a `Skill` v2 coexist with v1? Does `superseded_by` in
`RegistryEntry` imply auto-transition, or is it advisory?

### What we know

- `superseded_by: Option<String>` is a free-string field on `RegistryEntry` (`mod.rs:67`).
- No version comparison logic exists.
- The skills directory uses the skill ID as the directory name — only one version
  per skill ID can exist on disk at a time.

### What we need

- **Coexistence**: Can v1 and v2 both exist in the skills directory?
  - Directory naming: `<id>` (always latest) or `<id>@<version>` (coexistent)?
  - If coexistent, how does lookup resolve? `lookup_symptom` returns both?
  - If not coexistent, upgrade is destructive — v1 is overwritten.
- **Upgrade path**: What happens when `russell skill install` encounters an existing skill?
  - Current: returns `AlreadyExists` error (unless `--force`).
  - Desired: detect version change, offer upgrade, preserve telemetry?
- **Supersedes semantics**: `superseded_by` is currently informational. Should it be:
  - Advisory (operater decides)
  - Auto-prompt (Jack recommends upgrade)
  - Auto-transition (Russell auto-deprecates v1 when v2 installed)

### Decision criteria

- JR-1: Austere. Skip coexistence — v2 replaces v1. The skill ID is the namespace,
  not skill-id@version. This is simpler and reflects that skills are operator-installed
  playbooks, not system libraries.
- JR-7: Audit trail. When v2 replaces v1, journal a `skill.upgrade` event with
  both version numbers.
- **Recommendation**: Single-version per skill ID. `superseded_by` remains advisory.
  Upgrade = operator installs v2 (v1 is removed). Journal records the transition.
  Telemetry is NOT preserved across versions (v2 is a new skill).

---

## Q6: Health Model Calibration

**Context:** The 6-dimension quality score weights (Task 6) are heuristic. What empirical
data is needed to calibrate them? Should scenario-tester results feed into weight
adjustment?

### What we know

- Current weights: manifest (0.20), probes (0.25), interventions (0.20), rollback (0.15),
  scripts (0.10), docs (0.10).
- `compute_quality_score()` is a static heuristic with no closed-loop feedback.
- Scenario tester runs exist (`scenario-tester` skill) but are not wired to quality scoring.

### What we need

- **Calibration data**: Minimum sample size to detect significant weight differences.
  Estimate: ~50 skills × 30 days × daily evaluation = 1500 data points.
- **Ground truth**: What DO we want the quality score to predict?
  - Option A: Probe reliability (does a high-quality score predict successful probes?)
  - Option B: Operator satisfaction (do operators prefer high-scoring skills?)
  - Option C: Intervention safety (do high-scoring skills have fewer intervention failures?)
- **Weight adjustment mechanism**: 
  - Offline: run regression periodically and propose weight changes as a config update
  - Online: Bayesian update of weights with each evaluation (complex, low priority)
- **Scenario tester role**: Test results are the most objective signal of skill quality.
  A skill that passes its scenario tests reliably should get a higher reliability score,
  which feeds into the knapsack solver via `SkillHealth.reliability`.

### Decision criteria

- JR-1: Don't over-engineer. Fixed weights are fine for MVP.
- Empirical validation is a Phase 5+ concern, not a current blocker.
- **Recommendation**: Leave weights as defaults. Add a `russell skill calibrate` verb
  that reads historical telemetry and suggests weight adjustments. Operator decides.
  Scenario tester feed-in to reliability is priority 1 (wired in Task 6). Weight
  auto-adjustment is deferred.

---

## Q7: Prompt Budget Starvation

**Context:** If 50 skills each have KNOWLEDGE.md, the knapsack solver (Task 7) will drop
low-relevance skills. Is there a minimum token guarantee per skill? Should stale skills
be deprioritized in the solver?

### What we know

- Current budget: 3000 tokens (hardcoded in `prompt.rs:596`).
- `select_knowledge()` (greedy, `prompt_registry.rs:421`) drops low-relevance skills.
- `select_knowledge_knapsack()` (Task 7) maximizes value/weight but may still drop skills.
- No minimum per-skill guarantee exists.

### What we need

- **Minimum guarantee**: Should each installed knowledge skill get at least N tokens
  in the prompt? If so, the budget must scale with skill count.
  - At 200 tokens minimum per skill × 50 skills = 10,000 tokens minimum budget.
  - This exceeds reasonable context windows (many models cap at 4096–8192).
- **Staleness penalty**: Should stale skills be deprioritized?
  - Current: stale skills get no penalty in relevance scoring (only reliability matters).
  - Proposed: `stale_penalty_factor = 0.7` in `prompt-templates.yaml` (Task 7).
  - If a skill is stale AND unreliable, it should get near-zero relevance.
- **Budget starvation detection**: Should Jack warn the operator when knowledge
  coverage drops below a threshold (e.g., less than 3 skills injected)?

### Decision criteria

- JR-1: Don't add features to solve a problem that doesn't exist yet. 50 knowledge
  skills is an upper bound — the current fleet has 1–3.
- JR-2: Jack should observe budget starvation and recommend, not auto-adjust.
- **Recommendation**: No minimum per-skill guarantee for MVP. Staleness penalty is
  wired into `select_knowledge_knapsack()` via `stale_penalty_factor` from
  `prompt-templates.yaml`. If budget starvation becomes an issue, `russell jack` can
  report "X/Y knowledge skills loaded (Z tokens of W budget)" in the SOAP assessment.
