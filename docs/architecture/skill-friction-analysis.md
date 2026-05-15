# Skill System — Organic Growth Friction Analysis

> Per-subsystem root cause with code citations, quantified by lines touched to resolve.
> Version: 1.0.0 | 2026-05-15

---

## F1: Two-Tier Registry (Compiled-in `symptoms.yaml` + Runtime `local-cache.yaml`)

**Root cause:** The symptom catalog and the registry cache evolved from different ancestors. `symptoms.yaml` was an early poka-yoke guard (ADR-0023, JR-3: "LLM never emits shell — ranks IDs from loaded manifests only"). `local-cache.yaml` arrived later with the workshop/lifecycle system (ADR-0024). Nobody unified them because they serve different read patterns — the symptom catalog is a static allowlist loaded once; the cache is a dynamic index saved after every mutation.

**Code evidence:**

```
symptoms.yaml (compiled-in):
  crates/russell-skills/data/symptoms.yaml        (101 lines, ~116 symptoms)
  crates/russell-skills/src/symptom_catalog.rs:177 (load_symptoms_from_file + fallback)

local-cache.yaml (runtime):
  crates/russell-skills/src/registry/mod.rs:8     (persisted at ~/.local/share/harness/registry/)
  crates/russell-skills/src/registry/mod.rs:202    (load from disk)
  crates/russell-skills/src/registry/mod.rs:220    (save to disk)
```

**Why not eliminate the cache altogether?** The registry cache isn't derivable fast enough at load time. `RegistryCache::load()` is a single `serde_yaml::from_str()` call (~100μs for N skills). Re-deriving from `load_all(skills_dir)` + journal queries requires:
- `load_all()` — filesystem walk + YAML parse for N skills (~1ms/skill)
- `JournalReader` queries for telemetry reconstruction — SQLite scans over the events table (potentially millions of rows)
- Worst-case latency: 500ms–2s for a mature registry with 50+ skills and months of telemetry

**Resolution cost:** ~180 lines touched.

1. Make `RegistryCache` an in-memory facade over two backing stores:
   - `SkillIdentityIndex` — rebuilt from `load_all()` on every startup (fast, stateless)
   - `SkillTelemetryStore` — persisted to `local-cache.yaml` (only telemetry fields, not identity/lifecycle)
2. Derive identity-only fields (version, symptoms, source, bundled) from manifest reads on every startup
3. Re-export telemetry back to YAML for persistence only
4. Remove identity duplication from `local-cache.yaml`

**Artifacts:**
- `crates/russell-skills/src/registry/mod.rs`: ~60 lines (split `RegistryEntry` into identity vs telemetry)
- `crates/russell-skills/src/registry/identity.rs`: ~50 lines (new module, identity index)
- `crates/russell-cli/src/commands/skill.rs`: ~30 lines (use new identity index)
- `crates/russell-cli/src/commands/workshop.rs`: ~40 lines (use new identity index)

---

## F2: Dual Prompt Composition Paths (Legacy `compose_with_kask` vs Templated `compose_templated`)

**Root cause:** `compose_templated()` was built as a drop-in replacement but never wired because its caller (`compose_and_augment_soap()` in `help.rs:102`) requires a `PromptRegistry` at the call site — and `PromptRegistry` is a MiniJinja `Environment` that must be initialized once at startup, not per-invocation. The CLI boot pathway constructs `russell-cli` commands before `russell-meta` prompt infrastructure is ready, creating a dependency inversion that was never resolved.

**Code evidence:**

```
Legacy path (active):
  crates/russell-meta/src/help.rs:102    compose_and_augment_soap()
  crates/russell-meta/src/prompt.rs:48   compose() → compose_with_kask()
  crates/russell-meta/src/prompt.rs:62   compose_with_kask() — 280 lines of writeln!()
  crates/russell-meta/src/prompt.rs:663  append_skill_knowledge() — unconditional

Templated path (unused):
  crates/russell-meta/src/prompt.rs:357  compose_templated() — 100 lines
  crates/russell-meta/src/prompt.rs:575  append_skill_knowledge_scored()
  crates/russell-meta/prompts/templates/soap.md.j2
  crates/russell-meta/prompts/templates/chat_objective.md.j2
```

**What blocks retiring the legacy path:**

1. `PromptRegistry` initialization is absent from CLI bootstrap. `russell-cli` main creates commands without a shared prompt registry.
2. `compose_and_augment_soap()` signature takes no `PromptRegistry` parameter. Adding one requires threading through 3 call sites.
3. The `compose_with_kask()` output has slightly different `rendered` field formatting (inline writeln! vs template) — migration must be verified with snapshot tests.
4. `compose()` is tested directly in `prompt.rs:913-983` with legacy path assumptions. New tests exist for `compose_templated()` but integration tests only test the legacy path.

**Resolution cost:** ~250 lines touched.

1. Add `PromptRegistry` construction to CLI bootstrap in `russell-cli/src/main.rs` (~15 lines)
2. Thread `&PromptRegistry` through `help.rs:compose_and_augment_soap()` → `compose_templated()` (~30 lines)
3. Switch `compose_and_augment_soap()` to call `compose_templated()` instead of `compose_with_kask()` (~10 lines)
4. Remove `compose_with_kask()` and `append_skill_knowledge()` dead code (~180 lines in `prompt.rs`)
5. Update integration test call chain (~15 lines)

---

## F3: Telemetry Pipeline Gap

**Root cause:** The telemetry fields on `RegistryEntry` (`probe_runs`, `recent_probe_failures`, `avg_probe_duration_ms`) were added to support the workshop REPL's skill stats view (`workshop.rs` commands like "stats" and "health"). They were never wired into the chat dispatch path because the chat handler (`chat/mod.rs` and `chat/execute.rs`) resolves ACTIONs via `action.rs:resolve_with_kask()` and dispatches via `dispatch.rs:Dispatcher::dispatch()` — but neither path calls `RegistryCache::record_execution()`. The telemetry is recorded by `workshop.rs` during workshop evaluation runs, not during operational skill use.

**Code evidence:**

```
Telemetry fields:
  crates/russell-skills/src/registry/mod.rs:73-91  (probe_runs, recent_probe_failures, etc.)

Recording (unwired from chat):
  crates/russell-skills/src/registry/mod.rs:279    record_execution() — wired only in workshop.rs
  crates/russell-skills/src/registry/mod.rs:295    record_intervention() — same

Chat dispatch (no telemetry recording):
  crates/russell-cli/src/commands/chat/mod.rs:275  handle_action_proposal()
  crates/russell-cli/src/commands/chat/execute.rs:178  execute_probe()
  crates/russell-cli/src/commands/chat/execute.rs:219  execute_intervention()

compute_quality_score (uncalled during normal operation):
  crates/russell-skills/src/registry/health.rs:80  compute_quality_score()
  crates/russell-skills/src/registry/mod.rs:339   RegistryCache::compute_score() — delegator
  Called only in: workshop.rs (during skill evaluation/manual scoring)
```

**Why `compute_quality_score()` is defined but uncalled during normal operation:** The quality score is a manifest-static heuristic (checks for `id:`, `version:`, probe count, etc.) that doesn't change between evaluations. It was designed for workshop discovery/evaluation — scoring a newly discovered manifest before installing it. It was not intended to run on every chat turn because its inputs (`manifest_content: &str`, `knowledge_exists: bool`) are loaded per-skill at evaluation time, not per-invocation. There's no scheduled re-scoring — `last_evaluated` is set once and never triggers re-evaluation.

**Resolution cost:** ~120 lines touched.

1. In `chat/execute.rs`, after `dispatch()` completes, call `RegistryCache::record_execution()` with the outcome (~20 lines)
2. In `chat/mod.rs`'s `handle_action_proposal()`, pass `&mut RegistryCache` through to the execute path (~15 lines)
3. Add `RegistryCache::with_update()` call after each ACTION execution in chat loop (~10 lines)
4. Wire `compute_quality_score()` into the staleness check timer — recompute when `last_evaluated` is older than 7 days (~40 lines)
5. Create a `russell skill evaluate` CLI verb for manual re-evaluation (~35 lines)

---

## F4: Knowledge Skills Lack Agency

**Root cause:** `SkillKind` was designed as a binary classifier for "does this skill have probes/interventions?" — a structural distinction, not a semantic one. When Lens-type skills were added (ADR-0024 workshop, skill-manager), they were given the same `Skill` struct but no probes or interventions. The `SkillKind` split doesn't capture that Lens skills have a different evaluation rubric (can't be scenario-tested), a different prompt integration path (no ACTION syntax, only knowledge injection), and a different staleness model (probe reliability is N/A — only author-date staleness applies).

**Code evidence:**

```
SkillKind enum:
  crates/russell-skills/src/lib.rs:158   Actionable (has probes + interventions)
  crates/russell-skills/src/lib.rs:165   Lens (KNOWLEDGE.md only)

Skill struct — shared for both:
  crates/russell-skills/src/lib.rs:175   probes: Vec<Probe> (empty for Lens)
  crates/russell-skills/src/lib.rs:183   interventions: Vec<Intervention> (empty for Lens)

Dispatch — Actionable only:
  crates/russell-skills/src/dispatch.rs:512   dispatch() — only works for Actionable skills

Knowledge injection — both kinds:
  crates/russell-meta/src/prompt.rs:386       actionable skills filtered for ACTION syntax
  crates/russell-meta/src/prompt.rs:398       lens skills filtered for knowledge injection
```

**Is `SkillKind` the right split?** No. The binary split addresses only one dimension: "can it be dispatched?" But skills differ across multiple axes:
- **Dispatchability**: can it execute probes/interventions? (Actionable = yes, Lens = no)
- **Evaluability**: can it be scenario-tested? (Actionable = yes, Lens = no for interventions, maybe for probes)
- **Staleness model**: how does it age? (Actionable = author-date + probe reliability; Lens = author-date only)
- **Knowledge scope**: what domain knowledge does it inject? (Lens = broad domain; Actionable = narrow symptom)
- **Installation gate**: what safety checks apply? (Actionable = full safety scanner; Lens = content-only scan)

A more useful split would be a capability bitmask or a set of agent trait flags rather than a binary enum.

**Resolution cost:** ~200 lines touched.

1. Replace `SkillKind` with a `#[bitflags] SkillCapability` type (~40 lines in `lib.rs`)
   - `CAN_PROBE`, `CAN_INTERVENE`, `HAS_KNOWLEDGE`, `CAN_BE_TESTED`
2. Add `SkillMeta` struct for knowledge-scope metadata (~30 lines)
3. Update `is_actionable()` / `is_lens()` to check flags rather than enum (~20 lines)
4. Add Lens-specific staleness check in `health.rs` (skip probe reliability dimension) (~30 lines)
5. Update `compose_templated()` to filter by capability flags (~20 lines)
6. Update workshop commands to present capability-aware UI (~60 lines)

---

## F5: No Remote Registry (ADR-0025 §8 Deferral)

**Root cause:** ADR-0025 §8 explicitly deferred remote skills: "Skills remain local-only. Distinction: local skills have Russell-enforced IDRS; Kask MCP tools have Kask-enforced governance." The deferral was pragmatic — getting the local dispatch pipeline solid before adding network dependency. But the current local-only stance doesn't serve Kask ecosystem goals because:

1. Russell has a skill editor (workshop), a skill lifecycle, and a skill health model — it's the ecosystem's most mature skill system. Kask has no comparable skill infrastructure.
2. Kask's MCP tools (193 across 16 servers) are infrastructure actions, not operator-facing playbooks. The skill model (playbook + knowledge + lifecycle) is a different abstraction layer.
3. Operator sharing (export/import between Russell instances) doesn't require a full registry — just a `SkillBundle` archive format.

**Minimum viable sharing primitive:** A `.rsk.tar.gz` archive (Task 8 `SkillBundle`) containing `manifest.yaml` + `KNOWLEDGE.md` + `scripts/` + `provenance.json`. This is the unit of human-to-human sharing. Registry federation (automated discovery from remote registries) is a separate concern that builds on the bundle format.

**Resolution cost (bundle format only):** ~200 lines touched. Remote registry federation is an additional ~300 lines (deferred). See Task 8 for full cost breakdown.

---

## F6: Workshop REPL as Management Bottleneck

**Root cause:** The workshop (`russell workshop`) was built as a standalone interactive REPL for skill lifecycle operations — a prompt → respond → prompt loop separate from `russell chat`. The `skill-manager` meta-skill partially bridges this by exposing skill CRUD as ACTION probes (e.g., `ACTION: skill-manager/list-skills`), but the workshop remains a separate surface. Two factors block full collapse:

1. **Different prompt modes**: Workshop uses `workshop.md.j2` (temperature=0.6, creative), while chat uses `chat_objective.md.j2` (temperature=0.4, conversational). Merging them requires a single prompt that can handle both skill creation AND operational queries.
2. **Different state contexts**: Workshop loads skill manifests as mutable workspace (`skill-manager/create-manifest` writes a `---manifest` block that the parser extracts). Chat loads skills as a read-only `Vec<Skill>` for dispatch. The chat loop would need to support manifest editing inline.

**What would collapse the two into a unified interaction model:**

1. Unify prompt modes: replace `workshop.md.j2` with extended `chat_objective.md.j2` that includes skill management capabilities when `skill-manager` skill is loaded
2. Make `---manifest` blocks a first-class chat parser concept, not workshop-only
3. Eliminate the workshop REPL entirely — "workshop" becomes a chat session with the `skill-manager` meta-skill loaded
4. `russell skill ...` CLI verbs remain as non-LLM fast paths for common operations (list, run, install, prune)

**Resolution cost:** ~350 lines touched.

1. Unify PromptMode::Workshop into PromptMode::Chat with skill-manager context (~30 lines in `prompt_registry.rs`)
2. Add `---manifest` block handling to chat parser (~50 lines in `chat/mod.rs`)
3. Make `skill-manager` interventions (create-manifest, build-skill, install-skill) work in chat context (~80 lines in `chat/execute.rs`)
4. Remove workshop.rs and workshop.rs CLI registration (~150 lines)
5. Update workshop.md.j2 template to be a chat context extension (~20 lines)
6. Update tests (~20 lines)

---

## Consolidated Technical Debt Quantification

| Friction | Lines Affected | Files Touched | Breaking API Change? |
|---|---|---|---|
| F1: Two-tier registry | ~180 | 5 | Yes — RegistryEntry split |
| F2: Dual prompt paths | ~250 | 3 | No — internal refactor |
| F3: Telemetry gap | ~120 | 3 | No — additive wiring |
| F4: Knowledge skill agency | ~200 | 4 | Yes — SkillKind replaced |
| F5: No remote registry (bundle) | ~200 | 3 | No — additive feature |
| F6: Workshop bottleneck | ~350 | 5 | Yes — workshop removed |
| **Total** | **~1,300** | **15+** | 3 breaking changes |

**Recommended execution order** (dependency chain):

```
F1 (registry split) → F3 (telemetry wiring) → F6 (workshop collapse)
                     ↘ F5 (bundle format)
                                     ↘ F2 (prompt unification)
F4 (skill capability) → F2 (prompt unification)
```
