---
title: "MVP Russell — the Minimal Viable Terrier"
audience: [operators, developers, architects, contributors, agents]
last_updated: 2026-04-18
togaf_phase: "Requirements Management"
version: "1.0.0"
status: "Active"
---

# MVP Russell — the Minimal Viable Terrier

<!-- TOGAF_DOMAIN: Requirements Management -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-04-18 -->

This is the **pinned boundary** of MVP Russell. Anything outside
this boundary is explicitly deferred; adding anything inside it
requires updating this document.

The principle that pins it is JR-1 (see
[`../architecture/PRINCIPLES_CATALOG.md`](../architecture/PRINCIPLES_CATALOG.md)):
*Though she be but little, she is fierce.*

## 1. The One-Paragraph Russell

MVP Russell is a single Rust binary, `russell`, run by one user
under user-scoped systemd on Ubuntu 25.10. He **observes** the
host every five minutes via a small probe set, **remembers** what
he saw in a SQLite journal, **reports** what he saw through four
read-only CLI verbs, **watches himself** through one self-vital,
and when asked he **cries for help** to a frontier
zero-data-retention LLM via OpenRouter. He does not mutate host
state, he does not run privileged operations, he does not dispatch
interventions, and he does not load skills. He is small and he is
fierce.

## 2. The Six Verbs

The MVP CLI exposes exactly six verbs. No more.

| Verb | Risk | Role | Reach |
|---|---|---|---|
| `russell status` | none | Read-only summary of paths, kill-switch, journal, profile | local fs |
| `russell list` | none | Most-recent journal events | local sqlite |
| `russell profile [--init]` | none | Show / initialise `profile.json` | local fs |
| `russell digest [--since-hours N]` | none | Markdown summary of recent activity | local sqlite |
| `russell sentinel-once` | none | Fire the Sentinel once and append samples | local fs |
| `russell jack [--note "..."]` | none | Compose SOAP-shaped prompt and consult the LLM; print response | network *(opt-in)* |

All six are read-only with respect to host state. The only thing
any of them writes is Russell's own journal and evidence bundles
under `~/.local/state/harness/`.

### 2.1 The `jack` verb — the "cry for help"

Under JR-4, Russell must be able to escalate from day one.

1. Gather the last 24h of Sentinel samples + severity counts
   + last 20 events.
2. Compose a SOAP-shaped prompt
   ([`../templates/soap-bundle.md`](../templates/soap-bundle.md))
   where Subjective is the operator's `--note`, Objective is the
   gathered evidence, and Assessment / Plan are left empty for
   the LLM to fill.
3. Submit via the copied `stack-llm` router
   ([`../operations/REUSE_MANIFEST.md`](../operations/REUSE_MANIFEST.md))
   to OpenRouter with the `zdr: true` parameter, targeting the
   frontier open-weight model (default:
   `moonshotai/kimi-k2.5`).
4. Write the full request / response / latency / model to
   the `help_sessions` table and an evidence bundle at
   `~/.local/state/harness/evidence/help/<session-id>/`.
5. Print the model's response, plain text, to stdout.

**Offline fallback.** If `OPENROUTER_API_KEY` is unset or the
request fails, Jack still speaks: a rule-based summary of severity
counts, most-recent events, and proprioception state is printed.
Jack is never silent.

**What `russell jack` does NOT do:**

- Parse the LLM's output for commands to execute.
- Mutate any file outside `~/.local/state/harness/`.
- Invoke any privileged operation.
- Retry on user's behalf; one call, one response, journaled.

## 3. The Observation Loop

A single 5-minute Sentinel cadence, driven by a user-scope
systemd timer (`russell-sentinel.timer`) in the installed form.
For MVP-dev, `russell sentinel-once` is the manual trigger.

**Probe set (MVP).** Deliberately tiny. Grows in Phase 2.

| Probe | Source | Unit |
|---|---|---|
| `mem_available_mib` | `/proc/meminfo` | MiB |
| `swap_used_mib` | `/proc/meminfo` | MiB |
| `loadavg_1m` | `/proc/loadavg` | — |

**Self-vital (MVP).** One, per JR-5:

| Self-vital | Source | Rule |
|---|---|---|
| `sentinel_last_run_age_s` | journal `MAX(ts)` on samples | Warn if > 450s (1.5× cadence); Alert if > 1800s |

## 4. Persistence Scope

Every byte Russell writes is named. Full catalog at
[`PERSISTENCE_CATALOG.md`](PERSISTENCE_CATALOG.md).

| Path | Owner | Schema | Retention |
|---|---|---|---|
| `~/.local/state/harness/journal.db` | `russell-core::journal` | `0001_init.sql` | unbounded (digest prunes later) |
| `~/.local/state/harness/journal.db-{wal,shm}` | SQLite | — | ephemeral |
| `~/.local/state/harness/profile.json` | `russell-core::profile` | `russell.profile.v1` | unbounded |
| `~/.local/state/harness/evidence/help/<session-id>/` | `russell-doctor::help` | per-session JSON | 90 days |
| `~/.local/state/harness/runs/<run-id>.json` | (reserved, unused in MVP) | per-run JSON | 90 days |
| `~/.config/harness/russell.env` | operator | key=value | operator-owned |
| `~/.config/harness/disable` | kill switch | empty file | operator-owned |

`rm -rf ~/.local/state/harness/` is an always-safe full reset.
`rm -rf ~/.config/harness/` is the operator's intentional reset
of their own configuration. Neither produces orphans.

## 5. Boundaries — what MVP does NOT do

All of these are deferred, with their ADRs living under
[`../adr/deferred/`](../adr/deferred/) where applicable:

- **No MCP server.** The `russell mcp` subcommand does not
  exist. (Deferred ADR-0003.)
- **No skill dispatcher.** The `skills/` directory is empty.
  (Deferred ADR-0007.)
- **No privileged operations.** No PolKit helpers, no sudo.
  (Deferred ADR-0005.)
- **No auto-mutation.** Even low-risk mutations (trash-empty,
  thumbnail prune) are deferred to Phase 2.
- **No LLM-driven intervention.** `russell help` calls the LLM
  for a *summary*, not a *plan-to-execute*.
- **No remote skill registry.** (Deferred part of ADR-0007.)
- **No tiered Tier I / II / III cadences.** One Sentinel
  cadence; the tier engines land in Phase 2+.
- **No chaos probe.** Deferred.
- **No rules engine.** The one self-vital rule is hard-coded.
- **No EWMA baselines.** Computed in Phase 2.

## 6. Success Criteria

MVP is **complete** when all three of these are empirically true:

1. **Stability:** Russell runs unattended on the observed
   Framework 16 / HX 370 / Ubuntu 25.10 machine
   ([`MACHINE_PROFILE.md`](../../MACHINE_PROFILE.md)) for **30
   consecutive days** with zero mystery gaps in the journal
   (gaps > 2× the cadence).
2. **Tests:** `cargo fmt --check`, `cargo clippy --workspace
   --all-targets -- -D warnings`, and `cargo test --workspace`
   all pass on every commit.
3. **Help channel proof:** At least **10 successful
   `russell help` round-trips** are journaled during the
   30-day window, and at least one was triggered in a real
   moment of operator uncertainty about the machine's state.

When all three are met, MVP is closed and Phase 2 opens. Not
before.

## 7. Status and Next Step

Phase 0 (skeleton, read-only observation loop minus `help`) is
**complete** as of 2026-04-18. See
[`../status/CONSOLIDATED-STATUS.md`](../status/CONSOLIDATED-STATUS.md)
for current state. The next concrete milestone is Phase 1:
implement `russell help` by copying `stack-llm` per JR-6 and
landing the persona, the env loader, the `help_sessions` table,
and the offline fallback.

## 8. References

- [`../architecture/PRINCIPLES_CATALOG.md`](../architecture/PRINCIPLES_CATALOG.md) — JR-1 through JR-7.
- [`../architecture/overview.md`](../architecture/overview.md) — system shape.
- [`../../cybernetic-health-harness.md`](../../cybernetic-health-harness.md) — full design (the aspirational target).
- [`../../MACHINE_PROFILE.md`](../../MACHINE_PROFILE.md) — the patient.
- [`../operations/REUSE_MANIFEST.md`](../operations/REUSE_MANIFEST.md) — upstream copy register.
