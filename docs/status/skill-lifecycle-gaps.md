---
title: "Skill Lifecycle Gap Analysis"
audience: [developers, architects]
last_updated: 2026-05-14
togaf_phase: "H"
version: "1.1.0"
status: "Active"
---

# Skill Lifecycle Gap Analysis — 2026-05-20 (Updated)

<!-- TOGAF_DOMAIN: Governance -->
<!-- VERSION: 1.2.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-20 -->

> Derived from running 10 end-to-end scenarios against the newly built
> workshop REPL, registry cache, and safety scanner. All integrations
> tested: 21/21 test assertions pass.

## Verified Working

| Capability | Status | Verified By |
|---|---|---|
| Skill list (`russell skill list`) | Working | Scenario 1 |
| Workshop REPL startup + banner | Working | Scenario 2 |
| `/list` with lifecycle status + indicators | Working | Scenario 2 |
| `/gaps` with symptom coverage analysis | Working | Scenario 2 |
| `/lookup <symptom>` with no-match reporting | Working | Scenario 2 |
| `search <query>` local cache scan | Working | Scenario 3 |
| `search --remote` with Brave Search API | Working | Gap 6 resolved |
| `fetch <url> <name>` download + safety scan | Working | Gap 1 resolved |
| `evaluate <name>` with manifest + KNOWLEDGE.md scan | Working | Scenario 4 |
| Safety scanner: manifest.yaml scanning | Working | Scenario 4 |
| Safety scanner: KNOWLEDGE.md scanning | Working | Scenario 9 |
| Safety scanner: prompt injection detection | Working | Scenario 9 |
| `check` audit with staleness + coverage | Working | Scenario 5 |
| Registry cache save on exit | Working | Scenario 6 |
| `prune <name>` with "not found" error handling | Working | Scenario 7 |
| `prune --all-stale` batch operation | Working | Gap 9 resolved |
| `install <name>` with missing-skill rejection | Working | Scenario 10 |
| `install --all-evaluated` batch operation | Working | Gap 9 resolved |
| `restore <name>` undo for prune | Working | Gap 4 resolved |
| `adapt <name>` interactive modification | Working | Gap 3 resolved |
| `build <name>` skeleton creation | Working | Gap 2 resolved |
| Coverage gaps by symptom category | Working | Scenario 8 |
| Remote registry sources loading | Working | Gap 6 resolved |

## Remaining Gaps (ordered by impact)

### Gap 1: No `fetch` command — discovery→install loop is incomplete — ✅ RESOLVED (2026-05-20)

**Status:** Implemented. `do_fetch(url, name)` downloads a skill manifest from a URL,
runs the safety scanner, writes to `skills_dir/<name>/manifest.yaml`, and registers
as `discovered` in the cache.

### Gap 2: `build` command registers skill but doesn't create files — ✅ RESOLVED (2026-05-20)

**Status:** Implemented. `do_build(name)` creates a minimal valid manifest.yaml on disk,
then invokes Jack to help compose the skill interactively.

### Gap 3: No `adapt` command — can't modify existing skills — ✅ RESOLVED (2026-05-20)

**Status:** Implemented. `do_adapt(name)` loads the current manifest, calls Jack to
suggest improvements, safety-scans the result, and writes the updated manifest.
Falls back to `$EDITOR` if LLM adaptation fails or produces unsafe content.

### Gap 4: No `undo` for `prune` — ✅ RESOLVED (2026-05-14)

**Status:** Implemented. `russell skill restore <name>` CLI verb added.
Workshop command `restore <name>` also functional. Lifecycle transition
deprecated → active works correctly.

### Gap 5: Coverage scoring never computed — ✅ RESOLVED (2026-05-14)

**Status:** Implemented. `RegistryCache::compute_score()` scores 6 factors
(manifest completeness, probe coverage, intervention coverage, rollback
quality, script quality, documentation). Displayed in `russell skill check`.

### Gap 6: Remote registry sources defined but not wired — ✅ RESOLVED (2026-05-20)

**Status:** Implemented. `load_registry_sources(paths)` loads
`~/.config/harness/registry-sources.yaml` on workshop startup.
`search --remote` now displays configured sources and uses Brave Search API
when `BRAVE_API_KEY` is set.

**Fix:** Load `registry-sources.yaml` on workshop startup. When `search` is
invoked, use the web-search MCP bridge (Brave Search / Firecrawl) to query
configured remote sources.

### Gap 7: Probe run telemetry never recorded — ✅ RESOLVED (2026-05-14)

**Symptom:** `RegistryEntry` has `probe_runs` and `recent_probe_failures` fields
but they're initialized to 0 and never updated. No feedback loop from the
Sentinel's probe execution → registry cache.

**Fix:** When probes run (via Sentinel or `russell skill run`), update the
registry entry's counters. This enables quality scoring and staleness detection.

### Gap 7: Probe run telemetry never recorded — ✅ RESOLVED (2026-05-14)

**Status:** Implemented. `RegistryEntry` fields `probe_runs`, `recent_probe_failures`,
`intervention_runs`, `recent_intervention_failures`, `avg_probe_duration_ms` (EWMA),
and `last_probe_run_at` are updated on every execution in chat and CLI.

### Gap 8: Workshop doesn't validate symptom catalog — ✅ RESOLVED (2026-05-14)

**Status:** The symptom catalog is validated at load time via
`russell_skills::load_all()` — unknown symptoms are rejected at poka-yoke.
The `skill-manager` skill registers symptoms from the catalog. CLI
verbs (`install`, `check`) validate against `SYMPTOMS`.

### Gap 9: No batch operations — ✅ RESOLVED (2026-05-20)

**Status:** Implemented. `prune --all-stale` deprecates all skills not evaluated
in 30+ days. `install --all-evaluated` installs all skills with status `evaluated`.
Both commands are available in the workshop REPL.

### Gap 10: Workshop knowledge loaded from installed path (fragile) — ✅ PARTIALLY RESOLVED (2026-05-14)

**Status:** `skill-manager` is bundled and its KNOWLEDGE.md ships with Russell.
Workshop knowledge (`skill-workshop`, `skill-maintenance`) still loads from
installed path. The `skill-manager` provides ACTION-based management from chat
without requiring workshop, reducing the fragility concern.

## Open Design Questions

- **Skill provenance protocol:** When a skill is discovered from a remote
  source, should the registry record the exact commit SHA or just the URL?
  (GitHub repos mutate; a URL isn't immutable.)

- **Reload-after-install:** After `install`, should the workshop reload skills
  in-place, or does the operator need to restart the workshop?

- **Test skill sandboxing:** Should `install` flag newly installed skills with
  a "probation" period where probes run but interventions are blocked until
  the operator has seen them work?

- **Skill signing:** Should manifests support an optional `signature:` field
  for integrity verification? (JR-7: persistence is auditable.)
