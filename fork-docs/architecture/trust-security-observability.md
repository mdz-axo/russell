---
title: "Russell Trust, Security, and Observability"
audience: [architects, developers, agents]
last_updated: 2026-05-25
version: "1.0.0"
status: "Active"
domain: "Cross-cutting"
ddmvss_categories: [trust, observability]
---

# Russell Trust, Security, and Observability

**Purpose:** Define Russell's security model (IDRS contract, risk bands, kill switches) and observability model (journal, proprioception, CNS spans).

**Axiom:** *First, do no harm.* — Hippocratic Oath.

---

## 1. Trust Model

### 1.1 IDRS Contract

Every mutation must satisfy all four:

| Letter | Requirement | Description |
|--------|-------------|-------------|
| **I** | Idempotent | Second run = first run's end state |
| **D** | Dry-run | `--dry-run` flag produces would-do record with zero side effects |
| **R** | Rollback | Pre-state captured; `rollback_id` or documented justification |
| **S** | Structured log | Event record appended to journal |

**Enforcement:** The skill dispatcher validates IDRS compliance before executing any intervention.

### 1.2 Risk Band Policy

| Risk Band | Auto-execute? | Consent Required? | Example |
|-----------|---------------|-------------------|---------|
| `none` | Yes | No | Probes (read-only) |
| `low` | Yes | No | Restart service, toggle config |
| `medium` | No | Yes | Edit config, install package |
| `high` | No | Yes + confirmation | Kernel module reload |
| `critical` | Never | Explicit approval | Delete files, format disk |

**Honeymoon period:** For the first 30 days after bootstrap, Russell clamps effective `max_auto_risk` to `low` for any skill with `risk: high` interventions.

### 1.3 Kill Switches

| Kill Switch | Path | Effect |
|-------------|------|--------|
| Global | `~/.config/harness/disable` | Disables all mutations |
| Per-module | `russell pause <module> --until <date>` | Pauses named module |
| Andon cord | Automatic | Halts mutations on repeated failures |

**Check:** Every mutating code path calls `is_disabled()` before proceeding.

### 1.4 Consent Flow

| Surface | Consent Mechanism |
|---------|-------------------|
| CLI | `russell skill run <id>` prompts for confirmation |
| ACP | `acp/consent/respond` method |
| Chat | Natural language: "ok", "yes", "do it", `/approve` |

**Parser:** `russell-meta::consent::parse_consent()` recognizes consent variants.

---

## 2. Security Model

### 2.1 Threat Model (STRIDE-lite)

| Threat | Mitigation |
|--------|-----------|
| **Spoofing** | Macaroon-based OCAP tokens for ACP |
| **Tampering** | Hash chain in journal (tamper-evident) |
| **Repudiation** | Structured logs with evidence bundles |
| **Information disclosure** | Local-first, no network by default |
| **Denial of service** | Timeout enforcement, kill switches |
| **Elevation of privilege** | No sudo, no system services |

### 2.2 Capability Boundaries

| Boundary | Enforcement |
|----------|-------------|
| **No ambient authority** | Every operation requires capability token |
| **Attenuation on delegation** | Tokens can be attenuated (reduced scope) |
| **Revocation via expiration** | Tokens have TTL |
| **No LLM shell** | LLM cannot generate shell commands |

### 2.3 Skill Security

| Security Property | Enforcement |
|-------------------|-------------|
| **Manifest validation** | Reject invalid manifests |
| **IDRS compliance** | Require IDRS for all interventions |
| **Risk enforcement** | Block high-risk without consent |
| **Timeout enforcement** | Kill processes exceeding timeout |
| **Rollback enforcement** | Execute rollback on failure |

---

## 3. Observability Model

### 3.1 Journal Schema

```sql
-- Samples (probe observations)
CREATE TABLE samples (
  ts INTEGER NOT NULL,
  scope TEXT NOT NULL,  -- 'host' or 'self'
  probe TEXT NOT NULL,
  value_num REAL,
  value_text TEXT,
  unit TEXT,
  PRIMARY KEY (ts, scope, probe)
);

-- Events (structured log)
CREATE TABLE events (
  id TEXT PRIMARY KEY,  -- ULID
  ts_unix INTEGER NOT NULL,
  ts TEXT NOT NULL,  -- RFC 3339
  schema TEXT NOT NULL,
  scope TEXT NOT NULL,
  severity TEXT NOT NULL,  -- info, warn, alert, crit
  action TEXT NOT NULL,
  dry_run INTEGER NOT NULL,
  summary TEXT,
  evidence_ref TEXT,
  duration_ms INTEGER,
  outputs TEXT,  -- JSON
  payload TEXT,  -- JSON
  prev_hash TEXT,
  hash TEXT
);

-- Baselines (EWMA statistics)
CREATE TABLE baselines (
  probe TEXT NOT NULL,
  scope TEXT NOT NULL,
  ewma_mean REAL,
  ewma_var REAL,
  p50 REAL,
  p95 REAL,
  p99 REAL,
  updated_ts INTEGER NOT NULL,
  PRIMARY KEY (probe, scope)
);

-- Help sessions (Jack consultations)
CREATE TABLE help_sessions (
  id TEXT PRIMARY KEY,  -- ULID
  ts_unix INTEGER NOT NULL,
  backend TEXT NOT NULL,
  model TEXT NOT NULL,
  note TEXT,
  prompt_chars INTEGER,
  response_chars INTEGER,
  latency_ms INTEGER,
  status TEXT NOT NULL,  -- ok, error, fallback, threshold_skip
  evidence_ref TEXT
);

-- Confirmations (consent records)
CREATE TABLE confirmations (
  evidence_id TEXT PRIMARY KEY,
  confirmed_ts INTEGER NOT NULL,
  actor TEXT NOT NULL,
  note TEXT
);

-- Used nonces (macaroon replay prevention)
CREATE TABLE used_nonces (
  token_id TEXT NOT NULL,
  nonce TEXT NOT NULL,
  expires_at INTEGER NOT NULL,
  PRIMARY KEY (token_id, nonce)
);

-- Schema migrations
CREATE TABLE schema_migrations (
  version INTEGER PRIMARY KEY,
  slug TEXT NOT NULL,
  applied_ts INTEGER NOT NULL
);
```

### 3.2 Hash Chain Integrity

Every `events` row includes `prev_hash` and `hash` columns forming a tamper-evident chain.

**Verification:** `russell verify-journal` audits the hash chain.

**Genesis:** First event uses `/etc/machine-id` or random 32-byte seed.

### 3.3 Proprioception (Self-Observation)

Nine self-vitals:

| Vital | Source | Rule |
|-------|--------|------|
| `sentinel_last_run_age_s` | journal `MAX(ts)` | Warn > 450s, Alert > 1800s |
| `journal_writer_stall_s` | write-append timing | Warn > 60s, Alert > 300s |
| `llm_p95_latency_ms` | help_session latency | Warn > 8000ms, Alert > 20000ms |
| `timer_drift_s` | cadence interval | Warn > target+20% |
| `help_error_rate_pct` | failed LLM calls / total | Warn > 20%, Alert > 50% |
| `hkask_mcp_reachable_ms` | MCP endpoint ping | Warn > 1000ms |
| `remote_discovery_latency_s` | remote skill registry lookup | Warn > 5s |
| `journal_chain_intact` | hash chain verification | Fail = `false` |
| `evidence_integrity_ok` | evidence bundle checksums | Fail = `false` |

**Reflex arcs:** `russell-proprio::reflex` evaluates threshold and rate breaches to propose interventions with cooldown enforcement.

### 3.4 CNS Spans

| Namespace | Covers |
|-----------|--------|
| `cns.probe.execute` | Probe execution |
| `cns.intervention.execute` | Intervention execution |
| `cns.skill.install` | Skill installation |
| `cns.skill.prune` | Skill pruning |
| `cns.skill.retire` | Skill retirement |
| `cns.journal.read` | Journal queries |
| `cns.evidence.export` | Evidence exports |
| `cns.jack.consult` | Jack consultations |

**Variety counters:** Track diversity of probes, interventions, skills.

**Algedonic alerts:** Trigger when variety deficit exceeds threshold.

---

## 4. Evidence Bundles

### 4.1 Bundle Structure

```
~/.local/state/harness/evidence/<evidence_id>/
├── soap.md              # Subjective / Objective / Assessment / Plan
├── skill.yaml           # Skill manifest
├── <probe-id>.json      # Probe outputs
├── <intervention-id>.json  # Intervention outputs
├── llm-transcript.jsonl # LLM round-trip
└── system-snapshot/     # dmesg.log, rocm-smi.json, etc.
```

### 4.2 Retention Policy

| Bundle Type | Retention |
|-------------|-----------|
| Help sessions | 90 days |
| Skill executions | 90 days |
| Archived bundles | Indefinite |

**Compaction:** `journal-compactor` skill removes expired bundles.

---

## 5. Focusing Assumptions

### FA-T1: IDRS-Only for Mutations

**Statement:** All mutations satisfy IDRS contract. No exceptions.

**Rationale:** Minimize mutation surface; ad-hoc execution is a liability.

**Evidence:** See §1.1 IDRS Contract.

---

### FA-O1: Journal Monitors Production; Tests Verify Correctness

**Statement:** Journal records production observations; tests verify correctness.

**Rationale:** Separate concerns — journal ≠ testing.

**Evidence:** See §3.1 Journal Schema.

---

## 6. Cross-References

| Category | Relation |
|----------|----------|
| Capability | Tokens governed by this spec |
| Observability | Audit spans for all security operations |
| Curation | Curation decisions are auditable security events |
| Trust | Security audit spans |
| Lifecycle | Health monitoring |

---

## 7. Completeness Checklist

- [x] STRIDE-lite analysis per component
- [x] IDRS contract defined
- [x] Risk band policy defined
- [x] Kill switches documented
- [x] Consent flow documented
- [x] Journal schema defined
- [x] Hash chain integrity verified
- [x] Proprioception vitals defined
- [x] CNS spans defined
- [x] Evidence bundle structure defined

---

## References

- hKask DDMVSS: `~/Clones/hKask/docs/architecture/DDMVSS.md` §5.5, §5.6
- ADR-0004 (SQLite journal)
- ADR-0015 (proprioception self-health)
- ADR-0021 (proprioception phase 2 reflex arcs)
- ADR-0032 (evidence bundle sealing)
