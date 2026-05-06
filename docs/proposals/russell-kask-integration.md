# Proposal: Russell as Kask's Local System Agent

**Date:** 2026-05-06
**Status:** Draft
**Audience:** Kask maintainers, Russell maintainers, operators

---

## 1. Executive Summary

Russell should ship as a **bundled local-system health agent** with every
Kask installation. Where Kask's existing `HealthSentinel` and Curator
system observe the *platform's internal state* (LLM provider health,
budget signals, dispatch latency, bitemporal store compaction), Russell
observes the *host machine and the Kask installation itself* — the layer
beneath that Kask currently cannot see.

The integration is natural: Kask already has the Curator architecture
(observe → classify → advise → escalate), the `arsenal-mcp-maintenance`
tool server (9 maintenance tools awaiting wiring), and the
`stack-control-plane` ports/adapters pattern. Russell provides the
missing bottom layer: host telemetry, build-chain health, dependency
freshness, and installation integrity — exposed as an MCP tool server
that Kask's Curators can consume.

---

## 2. The Gap Russell Fills

### What Kask observes today (internal)

| Layer | Observer | Cadence | Evidence |
|-------|----------|---------|----------|
| LLM provider health | `HealthSentinel` in `stack-api` | 60 s | Error rates, latency p50/p95, circuit breaker trips |
| Budget/cost | `HealthSentinel` + `stack-paid` | 60 s | Spend tracking, projected monthly, algedonic alerts |
| Dispatch health | `DispatchCurator` | 60 s | Intent resolution, catalog availability |
| Bitemporal store | `BitemporalStoreCurator` | 60 s | Compaction, backup freshness |
| Embedding health | `EmbeddingCurator` | 60 s | Model compatibility, re-embedding needs |

### What Kask cannot observe today (external/host)

| Layer | Gap | Impact |
|-------|-----|--------|
| **Host resources** | RAM pressure, swap saturation, disk space, GPU VRAM | Kask inference OOMs without warning; store writes fail silently on full disk |
| **Rust toolchain** | MSRV drift, missing components, snap/rustup hangs | `cargo check` fails; development halts (we just hit this) |
| **Kask build health** | Workspace compilation, clippy, test suite | Regressions accumulate unnoticed between sessions |
| **Dependency freshness** | Cargo.lock staleness, security advisories | Supply-chain risk grows silently |
| **Ollama/local inference** | Process liveness, model availability, VRAM allocation | `stack-llm` falls back to remote without explanation |
| **systemd service state** | Timer health, unit failures, journal errors | MCP servers silently dead; no one notices |
| **Network/DNS** | Connectivity to OpenRouter, Qdrant, Runpod | Provider timeouts blamed on provider when it's local DNS |
| **Disk I/O / NVMe health** | SMART status, write latency, queue depth | Silent data corruption; slow journal writes |

Russell already observes the first row (mem, swap, load). The others are
the Phase-2+ probe set that the rule engine enables.

---

## 3. Functional Mapping: Russell → Kask Requirements

### 3.1 Russell capabilities mapped to Kask maintenance needs

| Russell capability | Kask requirement it serves | Integration point |
|--------------------|---------------------------|-------------------|
| **Host telemetry** (mem, swap, load, GPU) | FR-17 inference needs VRAM awareness; Curators need host context for escalation decisions | Russell samples → MCP tool `russell/host_snapshot` |
| **Self-vital / proprioception** | Kask's `arsenal-mcp-maintenance` `health_snapshot` tool needs host-level data | Russell self-vital feeds Curator health reports |
| **Build-chain probe** (cargo check, clippy, test) | `TODO.md` P0: "fix stack-control-plane test compile error" — Russell could catch this automatically | New probe: `kask_build_health` |
| **Dependency audit** (cargo audit, advisory DB) | NFR-03 dependency direction + supply-chain hygiene | New probe: `kask_dependency_freshness` |
| **Ollama liveness** | `stack-llm` router needs to know if local inference is actually available before routing | New probe: `ollama_health` (process, model list, VRAM) |
| **systemd unit health** | MCP servers (`arsenal-mcp-*`) run as services; silent death = silent capability loss | New probe: `kask_services_health` |
| **Journal / SQLite integrity** | Kask uses SQLite for auth, Arsenal journal, kask-store; corruption = data loss | New probe: `sqlite_integrity` |
| **Disk space / NVMe SMART** | `stack-bitemporal-store` and `arsenal-artifact-store` write to local filesystem | New probe: `disk_health` |
| **Network connectivity** | Provider timeouts (F-5 in Russell soak) need local vs. remote disambiguation | New probe: `network_reachability` |
| **Cry for help** (`russell jack`) | Operator's "what's wrong with my box?" when Kask misbehaves | Already implemented; becomes the first-line triage |

### 3.2 Russell mapped to Kask's Curator architecture

Kask's Curator system has a two-tier design:

1. **Subsystem Curators** — pure, synchronous health assessors (trait `Curator`)
2. **Domain Curators** — named agents that aggregate, schedule, escalate

Russell maps cleanly as a **new Domain Curator** (working name: **"Duncan"** —
the infrastructure guardian) that owns host-level subsystem Curators:

```
Duncan (Domain Curator — infrastructure)
├── HostResourceCurator      ← Russell host telemetry
├── BuildChainCurator        ← Russell cargo/toolchain probes
├── ServiceHealthCurator     ← Russell systemd probes
├── InferenceSubstrateCurator ← Russell Ollama/GPU probes
└── StorageIntegrityCurator  ← Russell disk/SQLite probes
```

Each subsystem Curator implements `trait Curator` from `stack-control-plane`,
consuming Russell's journal data as its state snapshot (OCAP-compliant:
read-only access to Russell's SQLite).

---

## 4. Architecture: How They Connect

### 4.1 Deployment topology

```
~/.local/bin/russell              ← Russell binary (installed by Kask installer)
~/.local/state/harness/journal.db ← Russell's SQLite journal
~/.config/harness/russell.env     ← Russell config (API keys, thresholds)
~/.config/systemd/user/
  russell-sentinel.timer          ← 5-min host observation cadence
  russell-sentinel.service        ← runs `russell sentinel-once`

~/Clones/kask/                    ← Kask source checkout
  stack/crates/stack-control-plane/
    src/curator_infrastructure.rs  ← Duncan's subsystem Curators (reads Russell journal)
  arsenal/crates/arsenal-mcp-russell/
    src/                           ← Russell MCP tool server (8-10 tools)
```

### 4.2 Integration pattern: MCP tool server

Russell exposes itself as an **MCP tool server** (`arsenal-mcp-russell`)
that Kask's Curators and operators can invoke. This follows the existing
Arsenal pattern (26 MCP servers already exist).

**Proposed tools:**

| Tool | Risk | Returns |
|------|------|---------|
| `russell/host_snapshot` | none | Current mem, swap, load, GPU, disk; last 24h min/avg/max |
| `russell/self_vital` | none | Sentinel age, cadence health, cycle count |
| `russell/build_check` | none | Last `cargo check` result for each workspace; staleness |
| `russell/dependency_audit` | none | Known advisories, Cargo.lock age, MSRV compliance |
| `russell/service_status` | none | systemd unit states for Kask-related services |
| `russell/ollama_health` | none | Process liveness, loaded models, VRAM usage |
| `russell/journal_query` | none | Time-range filtered events/samples from Russell's journal |
| `russell/ask_jack` | none | Compose SOAP + consult LLM (the cry-for-help path) |

All tools are **read-only** (JR-2: observe > recommend > act). The MCP
server reads Russell's journal and runs probes on demand; it never
mutates host state.

### 4.3 Data flow

```
[systemd timer: 5 min]
    → russell sentinel-once
        → host probes (mem, swap, load, GPU, disk, services, ollama, cargo)
        → self-vital (proprio)
        → write samples + events → journal.db

[Kask CuratorRuntime: 60s tick]
    → Duncan.assess_domain_health()
        → reads russell journal.db (via JournalReader or MCP tool call)
        → classifies findings (Healthy / Warning / Critical)
        → emits ProvenanceEvent if escalation warranted
        → advisory attached to next operator interaction

[Operator: `russell jack` or Kask admin CLI]
    → SOAP prompt includes both Russell journal AND Kask Curator findings
    → LLM sees the full picture: host + platform + application
```

### 4.4 What Russell does NOT become

- **Not a Kask dependency.** Russell is a standalone binary. Kask reads
  its journal; Russell does not import Kask crates. (JR-6: reuse, don't
  depend.)
- **Not a mutation engine.** Russell observes and reports. Kask's
  `arsenal-mcp-maintenance` is the action surface. Russell feeds
  findings; maintenance tools execute procedures.
- **Not a replacement for HealthSentinel.** Kask's internal sentinel
  observes LLM providers, budgets, dispatch. Russell observes the host
  and the installation. They are complementary layers (VSM System 1 vs.
  System 3*).

---

## 5. Kask Installation Requirements Russell Would Serve

Based on the Kask developer guide, TODO.md, and technology architecture,
these are the concrete maintenance requirements on a local machine:

### 5.1 Build environment health

| Requirement | Russell probe | Threshold |
|-------------|---------------|-----------|
| Rust toolchain present and correct MSRV | `toolchain_health` | MSRV from `Cargo.toml` vs. installed version |
| `cargo check --workspace` succeeds for stack/ | `kask_stack_build` | Exit code 0 |
| `cargo check --workspace` succeeds for arsenal/ | `kask_arsenal_build` | Exit code 0 |
| `cargo clippy` clean | `kask_clippy` | Zero warnings with `-D warnings` |
| No `stack-control-plane` test failures | `kask_test_health` | Specific regression from TODO.md |

### 5.2 Runtime environment health

| Requirement | Russell probe | Threshold |
|-------------|---------------|-----------|
| Ollama running and responsive | `ollama_liveness` | HTTP 200 from `/api/tags` within 5s |
| At least one GGUF model loaded | `ollama_models` | Model count > 0 |
| GPU VRAM available for inference | `gpu_vram_available` | > 2 GiB free |
| MCP registry file valid JSON | `mcp_registry_valid` | Parse succeeds |
| Kask store directory writable | `kask_store_writable` | Write + delete test file |
| SQLite databases not corrupted | `sqlite_integrity_check` | `PRAGMA integrity_check` = "ok" |

### 5.3 Dependency and security health

| Requirement | Russell probe | Threshold |
|-------------|---------------|-----------|
| No known vulnerabilities in Cargo.lock | `cargo_audit` | Zero advisories (or only ignored) |
| Cargo.lock not older than 30 days | `lockfile_freshness` | mtime within 30 days |
| No path-dep breakage (Axolotl issue) | `path_dep_resolution` | `cargo metadata --no-deps` succeeds |

### 5.4 Operational health

| Requirement | Russell probe | Threshold |
|-------------|---------------|-----------|
| Disk space sufficient | `disk_free` | > 10 GiB on partition holding `~/.local/state/` |
| NVMe SMART healthy | `nvme_smart` | No critical warnings |
| Network reaches OpenRouter | `network_openrouter` | HTTPS connect within 10s |
| Network reaches Ollama | `network_ollama` | HTTP connect to configured host |
| systemd user timers firing | `timer_health` | No timers in "failed" state |

---

## 6. Implementation Plan

### Phase A: Russell ships with Kask install (minimal, 1 week)

1. Add Russell binary build to Kask's install/bootstrap script.
2. Install `russell-sentinel.timer` alongside Kask services.
3. Add `russell/host_snapshot` and `russell/journal_query` as MCP tools
   in `arsenal-mcp-russell` (new crate, 2 tools initially).
4. Register `arsenal-mcp-russell` in `~/.config/stack/mcp-registry.json`.
5. Kask operators can now ask Jack about their box via `russell jack`.

### Phase B: Kask-aware probes (2 weeks)

1. Add probes to Russell's sentinel for: Ollama liveness, Kask SQLite
   integrity, disk space, systemd service state.
2. These require lifting Russell's rule engine (Phase 2 work already
   planned) so thresholds can fire events.
3. Wire `arsenal-mcp-russell` to expose the new probes as MCP tools.

### Phase C: Curator integration (2 weeks)

1. Implement `InfrastructureDomainCurator` ("Duncan") in
   `stack-control-plane` that reads Russell's journal.
2. Wire Duncan into `CuratorRuntime` alongside existing Curators.
3. Duncan's findings flow through the existing escalation protocol
   and surface in `arsenal-mcp-maintenance` health snapshots.

### Phase D: Build-chain probes (1 week)

1. Add `kask_build_health` probe that runs `cargo check --workspace`
   for both workspaces on a daily cadence (separate timer, not the
   5-min sentinel — builds are expensive).
2. Add `kask_dependency_freshness` probe (cargo audit, lockfile age).
3. Duncan classifies build failures as Critical findings.

---

## 7. Why This Works (Design Alignment)

| Russell principle | Kask alignment |
|-------------------|----------------|
| **JR-1** (austere) | Kask AGENTS.md: "when in doubt, write the smaller version" |
| **JR-2** (observe > recommend > act) | Curator pattern: assess → advise → escalate (never auto-act without policy) |
| **JR-3** (LLM never emits shell) | Kask: "Rust should route context, manage state… strategy belongs in prompts" |
| **JR-5** (proprioception) | Kask: HealthSentinel + CuratorRuntime already implement self-observation for the platform layer |
| **JR-6** (reuse, don't depend) | Russell reads Kask's journal; Kask reads Russell's journal. No crate dependency. |
| **JR-7** (persistence is auditable) | Kask: SQLite + redb + filesystem stores, all named |
| **IDRS** (idempotent, dry-run, rollback, structured-log) | Kask: `arsenal-mcp-maintenance` procedures are template-rendered, auditable, reversible |

The VSM mapping is particularly clean:

| VSM Layer | Russell | Kask |
|-----------|---------|------|
| System 1 (Operations) | Sentinel probes | Arsenal MCP tools |
| System 2 (Coordination) | systemd timers | CuratorRuntime scheduling |
| System 3 (Control) | Doctor / Jack | Domain Curators |
| System 3* (Audit) | Proprioception | HealthSentinel |
| System 4 (Intelligence) | LLM consultation | stack-llm routing |
| System 5 (Policy) | The human operator | The human operator |

---

## 8. What This Proposal Does NOT Recommend

- **Do not merge the codebases.** Russell stays in `~/Clones/russell`;
  Kask stays in `~/Clones/kask`. They communicate via journal reads and
  MCP tools. JR-6 is absolute.
- **Do not give Russell write access to Kask state.** Russell observes
  Kask's filesystem (read-only). Kask observes Russell's journal
  (read-only). Mutations go through their respective IDRS-governed
  paths.
- **Do not make Russell async/Tokio.** Russell is a synchronous oneshot
  binary triggered by systemd. Kask is an async runtime. They don't
  need to share a process model.
- **Do not add Kask as a Russell dependency.** Russell's `Cargo.toml`
  must never reference `stack-*` or `arsenal-*` crates. The MCP tool
  server (`arsenal-mcp-russell`) lives in the Kask repo and reads
  Russell's journal as a SQLite file.

---

## 9. Immediate Next Steps (if accepted)

1. **Russell side:** Complete Phase 2 (rule engine + EWMA) as planned.
   Add Ollama and disk probes to the sentinel. These are useful
   regardless of Kask integration.
2. **Kask side:** Create `arsenal-mcp-russell` crate (skeleton + 2
   tools: `host_snapshot`, `journal_query`). Register in MCP registry.
3. **Kask side:** Add `russell` binary build + install to Kask's
   bootstrap/install script.
4. **Joint:** Define the `CuratorFinding` severity mapping from
   Russell's `Severity` enum to Kask's `FindingSeverity` enum.
5. **Joint:** Write an ADR in both repos documenting the integration
   boundary and the "no cross-dependency" rule.

---

## 10. The Toolchain Lesson

Today's session produced a concrete example of why this integration
matters. Russell's `rust-toolchain.toml` pinned an exact version
(`1.94.1`) that wasn't installed; the snap-based rustup hung trying to
download it. This blocked all Russell development for the session.

With the proposed integration:
- Russell's `toolchain_health` probe would have detected the mismatch
  on the next 5-minute cycle.
- Duncan (the infrastructure Curator) would have classified it as a
  Critical finding ("build environment broken").
- The operator would have seen it in `russell jack` or in Kask's admin
  CLI before sitting down to code.
- The fix (pin to `stable`) would have been suggested by Jack in the
  SOAP response.

That's the cybernetic loop closing: sense → classify → advise → fix →
verify. Russell provides the first two; Kask's Curators provide the
middle two; the operator provides the last one.
