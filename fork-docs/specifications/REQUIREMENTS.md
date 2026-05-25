---
title: "Russell Requirements"
audience: [architects, developers, operators]
last_updated: 2026-05-25
version: "1.0.0"
status: "Active"
domain: "Cross-cutting"
ddmvss_categories: [domain, capability]
---

# Russell Requirements

**Purpose:** Define Russell's goal specifications.

**Axiom:** `Goal ≡ Requirement` — bidirectional equivalence.

---

## Functional Requirements

### FR-1: Observe the Host

**Goal:** Russell observes the host on a 5-minute cadence.

**Criteria:**
- [x] Collects 25+ probe types (memory, swap, load, processes, GPU, disk, network, systemd)
- [x] Writes samples to journal with timestamps
- [x] Evaluates samples against rules (warn, alert, crit thresholds)
- [x] Emits events for threshold breaches

**Bounded context:** `sentinel`  
**Capability:** `observe`  
**DDMVSS categories:** Domain, Capability, Observability

---

### FR-2: Remember Observations

**Goal:** Russell remembers what he saw in a SQLite journal.

**Criteria:**
- [x] Stores samples in `journal.db::samples` table
- [x] Stores events in `journal.db::events` table
- [x] Computes EWMA baselines (30-day rolling p50/p95/p99)
- [x] Maintains hash chain integrity (tamper-evident)
- [x] Retains data for 90 days (configurable)

**Bounded context:** `journal`  
**Capability:** `remember`  
**DDMVSS categories:** Domain, Capability, Persistence

---

### FR-3: Report Through ACP

**Goal:** Russell reports through ACP (Agent Client Protocol) to hKask.

**Criteria:**
- [x] Implements ACP server (`russell-acp-server`)
- [x] Supports JSON-RPC 2.0 over stdio
- [x] Authenticates with macaroon-based OCAP tokens
- [x] Exposes 9 ACP methods (capabilities, probe/run, journal/query, skill/run, jack/consult, session/create, session/message, session/close, consent/respond)

**Bounded context:** `acp`  
**Capability:** `report`  
**DDMVSS categories:** Domain, Capability, Interface

---

### FR-4: Watch Himself (Proprioception)

**Goal:** Russell watches himself the same way he watches the host.

**Criteria:**
- [x] Collects 9 self-vitals (sentinel_last_run_age_s, journal_writer_stall_s, llm_p95_latency_ms, timer_drift_s, help_error_rate_pct, hkask_mcp_reachable_ms, remote_discovery_latency_s, journal_chain_intact, evidence_integrity_ok)
- [x] Evaluates self-vitals against thresholds
- [x] Emits events for self-vital breaches
- [x] Implements reflex arcs (threshold + rate breaches → interventions)

**Bounded context:** `proprioception`  
**Capability:** `self-watch`  
**DDMVSS categories:** Domain, Capability, Observability

---

### FR-5: Cry for Help (Jack)

**Goal:** Russell cries for help via a local LLM when asked.

**Criteria:**
- [x] Implements `russell jack` command
- [x] Assembles SOAP bundle (Subjective, Objective, Assessment, Plan)
- [x] Sends to LLM backend (Okapi default, OpenRouter opt-in)
- [x] Writes round-trip to journal
- [x] Prints response to operator
- [x] Supports natural-language consent ("ok", "yes", "do it", `/approve`)

**Bounded context:** `jack`  
**Capability:** `cry-for-help`  
**DDMVSS categories:** Domain, Capability, Interface

---

### FR-6: Act Through Skills

**Goal:** Russell acts through IDRS-compliant skills.

**Criteria:**
- [x] Loads skills from YAML manifests
- [x] Validates manifests against schema
- [x] Enforces IDRS contract (Idempotent, Dry-run, Rollback, Structured-log)
- [x] Enforces risk bands (none, low, medium, high, critical)
- [x] Requires consent for high-risk interventions
- [x] Executes probes (risk: none) without consent
- [x] Executes interventions (risk: low+) with consent
- [x] Captures evidence bundles for all executions

**Bounded context:** `skill`  
**Capability:** `act`  
**DDMVSS categories:** Domain, Capability, Composition, Trust

---

### FR-7: Manage Skills

**Goal:** Russell manages skill lifecycle.

**Criteria:**
- [x] Installs skills (`russell skill install <id>`)
- [x] Prunes skills (`russell skill prune <id>`)
- [x] Retires skills (`russell skill retire <id>`)
- [x] Lists skills (`russell skill list`)
- [x] Runs skills (`russell skill run <id>`)
- [x] Tracks skill lifecycle (discovered → evaluated → installed → active → stale → deprecated → retired)

**Bounded context:** `skill`  
**Capability:** `install`, `prune`, `retire`  
**DDMVSS categories:** Domain, Capability, Lifecycle

---

### FR-8: Maintain Profile

**Goal:** Russell maintains a profile of the host.

**Criteria:**
- [x] Generates profile on bootstrap (`profile.json`)
- [x] Captures host info (OS, chassis, CPU, memory, swap)
- [x] Captures GPU info (PCI, vendor, name, VRAM)
- [x] Captures storage info (device, size, filesystem)
- [x] Captures toolchain info (Rust, Node, container, AI)
- [x] Captures editor info (VSCodium, Zed)
- [x] Tracks honeymoon period (30 days)

**Bounded context:** `profile`  
**Capability:** —  
**DDMVSS categories:** Domain, Persistence

---

## Non-Functional Requirements

### NFR-1: Austere by Default (JR-1)

**Goal:** Russell is austere by default.

**Criteria:**
- [x] Binary size < 50 MB
- [x] Memory usage < 100 MB
- [x] Boot time < 5 seconds
- [x] No unnecessary dependencies

**Principle:** JR-1  
**DDMVSS categories:** Trust, Lifecycle

---

### NFR-2: Observe > Recommend > Act (JR-2)

**Goal:** Russell's default posture is observe > recommend > act.

**Criteria:**
- [x] Probes are read-only (risk: none)
- [x] Interventions require IDRS compliance
- [x] High-risk interventions require consent
- [x] Kill switches disable all mutations

**Principle:** JR-2  
**DDMVSS categories:** Trust

---

### NFR-3: LLM Never Emits Shell (JR-3)

**Goal:** The LLM never emits shell commands.

**Criteria:**
- [x] LLM selects from known IDs in manifests
- [x] Dispatcher rejects unknown IDs
- [x] No ad-hoc shell execution
- [x] No code generation from LLM

**Principle:** JR-3  
**DDMVSS categories:** Trust

---

### NFR-4: Small but Present — The Nurse (JR-4)

**Goal:** Russell must be able to cry for help from day one.

**Criteria:**
- [x] `russell jack` command exists
- [x] Jack persona is defined (`jack-persona.md`)
- [x] LLM backend is configurable (Okapi, OpenRouter)
- [x] Round-trip is journaled

**Principle:** JR-4  
**DDMVSS categories:** Domain, Capability

---

### NFR-5: Proprioception — Jack Watches Jack (JR-5)

**Goal:** Russell watches himself the same way he watches the host.

**Criteria:**
- [x] 9 self-vitals are collected
- [x] Self-vitals are evaluated against thresholds
- [x] Reflex arcs propose interventions
- [x] Autoimmune guard prevents cascading failures

**Principle:** JR-5  
**DDMVSS categories:** Domain, Capability, Observability

---

### NFR-6: Reuse, Don't Depend (JR-6)

**Goal:** Russell copies code from upstream rather than depending on them.

**Criteria:**
- [x] REUSE_MANIFEST.md documents all copied code
- [x] Copied code is in `vendor/` directories
- [x] Sync policy is documented
- [x] No unnecessary Cargo dependencies

**Principle:** JR-6  
**DDMVSS categories:** Lifecycle

---

### NFR-7: Persistence is Auditable (JR-7)

**Goal:** Everywhere Russell remembers something, it is documented.

**Criteria:**
- [x] PERSISTENCE_CATALOG.md documents all storage
- [x] Journal schema is documented
- [x] Profile schema is documented
- [x] Evidence bundle structure is documented
- [x] Hash chain integrity is verifiable

**Principle:** JR-7  
**DDMVSS categories:** Persistence

---

## Operator Sovereignty Requirements

### OSR-1: The Operator Controls Russell

**Goal:** The operator can stop, delete, modify, and uninstall Russell at any time.

**Criteria:**
- [x] `systemctl --user stop russell-*` stops Russell
- [x] `rm -rf ~/.local/state/harness/` deletes state
- [x] `~/.config/harness/` is user-editable
- [x] `./packaging/bin/uninstall.sh` uninstalls Russell

**Magna Carta:** S-1  
**DDMVSS categories:** Trust, Lifecycle

---

### OSR-2: Russell Does Not Phone Home

**Goal:** Russell makes no network connections unless explicitly configured.

**Criteria:**
- [x] No telemetry by default
- [x] No update checks by default
- [x] No crash reports by default
- [x] No analytics by default
- [x] Network access is opt-in

**Magna Carta:** S-2  
**DDMVSS categories:** Trust

---

### OSR-3: Russell Does Not Escalate Privileges

**Goal:** Russell runs as the operator's user.

**Criteria:**
- [x] No sudo access required
- [x] No system-wide package installation
- [x] No system configuration modification
- [x] No system service creation

**Magna Carta:** S-3  
**DDMVSS categories:** Trust

---

## Single-Host Requirements

### SHR-1: One Machine, One Operator

**Goal:** Russell monitors exactly one machine for exactly one operator.

**Criteria:**
- [x] No multi-tenant mode
- [x] No fleet management
- [x] No cross-machine correlation
- [x] No central aggregator

**Magna Carta:** H-1  
**DDMVSS categories:** Domain

---

### SHR-2: Local-First, Local-Only

**Goal:** All Russell state lives on the host machine.

**Criteria:**
- [x] Journal is local (`~/.local/state/harness/journal.db`)
- [x] Profile is local (`~/.local/state/harness/profile.json`)
- [x] Evidence is local (`~/.local/state/harness/evidence/`)
- [x] Skills are local (`~/.local/share/harness/skills/`)
- [x] No synchronization to external services

**Magna Carta:** H-2  
**DDMVSS categories:** Persistence

---

### SHR-3: The Operator is the Policy Layer

**Goal:** Russell has no role-based access control.

**Criteria:**
- [x] No RBAC
- [x] No multi-tenant auth
- [x] No permission model beyond "the user who launched systemd --user"

**Magna Carta:** H-3  
**DDMVSS categories:** Trust

---

## Traceability Matrix

| Requirement | Bounded Context | Capability | Principle | DDMVSS Categories |
|-------------|-----------------|------------|-----------|-------------------|
| FR-1 | sentinel | observe | — | Domain, Capability, Observability |
| FR-2 | journal | remember | JR-7 | Domain, Capability, Persistence |
| FR-3 | acp | report | — | Domain, Capability, Interface |
| FR-4 | proprioception | self-watch | JR-5 | Domain, Capability, Observability |
| FR-5 | jack | cry-for-help | JR-4 | Domain, Capability, Interface |
| FR-6 | skill | act | JR-2, JR-3 | Domain, Capability, Composition, Trust |
| FR-7 | skill | install, prune, retire | — | Domain, Capability, Lifecycle |
| FR-8 | profile | — | JR-7 | Domain, Persistence |
| NFR-1 | — | — | JR-1 | Trust, Lifecycle |
| NFR-2 | — | — | JR-2 | Trust |
| NFR-3 | — | — | JR-3 | Trust |
| NFR-4 | — | — | JR-4 | Domain, Capability |
| NFR-5 | — | — | JR-5 | Domain, Capability, Observability |
| NFR-6 | — | — | JR-6 | Lifecycle |
| NFR-7 | — | — | JR-7 | Persistence |
| OSR-1 | — | — | — | Trust, Lifecycle |
| OSR-2 | — | — | — | Trust |
| OSR-3 | — | — | — | Trust |
| SHR-1 | — | — | — | Domain |
| SHR-2 | — | — | — | Persistence |
| SHR-3 | — | — | — | Trust |

---

## Completeness

**Total requirements:** 21  
**Satisfied:** 21 (100%)  
**Unsatisfied:** 0 (0%)

**Status:** MVP complete

---

## References

- DDMVSS: `architecture/DDMVSS.md`
- Principles: `architecture/PRINCIPLES.md`
- Magna Carta: `architecture/magna-carta.md`
- Domain and Capability: `architecture/domain-and-capability.md`
