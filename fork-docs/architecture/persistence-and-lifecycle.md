---
title: "Russell Persistence and Lifecycle"
audience: [architects, developers, agents]
last_updated: 2026-05-25
version: "1.0.0"
status: "Active"
domain: "Cross-cutting"
ddmvss_categories: [persistence, lifecycle]
---

# Russell Persistence and Lifecycle

**Purpose:** Define Russell's persistence model (journal, profile, evidence) and lifecycle model (bootstrap, evolution, deprecation).

**Focusing assumption:** Local-first, Git backup — no sync.

---

## 1. Persistence Model

### 1.1 Storage Architecture

| Component | Technology | Location |
|-----------|-----------|----------|
| Journal | SQLite + WAL | `~/.local/state/harness/journal.db` |
| Profile | JSON | `~/.local/state/harness/profile.json` |
| Evidence | Filesystem | `~/.local/state/harness/evidence/` |
| Skills | Filesystem | `~/.local/share/harness/skills/` |
| Config | Filesystem | `~/.config/harness/` |

### 1.2 Journal Schema

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

### 1.3 Profile Schema

```json
{
  "schema": "russell.profile.v1",
  "profile_id": "<ulid>",
  "authored_at": "2026-04-17T03:30:00Z",
  "host": {
    "os": { "family": "linux", "distro": "ubuntu", "version": "25.10", "kernel": "6.17.0-20-generic" },
    "chassis": { "vendor": "Framework", "product": "Laptop 16 (AMD Ryzen AI 300 Series)", "bios": "..." },
    "cpu": { "vendor": "AuthenticAMD", "model": "AMD Ryzen AI 9 HX 370", "family": 26, "cores": 12, "threads": 24 },
    "memory_mib": 93184,
    "swap_mib": 8192
  },
  "gpus": [
    { "pci": "c4:00.0", "vendor_id": "0x1002", "name": "Radeon RX 7700S", "gfx": "gfx1102", "role": "compute" },
    { "pci": "c5:00.0", "vendor_id": "0x1002", "name": "Radeon 890M", "gfx": "gfx1150", "role": "display" }
  ],
  "storage": [
    { "dev": "/dev/nvme0n1", "size_bytes": 3600000000000, "model": "...", "fs_primary": "ext4" }
  ],
  "toolchains": {
    "rust": { "rustup_version": "1.28.2", "toolchains": ["stable", "nightly", "1.75", "1.90", "1.94.1"] },
    "node": { "manager": "nvm", "version": "24.14" },
    "container": { "podman": "5.4.2" },
    "ai": { "ollama": "0.20.4", "rocm": "7.2.0" }
  },
  "editors": { "vscodium": "1.105.17075", "zed": "0.233.1" },
  "bootstrap_completed_at": "2026-04-17T03:30:00Z",
  "honeymoon_ends_at": "2026-05-17T03:30:00Z",
  "capabilities": ["rocm", "lvfs", "polkit", "systemd-user"],
  "network": { "llm_egress": false, "skill_registry_egress": false }
}
```

### 1.4 Evidence Bundle Structure

```
~/.local/state/harness/evidence/<evidence_id>/
├── soap.md              # Subjective / Objective / Assessment / Plan
├── skill.yaml           # Skill manifest
├── <probe-id>.json      # Probe outputs
├── <intervention-id>.json  # Intervention outputs
├── llm-transcript.jsonl # LLM round-trip
└── system-snapshot/     # dmesg.log, rocm-smi.json, etc.
```

### 1.5 Encryption

| Component | Encryption |
|-----------|-----------|
| Journal | None (local-first) |
| Profile | None (local-first) |
| Evidence | None (local-first) |
| ACP tokens | HMAC-SHA256 |

**Rationale:** Local-first, no sync. Encryption adds complexity without benefit for single-host, single-operator tool.

---

## 2. Lifecycle Model

### 2.1 Bootstrap Sequence

1. Initialize journal database (`journal.db`)
2. Load Russell vocabulary (sentinel, journal, jack, skill, etc.)
3. Register built-in skills (okapi-watcher, sysadmin, etc.)
4. Generate profile (`profile.json`)
5. Start honeymoon period (30 days)
6. Enable systemd timers

**Command:** `./packaging/bin/install.sh`

### 2.2 Evolution Rules

| Rule | Description |
|------|-------------|
| **Git-only versioning** | SHA-based, no SemVer |
| **Forward-only migrations** | No rollback |
| **ADR lifecycle** | Proposed → Accepted → Superseded → Deprecated |
| **Skill lifecycle** | Discovered → Evaluated → Installed → Active → Stale → Deprecated → Retired |

### 2.3 ADR Lifecycle

```
Proposed → Accepted → Superseded by ADR-MMMM
                  ↓
              Deprecated (decision no longer applies)
```

**Status transitions:**
- **Proposed:** ADR is under review
- **Accepted:** ADR is locked; code implements it
- **Superseded:** New ADR replaces this one
- **Deprecated:** Decision no longer applies (rare)

**Supersession:** When ADR-MMMM supersedes ADR-NNNN:
1. ADR-MMMM includes `Supersedes: ADR-NNNN` in frontmatter
2. ADR-NNNN includes `Superseded by: ADR-MMMM` in frontmatter
3. ADR-NNNN content is not deleted (historical record)

### 2.4 Skill Lifecycle

```
Discovered → Evaluated → Installed → Active → Stale → Deprecated → Retired
```

**Status transitions:**
- **Discovered:** Skill found in registry or manually added
- **Evaluated:** Safety scanner passed, quality score computed
- **Installed:** Skill files copied to `~/.local/share/harness/skills/`
- **Active:** Skill is loaded and available
- **Stale:** Skill not updated in 180 days
- **Deprecated:** Skill marked as deprecated (files remain)
- **Retired:** Skill removed from registry and disk

**Commands:**
- `russell skill install <id>` → Installed → Active
- `russell skill prune <id>` → Active → Deprecated
- `russell skill retire <id>` → Deprecated → Retired (deletes files)

### 2.5 Deprecation Policy

**Principle:** Prefer deletion over deprecation (JR-1: austere by default).

**Process:**
1. Delete code / files
2. Remove from registry
3. Emit `cns.skill.retired` span
4. Update ADR if applicable

**Rationale:** Deprecation adds cognitive load. Deletion is cleaner.

---

## 3. Backup Strategy

### 3.1 Git Backup

| Component | Backup Method |
|-----------|---------------|
| Journal | Not backed up (regenerable from probes) |
| Profile | Not backed up (regenerable from hardware) |
| Evidence | Not backed up (ephemeral) |
| Skills | Git repository |
| Config | Git repository |
| ADRs | Git repository |

**Rationale:** Local-first, no sync. Git is the archive.

### 3.2 Reset Procedure

```bash
rm -rf ~/.local/state/harness/
./packaging/bin/install.sh
```

**Result:** Clean slate. No orphaned state.

---

## 4. Focusing Assumptions

### FA-P1: Local-First, Git Backup — No Sync

**Statement:** All state is local. Git is the archive. No cross-machine sync.

**Rationale:** User sovereignty; no cross-machine complexity.

**Evidence:** See §3 Backup Strategy.

---

### FA-L1: Git-Only Versioning — SHA-Based, No SemVer

**Statement:** Versioning is Git SHA only. No SemVer.

**Rationale:** Minimize versioning surface; Git is archive.

**Evidence:** See §2.2 Evolution Rules.

---

## 5. Cross-References

| Category | Relation |
|----------|----------|
| Domain | Entities stored in journal |
| Trust | Encryption governed by keystore |
| Observability | Journal records observations |
| Composition | Registry entries evolve |
| Curation | Curator initialized during bootstrap |

---

## 6. Completeness Checklist

- [x] Every domain entity has storage schema
- [x] Journal schema defined
- [x] Profile schema defined
- [x] Evidence bundle structure defined
- [x] Bootstrap sequence defined
- [x] Evolution rules documented
- [x] ADR lifecycle documented
- [x] Skill lifecycle documented
- [x] Deprecation policy specified
- [x] Backup strategy defined
- [x] Reset procedure documented

---

## References

- hKask DDMVSS: `~/Clones/hKask/docs/architecture/DDMVSS.md` §5.7, §5.8
- ADR-0004 (SQLite journal)
- ADR-0006 (profile abstraction)
- ADR-0024 (skill registry, workshop, lifecycle)
