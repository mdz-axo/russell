---
title: "MVP Russell — the Minimal Viable Terrier"
audience: [operators, developers, architects, contributors, agents]
last_updated: 2026-05-11
togaf_phase: "Requirements Management"
version: "1.1.0"
status: "Active"
---

# MVP Russell — the Minimal Viable Terrier

<!-- TOGAF_DOMAIN: Requirements Management -->
<!-- VERSION: 1.1.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-11 -->

This is the **pinned boundary** of MVP Russell. Anything outside
this boundary requires an ADR or a spec update to this document.

The principle that pins it is JR-1 (see
[`../architecture/PRINCIPLES_CATALOG.md`](../architecture/PRINCIPLES_CATALOG.md)):
*Though she be but little, she is fierce.*

## 1. The One-Paragraph Russell

MVP Russell is a single Rust binary, `russell`, run by one user
under user-scoped systemd on Ubuntu 25.10. He **observes** the
host every five minutes via a broad probe set covering
CPU, memory, processes, GPU, disks, and systemd, **remembers**
what he saw in a SQLite journal, **reports** through CLI verbs,
**watches himself** through five self-vitals (proprioception),
and when asked he **cries for help** to a local LLM
(Ollama by default, DeepSeek V4 Pro). He **proposes**
interventions via loaded skills, and with the operator's
explicit consent, he **executes** them through the IDRS-gated
skill dispatcher. He is small and he is fierce.

## 2. The Verbs

The MVP CLI exposes these verbs:

| Verb | Risk | Role | Reach |
|---|---|---|---|
| `russell status` | none | Read-only summary of paths, kill-switch, journal, profile | local fs |
| `russell list` | none | Most-recent journal events | local sqlite |
| `russell profile [--init]` | none | Show / initialise `profile.json` | local fs |
| `russell digest [--since-hours N]` | none | Markdown summary of recent activity | local sqlite |
| `russell sentinel-once` | none | Fire the Sentinel once, append samples, evaluate rules | local fs |
| `russell jack [--note "..."]` | none | Compose SOAP-shaped prompt and consult the LLM; print response | network *(opt-in)* |
| `russell chat` | none | Interactive multi-turn conversation with Jack, consent flow for interventions | network *(opt-in)* |
| `russell skill list` | none | List loaded skills, probes, and interventions | local fs |
| `russell skill run <id>` | varies | Execute a skill probe or intervention via the IDRS-gated dispatcher | local fs + process |
| `russell okapi-probe` | none | Probe the Okapi inference engine metrics endpoint | network |
| `russell proprio` | none | Run self-observation and append self-vital samples | local sqlite |

### 2.1 The `jack` verb — the "cry for help"

Under JR-4, Russell escalates from day one.

1. Gather the last 24h of Sentinel samples + severity counts
   + last 20 events + 30-day baselines.
2. Compose a SOAP-shaped prompt where Subjective is the
   operator's `--note`, Objective is the gathered evidence plus
   baseline deviation data, and Assessment / Plan are left empty.
3. Submit via the LLM router to the configured backend.
4. Write the full request/response/latency/model to the
   `help_sessions` table and an evidence bundle.
5. Print the model's response.

If Jack proposes an intervention (`ACTION: <skill>/<id>`), the
response is parsed and the operator is shown guidance on how to
execute it via `russell chat` or `russell skill run`.

**Offline fallback.** If the LLM is unreachable, a rule-based
summary of severity counts and most-recent events is printed.

### 2.2 The `chat` verb — interactive consent flow

`russell chat` is the canonical path for operator consent to
interventions. Jack proposes actions; the operator types
`/approve` or `/deny`. The skill dispatcher executes approved
interventions with full IDRS journaling. Risk enforcement gates
on `max_auto_risk` (default: Low). Sudo-requiring interventions
require NOPASSWD configuration by the operator.

## 3. The Observation Loop

A single 5-minute Sentinel cadence, driven by systemd timer
(`russell-sentinel.timer`). `russell sentinel-once` is the
manual trigger for development.

**Probe set (current).** The original 3-probe MVP set has grown
to 21 probes across 6 categories.

| Category | Probes |
|---|---|
| Memory | `mem_available_mib`, `mem_used_mib`, `mem_total_mib`, `swap_used_mib`, `swap_total_mib` |
| Load | `loadavg_1m` |
| Processes | `proc_total_count`, `proc_zombie_count`, `proc_stuck_count`, `proc_running_count`, `proc_top_cpu_name` (text), `proc_top_mem_name` (text), `proc_top_mem_pct` |
| GPU | `gpu_vram_used_pct`, `gpu_vram_used_mib`, `gpu_vram_total_mib`, `gpu_temp_c`, `gpu_util_pct` |
| Disks | `disk_io_pressure_some_pct`, `disk_io_pressure_full_pct` |
| Systemd | `systemd_degraded`, `systemd_user_failed_count`, `systemd_system_failed_count` |
| Okapi (external) | `okapi_requests_active`, `okapi_errors_total`, `okapi_gpu_memory_used_pct`, etc. — via `russell okapi-probe` |

**Rule evaluation.** All numeric probes are evaluated against
the rule engine. Default rules ship in `rules.d/`. Operator
overrides live in `~/.local/share/harness/rules.d/`. Breach
events are written to the journal.

**Self-vitals (proprioception).** Five active, per JR-5:

| Self-vital | Source | Rule |
|---|---|---|
| `sentinel_last_run_age_s` | journal `MAX(ts)` | Warn > 450s, Alert > 1800s |
| `journal_writer_stall_s` | write-append timing | Warn > 5s |
| `llm_p95_latency_ms` | help_session latency | Warn > 2000ms |
| `timer_drift_s` | cadence interval | Warn > target+20% |
| `help_error_rate_pct` | failed LLM calls / total | Warn > 10% |

## 4. The Skill System

Skills are YAML-manifested bundles of probes and interventions.
They live under `~/.local/share/harness/skills/<id>/`.

Each intervention declares a risk band (`none`/`low`/`medium`/`high`/`critical`),
an `idempotent` flag, a rollback strategy, a timeout, and
a `needs_sudo` flag. The dispatcher enforces IDRS:

- **I — Idempotent.** Claimed; verified by the manifest author.
- **D — Dry-run.** `DryRun::Enabled` produces the would-do record.
- **R — Rollback.** Pre-state capture is the caller's responsibility;
  automatic rollback on forward failure is supported by the dispatcher.
- **S — Structured log.** Every dispatch writes a `harness.event.v1`
  record and an evidence bundle.

**Active skills:**
- `okapi-watcher` — probes Okapi health, intervenes with `restart-okapi`
  (risk: low, `systemctl --user restart okapi`)

**Risk enforcement.** `max_auto_risk` defaults to `Low`. The consent
flow blocks interventions above the cap with a refusal message.
Dry-run always bypasses the cap (no mutation occurs).

## 5. Persistence Scope

Every byte Russell writes is named. Full catalog at
[`PERSISTENCE_CATALOG.md`](PERSISTENCE_CATALOG.md).

| Path | Owner | Schema | Retention |
|---|---|---|---|
| `~/.local/state/harness/journal.db` | `russell-core::journal` | `0001_init.sql` | unbounded |
| `~/.local/state/harness/baselines.db` | baseline snapshot of journal | `0001_init.sql` | refreshed daily |
| `~/.local/state/harness/profile.json` | `russell-core::profile` | `russell.profile.v1` | unbounded |
| `~/.local/state/harness/evidence/help/<session-id>/` | `russell-doctor::help` | per-session JSON | 90 days |
| `~/.local/state/harness/evidence/skills/<skill>/<step>/` | `russell-skills::dispatch` | per-dispatch JSON | 90 days |
| `~/.local/share/harness/skills/<id>/` | `russell-skills` | manifest + scripts | operator-owned |
| `~/.local/share/harness/rules.d/*.toml` | `russell-core::rule` | TOML | operator-owned |
| `~/.local/share/harness/memory/chats/<session-id>.jsonl` | `russell-cli::chat` | JSON lines | unbounded |
| `~/.config/harness/russell.env` | operator | key=value | operator-owned |
| `~/.config/harness/disable` | kill switch | empty file | operator-owned |

## 6. Boundaries — what is deferred

These items remain deferred beyond the current build:

- **No MCP server.** The `russell mcp` crate exists as a
  placeholder; full MCP surface is deferred. (ADR-0003.)
- **No PolKit helpers.** Sudo for interventions uses NOPASSWD
  configuration by the operator. No PolKit integration. (ADR-0005.)
- **No remote skill registry.** Skills are local to the machine.
  (ADR-0007.)
- **No tiered Tier I / II / III cadences.** One Sentinel cadence.
- **No chaos probe.** Deferred.
- **No corrective proprioception arcs.** Detection-only (Phase 2A).
  Corrective arcs are deferred.
- **No full MCP surface.** `russell-mcp` crate exists, surface deferred.

## 7. Known Limitations

- **GPU path is hardcoded to `card1`.** On machines where the
  dGPU is at a different DRM card index, GPU probes return `None`.
- **No root-filesystem usage probe.** Requires `statvfs` (libc/nix
  dependency) which is deferred to avoid `unsafe` in the sentinel crate.
- **No NVMe SMART probe.** NVMe health files under sysfs require
  root on this kernel/driver combination.
- **Sudo requires NOPASSWD.** The consent flow does not prompt for
  a password. Operators configure `sudoers.d/` entries for each
  skill's commands.
- **Process scan is full /proc sweep.** On machines with >10,000
  processes, the 5-minute scan may be noticeable. Acceptable for
  the single-workstation target.
- **Baselines lack freshness guard.** `read_baselines()` does not
  check `updated_ts`; if baseline computation stops, Jack cites
  stale baselines.

## 8. Success Criteria

MVP is **complete** when all three of these are empirically true:

1. **Stability:** Russell runs unattended on the observed
   Framework 16 / HX 370 / Ubuntu 25.10 machine for **20
   consecutive days** with zero unexplained gaps in the journal.
2. **Tests:** `cargo fmt --check`, `cargo clippy --workspace
   --all-targets -- -D warnings`, and `cargo test --workspace`
   all pass on every commit.
3. **Help channel proof:** At least **5 successful
   `russell jack` LLM round-trips** with demonstrated
   offline-fallback resilience, and at least one was triggered
   in a real moment of operator uncertainty.

## 9. Status and Next Step

Phase 1 (MVP Doctor) is **complete**. Phase 2 (observation
expanded) is **in progress** with process, GPU, disk, and
systemd probes active. Phase 3 (skills and dispatch) is
**active** with the IDRS-gated dispatcher, consent flow, and
the `okapi-watcher` skill operational. The next priorities are
packaging, install automation, and the 20-day soak.

See [`../status/CONSOLIDATED-STATUS.md`](../status/CONSOLIDATED-STATUS.md)
for current state.

## 10. References

- [`../architecture/PRINCIPLES_CATALOG.md`](../architecture/PRINCIPLES_CATALOG.md) — JR-1 through JR-7.
- [`../architecture/overview.md`](../architecture/overview.md) — system shape.
- [`../../cybernetic-health-harness.md`](../../cybernetic-health-harness.md) — full design (the aspirational target).
- [`../../MACHINE_PROFILE.md`](../../MACHINE_PROFILE.md) — the patient.
- [`../operations/REUSE_MANIFEST.md`](../operations/REUSE_MANIFEST.md) — upstream copy register.
