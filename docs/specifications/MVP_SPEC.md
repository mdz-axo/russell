---
title: "MVP Russell — the Minimal Viable Terrier"
audience: [operators, developers, architects, contributors, agents]
last_updated: 2026-05-24
togaf_phase: "Requirements Management"
version: "1.3.0"
status: "Active"
---

# MVP Russell — the Minimal Viable Terrier

<!-- TOGAF_DOMAIN: Requirements Management -->
<!-- VERSION: 1.3.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-24 -->

This is the **pinned boundary** of MVP Russell. Anything outside
this boundary requires an ADR or a spec update to this document.

The principle that pins it is JR-1 (see
[`../architecture/PRINCIPLES_CATALOG.md`](../architecture/PRINCIPLES_CATALOG.md)):
*Though she be but little, she is fierce.*

## 1. The One-Paragraph Russell

MVP Russell is a cybernetic health harness for a single Linux AI/ML workstation, operating as an **ACP (Agent Client Protocol) agent** integrated with hKask. He **observes** the host every five minutes via a broad probe set covering CPU, memory, processes, GPU, disks, and systemd, **remembers** what he saw in a SQLite journal, **reports** through ACP sessions with Jack (the nurse persona) and CLI verbs, **watches himself** through five self-vitals (proprioception), and when asked he **cries for help** to a local LLM (Okapi by default). He **proposes** interventions via loaded skills, and with the operator's explicit consent, he **executes** them through the IDRS-gated skill dispatcher. He is small and he is fierce.

**Primary interface:** ACP server for hKask integration  
**Secondary interface:** CLI for local operator actions  
**Deployment:** Hybrid (ACP server + systemd sentinel timer)

## 2. The Verbs

The MVP exposes capabilities through two interfaces:

### 2.1 ACP Methods (Primary — hKask Integration)

| Method | Risk | Role | Reach |
|---|---|---|---|
| `acp/session.create` | none | Create multi-turn session with Jack | stdio |
| `acp/session.message` | none | Send message, receive Jack response | stdio |
| `acp/session.close` | none | Close session | stdio |
| `acp/capabilities` | none | List public skills and probes | stdio |
| `acp/skill/info` | none | Get skill metadata | stdio |
| `acp/probe/run` | none | Run read-only probe | stdio + process |
| `acp/skill/run` | varies | Run skill (probes auto, interventions require consent) | stdio + process |

### 2.2 CLI Verbs (Secondary — Local Operator)

| Verb | Risk | Role | Reach |
|---|---|---|---|
| `russell status` | none | Read-only summary of paths, kill-switch, journal, profile | local fs |
| `russell list` | none | Most-recent journal events | local sqlite |
| `russell profile [--init]` | none | Show / initialize `profile.json` | local fs |
| `russell digest [--since-hours N]` | none | Markdown summary of recent activity | local sqlite |
| `russell sentinel-once` | none | Fire the Sentinel once, append samples, evaluate rules | local fs |
| `russell jack [--note "..."]` | none | Compose SOAP-shaped prompt and consult the LLM; print response | network *(opt-in)* |
| `russell skill list` | none | List loaded skills, probes, and interventions | local fs |
| `russell skill run <id>` | varies | Execute a skill probe or intervention via the IDRS-gated dispatcher | local fs + process |
| `russell proprio` | none | Run self-observation and append self-vital samples | local sqlite |
| `russell self-triage` | none | Russell diagnoses own health | local sqlite |
| `russell self-triage` | none | Russell diagnoses own health | local sqlite |

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

If Jack proposes a probe (`ACTION: <skill>/<probe-id>`), it
executes immediately and the output is printed. If Jack proposes
an intervention (`ACTION: <skill>/<intervention-id>`), the
operator is shown the risk band and prompted for consent.

**Offline fallback.** If the LLM is unreachable, a rule-based
summary of severity counts and most-recent events is printed.

### 2.2 The ACP consent flow — interactive operator consent

The ACP session interface is the canonical path for operator consent
to interventions (ADR-0041). Through ACP sessions, Jack can:

- **Run probes** (risk: none) immediately via `ACTION:` — no
  consent required. Probe output is returned in the session.
- **Propose interventions** via `ACTION:` — the hKask agent
  surfaces a `PendingAction` to the operator, who responds
  with `acp/consent.respond` (approve or deny).

The skill dispatcher executes approved interventions with full
IDRS journaling. Risk enforcement gates on `max_auto_risk`
(default: Low). Sudo-requiring interventions require NOPASSWD
configuration by the operator.

## 3. The Observation Loop

A single 5-minute Sentinel cadence, driven by systemd timer
(`russell-sentinel.timer`). `russell sentinel-once` is the
manual trigger for development.

**Probe set (current).** The original 3-probe MVP set has grown
to 25 probes across 7 categories.

| Category | Probes |
|---|---|
| Memory | `mem_available_mib`, `mem_pressure_some_pct`, `mem_pressure_full_pct` |
| Swap | `swap_used_mib` |
| Load | `loadavg_1m` |
| Processes | `proc_total_count`, `proc_zombie_count`, `proc_stuck_count`, `proc_running_count`, `proc_top_cpu_name` (text), `proc_top_mem_name` (text), `proc_top_mem_pct` |
| GPU | `gpu_vram_used_pct`, `gpu_vram_used_mib`, `gpu_vram_total_mib`, `gpu_temp_c`, `gpu_util_pct` |
| Disks | `disk_root_used_pct`, `disk_io_pressure_some_pct`, `disk_io_pressure_full_pct` |
| Network | `net_tcp_connections`, `net_tcp6_connections` |
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
| `~/.local/state/harness/journal.db` | `russell-core::journal` | `0001_init.sql` | unbounded (baselines refreshed daily in-table) |
| `~/.local/state/harness/profile.json` | `russell-core::profile` | `russell.profile.v1` | unbounded |
| `~/.local/state/harness/evidence/help/<session-id>/` | `russell-meta::help` | per-session JSON | 90 days |
| `~/.local/state/harness/evidence/skills/<skill>/<step>/` | `russell-skills::dispatch` | per-dispatch JSON | 90 days |
| `~/.local/share/harness/skills/<id>/` | `russell-skills` | manifest + scripts | operator-owned |
| `~/.local/share/harness/rules.d/*.toml` | `russell-core::rule` | TOML | operator-owned |
| `~/.config/harness/russell.env` | operator | key=value | operator-owned |
| `~/.config/harness/disable` | kill switch | empty file | operator-owned |

## 6. Boundaries — what is deferred

These items remain deferred beyond the current build:

- **No PolKit helpers.** Sudo for interventions uses NOPASSWD
  configuration by the operator. No PolKit integration. (ADR-0005.)
- **No remote skill registry.** Skills are local to the machine.
  (ADR-0007.)
- **No tiered Tier I / II / III cadences.** One Sentinel cadence.
- **No chaos probe.** Deferred.
- **No corrective proprioception arcs.** Detection-only (Phase 2A).
  Corrective arcs are deferred.
- **No multi-agent session topology.** Single-agent 1:1 sessions.
  (ADR-0045, deferred.)
- **No ACP protocol versioning.** Single protocol version.
  (ADR-0046, deferred.)

### Implemented (formerly deferred)

- **MCP client** — `russell-mcp` client connects to hKask MCP endpoint
  for tool access. Server feature deprecated; ACP is the primary
  integration protocol. (ADR-0003, deferral lifted; ADR-0027.)
- **Landlock sandboxing** — Skill subprocesses run under Landlock
  filesystem confinement. (ADR-0024.)
- **ACP server** — `russell-acp-server` provides Agent Client Protocol
  over JSON-RPC 2.0 stdio with macaroon OCAP auth. (ADR-0027.)

## 7. Security Hardening (Phase 5, 2026-05-23)

The following security improvements are **complete**:

- **Unified `RiskBand` type** — Eliminated 4 duplicate risk enums across
  `russell-skills`, `russell-acp-server`, `russell-reflex`, and `russell-core`.
  Single canonical definition in `russell-core::risk::RiskBand`. (C4, C7)
- **DNS rebinding protection** — MCP client validates all resolved IPs are
  loopback, rejecting hostnames that resolve to non-loopback addresses.
  Prevents `localhost.evil.com` attacks. (W-20)
- **Configurable endpoints** — Eliminated hardcoded `127.0.0.1:8080` and
  `127.0.0.1:11435`. All endpoints now configurable via `RuntimeConfig`
  with env var overrides. (W-14, W-21)
- **Service token authentication** — Russell generates and persists a
  service token for hKask inference requests. Token stored at
  `~/.local/state/harness/russell.token` with `0600` permissions. (W-22)
- **Hardened hash chain genesis** — Removed fallback constant seed.
  Genesis now uses `/etc/machine-id` or generates random 32-byte seed
  on first run. (W-11)
- **Hexagonal port migration** — Journal write/read operations abstracted
  via `JournalWritePort` and `JournalReadPort` traits. Enables test doubles
  and future storage backends. (W-04)
- **ACP server hardening** — Added `#![deny(unsafe_code)]` to
  `russell-acp-server`. (W-12)

## 8. Known Limitations

- **GPU path is hardcoded to `card1`.** On machines where the
  dGPU is at a different DRM card index, GPU probes return `None`.
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
- **`df` subprocess required for disk usage.** The `disk_root_used_pct`
  probe spawns `df` as a controlled subprocess. The binary must
  be in `$PATH`.

## 9. Success Criteria

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

## 10. Status and Next Step

Phase 0 (skeleton) is **complete**. Phase 1 (MVP Doctor) is
**complete**. Phase 1b (install artifacts) is **shipped**.
Phase 1c (20-day soak) is **closed**. Phase 2 (observation
sharpened) is **active** with 5 self-vitals, rule engine,
EWMA baselines, and expanded probes. Phase 3 (skills and
dispatch) is **complete** with the IDRS-gated dispatcher,
consent flow, and the `okapi-watcher` skill operational.
Phase 5 (security hardening) is **complete** with unified
risk types, DNS rebinding protection, configurable endpoints,
service token auth, hardened hash chain, and hexagonal ports.
The next priorities are packaging hardening and the next
soak cycle.

See [`../status/CONSOLIDATED-STATUS.md`](../status/CONSOLIDATED-STATUS.md)
for current state.

## 11. References

- [`../architecture/PRINCIPLES_CATALOG.md`](../architecture/PRINCIPLES_CATALOG.md) — JR-1 through JR-7.
- [`../architecture/overview.md`](../architecture/overview.md) — system shape.
- [`../../cybernetic-health-harness.md`](../../cybernetic-health-harness.md) — full design (the aspirational target).
- [`../../MACHINE_PROFILE.md`](../../MACHINE_PROFILE.md) — the patient.
- [`../operations/REUSE_MANIFEST.md`](../operations/REUSE_MANIFEST.md) — upstream copy register.
