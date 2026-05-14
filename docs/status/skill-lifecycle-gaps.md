---
title: "Skill Lifecycle Gap Analysis"
audience: [developers, architects]
last_updated: 2026-05-14
togaf_phase: "H"
version: "1.0.0"
status: "Active"
---

# Skill Lifecycle Gap Analysis — 2026-05-13

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
| `evaluate <name>` with manifest + KNOWLEDGE.md scan | Working | Scenario 4 |
| Safety scanner: manifest.yaml scanning | Working | Scenario 4 |
| Safety scanner: KNOWLEDGE.md scanning | Working | Scenario 9 |
| Safety scanner: prompt injection detection | Working | Scenario 9 |
| `check` audit with staleness + coverage | Working | Scenario 5 |
| Registry cache save on exit | Working | Scenario 6 |
| `prune` with "not found" error handling | Working | Scenario 7 |
| `install` with missing-skill rejection | Working | Scenario 10 |
| Coverage gaps by symptom category | Working | Scenario 8 |

## Remaining Gaps (ordered by impact)

### Gap 1: No `fetch` command — discovery→install loop is incomplete

**Symptom:** `search` finds a skill by name but there's no way to download it.
The workshop says "Use 'fetch <url>' to download it" but `fetch` isn't implemented.
A discovered skill can't be installed if it isn't already on disk.

**Fix:** Add `do_fetch(url, name)` that downloads a skill manifest from a URL,
writes it to a temp directory, runs the safety scanner, and registers it as
`discovered` → `evaluated` in the cache.

### Gap 2: `build` command registers skill but doesn't create files

**Symptom:** `build` adds a `RegistryEntry` with status `discovered` and
prompts Jack for composition, but no manifest.yaml, KNOWLEDGE.md, or probe
scripts are written to disk. The skill exists only in the cache. `install`
can't find it because the directory doesn't exist.

**Fix:** After Jack generates the manifest content (in the LLM response),
parse the YAML, validate against the symptom catalog, write to
`skills_dir/<name>/manifest.yaml`, and then the `install` command can find it.

### Gap 3: No `adapt` command — can't modify existing skills

**Symptom:** `adapt <name>` is in the help text but routes to the LLM with no
structured UI. There's no way to add/remove probes, change thresholds, update
symptoms, or add rollback strategies to an installed skill.

**Fix:** `do_adapt(name)` that loads the current manifest, writes it to a temp
file, opens an editor or prompts Jack for changes interactively, re-validates,
and replaces the manifest.

### Gap 4: No `undo` for `prune`

**Symptom:** `prune` moves a skill from active → deprecated but there's no
`restore` or `unprune` command. Operator mistake requires manual registry cache
editing.

**Fix:** `do_restore(name)` that moves deprecated → active. Add a new
lifecycle transition.

### Gap 5: Coverage scoring never computed

**Symptom:** `skill-maintenance/KNOWLEDGE.md` describes a 0.0–1.0 quality score
with 6 weighted factors, but the score is never calculated. `print_check`
doesn't show scores. `coverage_score` field in `RegistryEntry` is always `None`.

**Fix:** `compute_score(entry, manifest_content)` that checks manifest
completeness, probe coverage, intervention coverage, rollback quality, script
quality, and documentation presence. Display in `check` and `evaluate`.

### Gap 6: Remote registry sources defined but not wired

**Symptom:** `RegistrySources`, `RegistrySource`, and `RegistryKind` structs
are defined in `registry.rs` but no code reads `~/.config/harness/registry-sources.yaml`.
The `search` command only scans the local cache.

**Fix:** Load `registry-sources.yaml` on workshop startup. When `search` is
invoked, use the web-search MCP bridge (Brave Search / Firecrawl) to query
configured remote sources.

### Gap 7: Probe run telemetry never recorded

**Symptom:** `RegistryEntry` has `probe_runs` and `recent_probe_failures` fields
but they're initialized to 0 and never updated. No feedback loop from the
Sentinel's probe execution → registry cache.

**Fix:** When probes run (via Sentinel or `russell skill run`), update the
registry entry's counters. This enables quality scoring and staleness detection.

### Gap 8: Workshop doesn't validate symptom catalog

**Symptom:** When building a skill, Jack suggests symptoms but the workshop
code doesn't validate them against `russell_skills::SYMPTOMS`. A skill with
an unknown symptom would fail poka-yoke at load time but succeeds workshop
registration.

**Fix:** Before `do_build` or `do_install`, validate that all declared symptoms
are in `SYMPTOMS`. Reject or warn. Same check should run in `do_adapt`.

### Gap 9: No batch operations

**Symptom:** `check` audits all skills individually. `prune` requires one name
at a time. Can't `prune --all-stale` or `install --all-evaluated`.

**Fix:** Add `prune --stale`, `install --evaluated`, `check --scores` batch
flags.

### Gap 10: Workshop knowledge loaded from installed path (fragile)

**Symptom:** `load_knowledge` loads from `paths.skills()/skill-name/KNOWLEDGE.md`.
If the skill isn't installed, the knowledge is empty. During development, the
knowledge skills must be installed before the workshop works. Currently they
are installed (per `install.sh`), but if a user runs workshop before install,
the LLM gets no knowledge context.

**Fix:** Bundle workshop/maintenance knowledge into the binary as `include_str!`
constants (or fall back to installed path if not installed yet).

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
