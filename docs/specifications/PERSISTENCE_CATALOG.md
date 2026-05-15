---
title: "Persistence Catalog — where Russell remembers"
audience: [operators, developers, architects]
last_updated: 2026-05-14
togaf_phase: "C"
version: "1.1.0"
status: "Active"
---

# Persistence Catalog

<!-- TOGAF_DOMAIN: Data Architecture -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-06 -->

Under JR-7 (persistence is auditable), every byte Russell writes
is registered here. If you add a new persistent artifact, you
update this file **in the same commit**. Unregistered state is a
review-blocker.

## 1. Summary

Russell uses **one SQLite database** for structured state, **one
JSON file** for the machine profile, **one directory** for
evidence bundles, and a **Markdown memory layer** for human-readable
exports derived from the journal. Plus the operator's own env file,
optional identity files, and optional kill switch.

```mermaid
flowchart LR
    subgraph STATE["~/.local/state/harness/ (Russell-owned)"]
        DB[(journal.db<br/>samples, events,<br/>baselines, confirmations,<br/>help_sessions)]
        PROF[profile.json]
        EVID[evidence/help/&lt;id&gt;/]
        RUNS[runs/ &nbsp;<em>(reserved)</em>]
        MEM[memory/<br/>REVIEW.md + daily/]
    end
    subgraph CONFIG["~/.config/harness/ (operator-owned)"]
        ENV[russell.env]
        KILL[disable]
        RULES[rules.d/]
        ID[PERSONA.md<br/>USER.md]
    end
    subgraph DATA["~/.local/share/harness/ (reserved)"]
        SKILLS[skills/]
    end
```

<!-- DIAGRAM_ALIGNMENT
id: DIAG-PERSIST-TOPO-001
type: flowchart
verified_date: 2026-04-18
verified_against: crates/russell-core/src/paths.rs, docs/specifications/MVP_SPEC.md §4
reference_sources: XDG Base Directory Specification
status: VERIFIED
-->

## 2. Files and Directories

### 2.1 `~/.local/state/harness/journal.db`

**Owner.** `russell-core::journal::JournalWriter` (single writer).
**Access.** `russell-core::journal::JournalReader` (many readers).
**Schema version.** Tracked in `schema_migrations` table. Current
version: **1** (file `crates/russell-core/src/journal/migrations/0001_init.sql`).
**Mode.** WAL (`PRAGMA journal_mode=WAL`,
`synchronous=NORMAL`) per ADR-0004.
**Retention.** Unbounded in MVP. Phase 2 introduces a digest-driven
compaction skill; until then the operator may run SQL manually
against the file.

#### Tables

| Table | Purpose | Authored by | Read by |
|---|---|---|---|
| `schema_migrations` | Forward-only migration log | `journal::migrations::apply_one` | migration runner |
| `samples` | Time-series probe observations | Sentinel, Meta-Sentinel, Proprio | digest, baselines, `arsenal-mcp-russell` |
| `events` | Structured log rows conforming to `harness.event.v1` | every mutating + observational action | `list`, `digest`, `help` |
| `baselines` | EWMA mean/var + p50/p95/p99 per probe | `journal::compute_baselines` (daily refresh in sentinel-once) | `read_baselines`, rules, SOAP prompt |
| `confirmations` | Consent-flow approval records for risk≥medium interventions | `russell-cli::chat` consent path | Phase 3 dispatcher audit |
| `help_sessions` | One row per `russell jack` round-trip | `russell-meta::help` | `digest`, future UI |

Column details are in `crates/russell-core/src/journal/migrations/`.

**Event action types (new additions).**

| Action | Meaning | Emitted by |
|---|---|---|
| `rate_breach` | Rate-of-change threshold crossed | `sentinel::evaluate_samples_with_rates` |
| `reflex_proposed` | Reflex arc matched a breach → intervention proposed | `sentinel-once` reflex evaluation |

**Baselines columns (new additions).**

The `baselines` table now stores per-probe EWMA statistics
alongside percentiles:

| Column | Type | Meaning |
|---|---|---|
| `ewma_mean` | REAL | Exponentially-weighted moving average, 7-day half-life |
| `ewma_var` | REAL | EWMA variance around the mean |

Both are `NULL` until the first `compute_baselines()` run for a
probe; computed once every 24h in `sentinel-once`. Surfacen in
Jack's SOAP sample table as `ewma (7d)` column.

#### Self-scope samples (`russell-proprio`)

The `samples` table also holds self-observation rows written by
`russell-proprio`. These are distinguished by `scope='self'`:

| Column | Value |
|---|---|
| `scope` | `'self'` |
| `probe` | `sentinel_last_run_age_s` |
| `value` | Seconds since the most recent Sentinel host sample |
| `severity` | `ok` (≤360s) or `warning` (>360s) |

**Owner.** `russell-proprio` (writes one row per cycle, before
host probes run).
**Retention.** Same as host samples — unbounded until Phase-2
digest compaction prunes.

### 2.2 `~/.local/state/harness/profile.json`

**Owner.** `russell-core::profile::Profile`.
**Author.** `russell profile --init` (MVP) → Bootstrap state
machine (post-MVP).
**Schema.** `russell.profile.v1` per ADR-0006. Schema tag is in
the file; unknown versions are refused at load.
**Atomicity.** Written via `tmp` → `fsync` → `rename`.
**Retention.** Unbounded; re-run of `profile --init` on a fresh
file is idempotent.

### 2.3 `~/.local/state/harness/evidence/help/<session-id>/`

**Owner.** `russell-meta::help` (Phase 1+).
**Contents.**

- `soap.md` — the rendered SOAP-shaped prompt sent to the LLM.
- `request.json` — full outbound request (provider, model, params,
  `zdr: true`).
- `response.json` — full inbound response, including latency and
  token counts.
- `transcript.jsonl` — request + response + metadata in
  `harness.llm-transcript.v1` format (one row per round-trip;
  in MVP there is exactly one).

**Session ID.** ULID prefix + `-help-<short>` slug, matching the
pattern used for other evidence bundles
(`docs/templates/soap-bundle.md`).
**Retention.** 90 days, evicted by a reaper (reserved — not
implemented in MVP; manual cleanup until Phase 2).

### 2.4 `~/.local/state/harness/runs/` (reserved)

Unused in MVP; hosts per-module JSON records once the tier
engines land in Phase 2.

### 2.5 `~/.config/harness/russell.env`

**Owner.** The operator.
**Format.** `KEY=value` lines, `#` comments.
**Required keys.**

| Key | Purpose | Optional |
|---|---|---|
| `OPENROUTER_API_KEY` | Authenticates the Doctor's LLM calls | yes (fallback kicks in if absent) |
| `RUSSELL_DOCTOR_MODEL` | Override default model ID | yes (default `nemotron-3-super:cloud`) |
| `RUSSELL_DOCTOR_BACKEND` | `okapi` \| `openrouter` \| `mock` \| `offline` | yes (default `okapi`) |
| `RUSSELL_LOG` | Tracing filter | yes |

File is created by the operator. Russell does **not** write to it.

### 2.6 `~/.config/harness/disable` (kill switch)

**Owner.** The operator.
**Format.** An empty file. Presence = kill-switch engaged.
**Effect.** Every Russell timer becomes a no-op on next trigger
(post-MVP; in MVP `russell status` surfaces the state).

### 2.7 `~/.config/harness/rules.d/`

Operator rule overrides. Default rules ship in
`crates/russell-sentinel/rules.d/` and are copied at install
time; operator overrides in this directory take precedence.

### 2.7a `~/.config/harness/reflex.d/`

Operator reflex arc overrides. Default arcs ship in
`crates/russell-core/src/reflex/defaults.toml` and are compiled
into the binary; operator overrides take precedence. Each TOML
file conforms to `russell.reflex.v1` schema:
```toml
schema = "russell.reflex.v1"
[[arc]]
probe = "disk_root_used_pct"
min_severity = "alert"
intervention = "sysadmin/sweep-caches"
cooldown_secs = 3600
max_retries = 3
```
Evaluation: `sentinel-once` checks each threshold/rate breach
against the reflex set and emits `reflex_proposed` events.

### 2.8 `~/.local/share/harness/skills/`

Operator-owned skill manifests. The `okapi-watcher` skill
ships as a reference implementation.

### 2.9 `~/.local/state/harness/memory/` (Russell-owned Markdown memory)

**Owner.** `russell-cli::commands::reflect` (planned verb) and
`russell-meta::help::run_help` (session log appends).
**Contents.**

- `REVIEW.md` — Russell's self-assessment review surface.
  Structured entries with confidence scores, evidence links,
  and type tags (`W`=world fact, `B`=biographical,
  `O`=opinion). Human-reviewed before observations graduate
  to durable memory. See
  [`../templates/review-entry.md`](../templates/review-entry.md).
- `daily/YYYY-MM-DD.md` — Daily logs with a `## Retain`
  section containing 2–5 durable observations tagged `[W]`,
  `[B]`, or `[O](c=N)`. Rebuildable from journal via
  `russell digest --format markdown`. See
  [`../templates/daily-log.md`](../templates/daily-log.md).

**Design contract.** All Markdown files in this tree are
**derived exports** from the journal. The journal is the sole
canonical store per C-2 and JR-7. Markdown files can be
regenerated from the journal at any time without data loss.

**Retention.** Unbounded (daily logs); REVIEW.md is
append-only with human trimming.

### 2.10 `~/.config/harness/PERSONA.md` (operator-owned)

**Owner.** The operator.
**Content.** Runtime persona customisation for Jack. If present,
the Doctor appends its content to the compiled-in
`JACK_PERSONA` from `crates/russell-meta/prompts/jack.md`.
**Format.** Free-form Markdown. Not parsed by Russell; read by
the Doctor at help-session startup. Russell never writes to
this file.
**Design reference.** [`../architecture/THE_JACK.md`](../architecture/THE_JACK.md).

### 2.11 `~/.config/harness/USER.md` (operator-owned)

**Owner.** The operator.
**Content.** Operator profile: timezone, communication style
preferences, what counts as "urgent", preferred cadence for
proactive notifications.
**Format.** Free-form Markdown. Not parsed; included in the
Doctor's system context if present. Russell never writes to
this file.

## 3. Reset Procedures

| What | How | Consequence |
|---|---|---|
| Full reset | `rm -rf ~/.local/state/harness/` | Russell loses all memory and history. Profile re-init needed. Safe. |
| Configuration reset | `rm -rf ~/.config/harness/` | Operator loses env vars, kill-switch, rule overrides. Russell keeps its memory. Safe. |
| Journal rollover | `mv journal.db{,.bak}` | Fresh journal; old data archived. Safe. |
| Profile regen | `rm profile.json && russell profile --init` | New profile_id; honeymoon clock resets when Bootstrap lands. Safe. |

No reset procedure involves manual SQLite `DELETE` statements.
If it did, we would document them here.

## 4. Retention Policies (summary)

| Artifact | Retention | Reaper |
|---|---|---|
| `samples` rows | unbounded (MVP) | Phase-2 digest compaction |
| `events` rows | unbounded (MVP) | Phase-2 digest compaction |
| `help_sessions` rows | unbounded (MVP); 90 days (Phase 2) | future |
| Memory Markdown (`memory/`) | rebuildable (derived) | None needed — regenerated from journal |
| Evidence bundles | 90 days nominally, manual in MVP | Phase-2 reaper |
| WAL / SHM files | ephemeral | SQLite (automatic) |

## 5. Backup and Export

Russell does not back itself up. The operator is responsible.
Recommended approach:

```sh
sqlite3 ~/.local/state/harness/journal.db ".backup /path/to/backup/journal-$(date +%F).db"
cp ~/.local/state/harness/profile.json /path/to/backup/
```

Export / import formats for the journal are not defined in MVP.
Users who want CSV or JSON exports should query the journal
directly; a `russell export` verb is out of MVP scope.

## 6. Privacy

The journal contains **host metrics** (memory, swap, loadavg) and
**Russell's own event log**. It does **not** contain passwords,
user file contents, or network payload. The evidence bundles
under `evidence/help/` contain the prompts Russell sent to the
LLM and the LLM's responses; those prompts include the samples
and events above, and any `--note` text the operator provided.

Ollama calls stay entirely local. OpenRouter calls route only
to providers with a zero-data-retention policy (per-request
`zdr: true` parameter). See
[ADR-0016](../adr/0016-doctor-and-llm-router.md) for the full contract.

## 7. Changing the Catalog

Adding, removing, or changing the schema of any persistent
artifact requires, in the same commit:

1. Update this catalog §2.
2. Add or adjust the relevant migration under
   `crates/russell-core/src/journal/migrations/`.
3. Run the migration idempotence test
   (`cargo test -p russell-core journal::migrations`).
4. Cite the ADR that mandated the change.
