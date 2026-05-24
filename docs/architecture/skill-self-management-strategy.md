---
title: "Skill Self-Management Strategy"
audience: [architects, developers]
last_updated: 2026-05-21
togaf_phase: "H"
version: "1.1.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Application Architecture -->
<!-- VERSION: 1.1.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-21 -->

# Skill Self-Management Strategy

> How Jack builds, loads, modifies, deletes, and measures skills
> — without the operator touching the workshop REPL.

## 1. Current State

### What works (2026-05-21)

| Capability | Surface | Automation |
|---|---|---|
| Run probes / interventions | ACP session — ACTION syntax | Jack proposes, probes auto-fire, interventions need consent |
| List skills | `russell skill list` | CLI + `skill-manager/list-skills` probe |
| Discover skills | `russell workshop` — `search --remote` | Operator-driven |
| Install / prune / adapt | `russell workshop` + `skill-manager` interventions | Both operator-driven and Jack-driven |
| Build skills | `russell skill build` + `skill-manager/build` | Both operator-driven and Jack-driven |
| Knowledge skills | KNOWLEDGE.md files loaded into Jack's context | Passive — just text |
| Command path validation | Dispatcher enforces `./scripts/` or absolute paths | Automatic at load time |

### What's missing (blocking self-management)

| Gap | Impact |
|---|---|
| **No telemetry feedback** | `probe_runs` and `recent_probe_failures` fields exist in `RegistryEntry` but are **never updated**. The dispatch path writes journal events but does not touch the registry cache. Jack has no way to know which skills are used or failing. |
| **No quality scoring** | `coverage_score` field exists but `compute_score()` is not implemented. The scoring rubric from `skill-maintenance/KNOWLEDGE.md` has no backing code. |
| **Registry only written on workshop exit** | Chat sessions don't read or write the registry cache. When Jack runs a probe in chat, the counter doesn't increment. |

## 2. Design Principles

1. **JR-2: Observe > Recommend > Act.** Telemetry first, then management actions. Jack must see usage data before he can make decisions.
2. **JR-4: Small but present.** A handful of new registry verbs, not a full rebuild. The existing workshop code is the foundation.
3. **The skill system manages skills.** A `skill-manager` skill wraps registry operations, not ad-hoc CLI commands. This keeps the model consistent: Jack runs skills, skills do things, the registry reflects the result.
4. **Auditable (JR-7).** Every registry mutation writes a `harness.event.v1` record. The registry cache remains rebuildable from journal + manifests.

## 3. Strategy — Three Tracks

### Track A: Telemetry Pipeline

**Goal:** Every probe and intervention execution updates the registry cache counters.

**How:**

1. **Add `metrics` fields to `RegistryEntry`** (in `russell-skills::registry`):

   ```rust
   // New fields in RegistryEntry:
   pub probe_runs: u64,              // exists, never updated
   pub recent_probe_failures: u64,   // exists, never updated
   pub last_probe_run_at: Option<String>,  // NEW: ISO 8601
   pub last_probe_error: Option<String>,   // NEW: error message
   pub intervention_runs: u64,       // NEW
   pub recent_intervention_failures: u64,  // NEW
   pub avg_probe_duration_ms: Option<f64>, // NEW: EWMA of run durations
   ```

2. **Add `record_probe_run()` and `record_intervention_run()` methods to `RegistryCache`:**

   ```rust
   impl RegistryCache {
       pub fn record_probe_run(&mut self, skill_id: &str, success: bool, duration_ms: u64);
       pub fn record_intervention_run(&mut self, skill_id: &str, success: bool);
   }
   ```

3. **Wire into the dispatch path.** In `chat.rs`, after every probe or intervention dispatch (both success and failure paths), call `registry.record_probe_run(skill_id, success, duration)`.

4. **Load registry on session start, save periodically.** Currently the ACP server loads the hKask tool registry but not the skill registry. Add skill registry load on startup and save on exit (or on a 5-minute periodic flush).

5. **Show metrics in Jack's context.** Add a "Skill Performance" table to the Objective section of Jack's SOAP prompt when the skill-maintenance knowledge skill is loaded:

   ```
   ## Skill Performance (past 24h)
   | skill | probes | failures | last run | health |
   |---|---|---|---|---|
   | sysadmin | 287 | 0 | 2 min ago | ✓ |
   | okapi-watcher | 287 | 3 | 2 min ago | ⚠ |
   | oom-watcher | 0 | 0 | never | (new) |
   ```

### Track B: Jack's Skill Management Capabilities

**Goal:** Jack can build, install, modify, prune, and restore skills from within the ACP session interface, using the ACTION syntax.

**Approach:** Create a bundled `skill-manager` skill with real probes and interventions.

#### The `skill-manager` manifest

**Implementation note (2026-05-21):** The `skill-manager` skill is installed and operational at `~/.local/share/harness/skills/skill-manager/`. All commands use wrapper scripts (`scripts/*.sh`) with the `RUSSELL_BIN` environment variable pattern to handle development builds not in PATH.

```yaml
id: skill-manager
version: 1.0.0
authored: 2026-05-14
symptoms:
  - skill_not_in_catalog
  - skill_version_stale
  - skill_install_failed
  - skill_probe_script_missing
  - skill_coverage_gap

probes:
  - id: list-skills
    cmd: ["bash", "./scripts/list-skills.sh"]
    risk: none
    timeout: 10s

  - id: check
    cmd: ["bash", "./scripts/check.sh"]
    risk: none
    timeout: 10s

  - id: stats
    cmd: ["bash", "./scripts/stats.sh"]
    risk: none
    timeout: 10s

interventions:
  - id: install
    cmd: ["bash", "./scripts/install.sh"]
    risk: low
    idempotent: true
    rollback: none_needed

  - id: build
    cmd: ["bash", "./scripts/build.sh"]
    risk: low
    idempotent: true
    rollback: none_needed

  - id: create-manifest
    cmd: ["bash", "./scripts/put.sh"]
    risk: low
    idempotent: true
    rollback: none_needed
    timeout: 15s

  - id: prune
    cmd: ["bash", "./scripts/prune.sh"]
    risk: low
    idempotent: true
    rollback_id: restore

  - id: restore
    cmd: ["bash", "./scripts/restore.sh"]
    risk: low
    idempotent: true
    rollback: none_needed

  - id: delete
    cmd: ["bash", "./scripts/retire.sh"]
    risk: medium
    idempotent: true
    rollback: none_needed
```

**Script pattern:** Each wrapper uses `RUSSELL_BIN` env var (defaults to dev build path):

```bash
#!/usr/bin/env bash
set -euo pipefail
RUSSELL_BIN="${RUSSELL_BIN:-/home/user/Clones/russell/target/debug/russell}"
"$RUSSELL_BIN" skill list
```

#### New CLI verbs required

**Status (2026-05-21):** All CLI verbs are implemented in `crates/russell-cli/src/commands/skill.rs`:

| Verb | Status | Purpose |
|---|---|---|
| `russell skill list` | ✓ Implemented | Lists loaded skills |
| `russell skill stats` | ✓ Implemented | Prints registry counters, last runs, health |
| `russell skill check` | ✓ Implemented | Prints staleness audit + coverage gaps |
| `russell skill install <name>` | ✓ Implemented | Installs or activates a skill (idempotent) |
| `russell skill prune <name>` | ✓ Implemented | Deprecates a skill |
| `russell skill restore <name>` | ✓ Implemented | Restores from deprecated → active |
| `russell skill retire <name>` | ✓ Implemented | Removes from disk + marks retired |
| `russell skill build <name>` | ✓ Implemented | Creates manifest skeleton on disk |
| `russell skill put <manifest>` | ✓ Implemented | Creates/updates skill manifest from YAML |

These verbs wrap the existing workshop code in `workshop.rs` (`do_install`, `do_prune`, `do_restore`, `do_build`, `print_check`) — no new logic, just a non-interactive CLI surface.

#### How Jack uses it in chat

```
Jack: Let me check the skill registry before I make a recommendation.
      ACTION: skill-manager/check-coverage

[probe result: skill-manager/check-coverage — 2 gaps, 0 stale]
[skill audit output...]

Jack: The swap-pressure symptom has no installed skill. I can build one.
     Want me to create a swap-watcher skill?

operator → yes

Jack: ACTION: skill-manager/build-skill --name swap-watcher

[intervention result: Created skills/swap-watcher/manifest.yaml]
[Operator approves]

Jack: I'll adapt it now to add the probe...
```

### Track C: Coverage Scoring

**Goal:** Compute and display 0.0–1.0 quality scores for installed skills.

**Algorithm** (from `skill-maintenance/KNOWLEDGE.md`):

| Factor | Weight | What it checks |
|---|---|---|
| Manifest completeness | 20% | id, version, authored, symptoms, applies_when — all required fields present |
| Probe coverage | 25% | At least one probe per declared symptom |
| Intervention coverage | 20% | At least one intervention per symptom |
| Rollback quality | 15% | Rollback strategies present, referenced IDs resolve |
| Script quality | 10% | Scripts exist, are executable, have acceptable exit codes |
| Documentation | 10% | KNOWLEDGE.md present, non-empty |

**Implementation:**

1. Add `pub fn compute_score(manifest_content: &str, skill_dir: &Path) -> f64` to `russell_skills::registry`.
2. Call it in `sync_registry_from_skills()` and store in `RegistryEntry.coverage_score`.
3. Display in `russell skill check` and `russell skill stats`.

## 4. Implementation Plan

### Phase A: Telemetry (2-3 hours) — DEFERRED

1. Extend `RegistryEntry` with new metric fields.
2. Add `record_probe_run()` / `record_intervention_run()` to `RegistryCache`.
3. Load `RegistryCache` on ACP session startup, wire dispatch path to record runs.
4. Save registry on chat exit and periodic flush.
5. Add `russell skill stats` CLI verb.

### Phase B: CLI verbs for management (1-2 hours) — ✓ COMPLETE (2026-05-21)

1. ✓ Add `skill install`, `skill prune`, `skill restore`, `skill retire`, `skill build`, `skill check` subcommands.
2. ✓ Each wraps existing workshop.rs functions in a non-interactive shell.
3. ✓ Added `skill put` for manifest creation/update.

### Phase C: skill-manager skill (1 hour) — ✓ COMPLETE (2026-05-21)

1. ✓ Created `skills/skill-manager/` with manifest.yaml and scripts.
2. ✓ Added to bundled skill set.
3. ✓ All commands use wrapper scripts with `RUSSELL_BIN` pattern for dev builds.

**Command path validation fix (2026-05-21):** The dispatcher enforces that all skill commands use either:
- Relative paths: `["bash", "./scripts/foo.sh"]`
- Absolute paths: `["/usr/bin/systemctl", "..."]`
- Allowed interpreters: `sh`, `bash`, `dash`, `python3`, `python`, `perl`, `ruby`

Bare command names like `["russell", "skill", "list"]` are rejected. The `skill-manager` skill was updated to use wrapper scripts.

### Phase D: Quality scoring (1 hour) — DEFERRED

1. Implement `compute_score()`.
2. Wire into registry sync and `skill check`.
3. Display scores in Jack's skill performance table.

### Phase E: Integration testing (1 hour) — PARTIAL

1. ✓ End-to-end skill creation works (`journal-viewer` skill built and installed).
2. ✓ `skill-manager` probes functional (`list-skills`, `check`, `stats`).
3. ◐ Registry counters not yet wired (Phase A dependency).

## 5. Registry Mutation Audit Trail

Every mutation to the registry cache writes a `harness.event.v1` record:

```
event_type: skill.lifecycle.transition
severity: info
payload:
  skill_id: "swap-watcher"
  from_status: "discovered"
  to_status: "installed"
  initiated_by: "jack" | "operator"
  session_id: "<chat session ulid>"
```

The registry cache (`local-cache.yaml`) is always rebuildable from:
1. Skill manifests on disk (`~/.local/share/harness/skills/`)
2. The journal events (`~/.local/state/harness/journal.db`)

This satisfies JR-7: persistence is auditable.

## 5.5. Command Path Validation (Lessons Learned)

**Issue (2026-05-21):** The original `skill-manager` manifest used bare command names:

```yaml
# REJECTED by dispatcher:
cmd: ["russell", "skill", "list"]
```

The dispatcher (`crates/russell-skills/src/dispatch.rs:validate_command_path`) enforces:
- ✓ Relative paths: `["bash", "./scripts/foo.sh"]`
- ✓ Absolute paths: `["/usr/bin/systemctl", "..."]`
- ✓ Allowed interpreters: `sh`, `bash`, `dash`, `python3`, `python`, `perl`, `ruby`
- ✗ Bare names: `["russell", "skill", "list"]` — PATH lookup rejected

**Resolution:** All `skill-manager` commands now use wrapper scripts in `scripts/` directory with the `RUSSELL_BIN` environment variable pattern:

```bash
#!/usr/bin/env bash
set -euo pipefail
RUSSELL_BIN="${RUSSELL_BIN:-/home/user/Clones/russell/target/debug/russell}"
"$RUSSELL_BIN" skill list
```

This pattern:
1. Satisfies dispatcher validation (uses `bash` interpreter + relative script path)
2. Handles development builds not in PATH
3. Allows runtime override via `RUSSELL_BIN=/custom/path/russell`

**Documentation:** See [`../standards/skill-building-rules.md`](../standards/skill-building-rules.md) for full skill-building guidance.

## 6. What This Does NOT Do (Out of Scope)

- **Auto-build:** Jack won't autonomously decide to create a skill. He can recommend it, but operator consent is required for interventions (per risk-band rules).
- **Remote skill push:** No mechanism to publish Jack-built skills to a remote registry. That's a future Phase 5 feature.
- **Skill signing:** No cryptographic verification of manifests. Only the safety scanner (prompt injection, pipe-to-shell, secret exfiltration checks).
- **Automated skill optimization:** Jack won't modify scripts inside installed skills. He can suggest changes; the operator adapts them.

## 7. Risk Register

| Risk | Severity | Mitigation |
|---|---|---|
| `prune`/`retire` removes the `skill-manager` skill itself | Medium | `skill-manager` is marked `max_auto_risk: none` for self-targeting operations. Jack's prompt explicitly forbids pruning bundled skills. |
| Registry corruption from concurrent writes | Low | Registry is single-writer (only chat CLI process touches it). Sentinel is read-only on skills. |
| Telemetry flooding (287 samples/day × many skills) | Low | Counters are integers, not arrays. The `RegistryEntry` struct grows by ~40 bytes total. |
| Operator confusion between workshop REPL and Jack's skill management | Low | Workshop remains available. Jack's approach is the "nurse doing it for you" — the REPL is the "DIY" path. |