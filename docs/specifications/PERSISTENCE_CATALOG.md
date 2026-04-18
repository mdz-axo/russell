---
title: "Persistence Catalog — where Russell remembers"
audience: [operators, developers, architects]
last_updated: 2026-04-18
togaf_phase: "C — Data Architecture"
version: "1.0.0"
status: "Active"
---

# Persistence Catalog

<!-- TOGAF_DOMAIN: Data Architecture -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-04-18 -->

Under JR-7 (persistence is auditable), every byte Russell writes
is registered here. If you add a new persistent artifact, you
update this file **in the same commit**. Unregistered state is a
review-blocker.

## 1. Summary

Russell uses **one SQLite database** for structured state, **one
JSON file** for the machine profile, and **one directory** for
evidence bundles. Plus the operator's own env file and optional
kill switch. That is the entire persistence surface.

```mermaid
flowchart LR
    subgraph STATE["~/.local/state/harness/ (Russell-owned)"]
        DB[(journal.db<br/>samples, events,<br/>baselines, confirmations,<br/>help_sessions)]
        PROF[profile.json]
        EVID[evidence/help/&lt;id&gt;/]
        RUNS[runs/ &nbsp;<em>(reserved)</em>]
    end
    subgraph CONFIG["~/.config/harness/ (operator-owned)"]
        ENV[russell.env]
        KILL[disable]
        RULES[rules.d/ &nbsp;<em>(reserved)</em>]
    end
    subgraph DATA["~/.local/share/harness/ (reserved)"]
        SKILLS[skills/ &nbsp;<em>(empty in MVP)</em>]
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
| `samples` | Time-series probe observations | Sentinel, Meta-Sentinel | digest, baselines |
| `events` | Structured log rows conforming to `harness.event.v1` | every mutating + observational action | `list`, `digest`, `help` |
| `baselines` | EWMA mean/var + p50/p95/p99 per probe | (reserved, unused in MVP) | Phase 2 rules |
| `confirmations` | Andon-cord records for risk≥medium actions | (reserved, unused in MVP) | Phase 3 dispatcher |
| `help_sessions` | One row per `russell help` round-trip | `russell-doctor::help` | `digest`, future UI |

Column details live in
`crates/russell-core/src/journal/migrations/0001_init.sql` and any
follow-up numbered migrations.

The `help_sessions` table is introduced by the Phase-1 migration
`0002_help_sessions.sql`; it is **not** in the current 0001 file
and must land before `russell help` ships.

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

**Owner.** `russell-doctor::help` (Phase 1+).
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
| `RUSSELL_DOCTOR_MODEL` | Override default model ID | yes |
| `RUSSELL_DOCTOR_BACKEND` | `openrouter` \| `ollama` \| `mock` | yes (default `openrouter` if key present, else `mock`) |
| `RUSSELL_LOG` | Tracing filter | yes |

File is created by the operator. Russell does **not** write to it.

### 2.6 `~/.config/harness/disable` (kill switch)

**Owner.** The operator.
**Format.** An empty file. Presence = kill-switch engaged.
**Effect.** Every Russell timer becomes a no-op on next trigger
(post-MVP; in MVP `russell status` surfaces the state).

### 2.7 `~/.config/harness/rules.d/` (reserved)

Empty in MVP. Phase 2 rule overrides will land here.

### 2.8 `~/.local/share/harness/skills/` (reserved)

Empty in MVP. Skill manifests will land here in Phase 3.

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

OpenRouter calls route only to providers with a zero-data-retention
policy (per-request `zdr: true` parameter) unless the operator
explicitly disables that. See
[ADR-0016 *(to be authored in Phase 1)*](../adr/README.md)
*(to be authored in Phase 1)* for the privacy contract.

## 7. Changing the Catalog

Adding, removing, or changing the schema of any persistent
artifact requires, in the same commit:

1. Update this catalog §2.
2. Add or adjust the relevant migration under
   `crates/russell-core/src/journal/migrations/`.
3. Run the migration idempotence test
   (`cargo test -p russell-core journal::migrations`).
4. Cite the ADR that mandated the change.
