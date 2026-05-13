---
title: "ADR-0006: Profile Abstraction"
audience: [developers, architects]
last_updated: 2026-04-18
togaf_phase: "H"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Change Management -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-04-18 -->

<!--
audience: bootstrap / profile contributors
last-reviewed: 2026-04-17
-->

# ADR-0006: Profile abstraction — `profile.json` as the single source of truth

- **Status:** Accepted
- **Date:** 2026-04-17
- **Deciders:** Project founders
- **Tags:** `profile`, `bootstrap`, `schema`

## Context

[`cybernetic-health-harness.md` §13](../../cybernetic-health-harness.md)
describes a bootstrap that captures hardware, OS, and installed
toolchains into a "patient chart." Every tier, the Doctor, and
the Sentinel need that chart. Without a single serialized
artifact:

- Each subsystem would re-probe what it needs, duplicating work
  and drifting over time.
- `applies_when` clauses in skill manifests would have no
  stable vocabulary to match against.
- The weekly digest would lack a canonical "what is this
  machine" header.

## Decision

The machine profile is a **single JSON file** at
`~/.local/state/harness/profile.json`, with schema
`russell.profile.v1`. It is:

1. **Authored only by the Bootstrap** (state machine in
   `russell-doctor::bootstrap`).
2. **Read by everything else** — tiers, Sentinel, Doctor,
   skill dispatcher, MCP server, digest renderer.
3. **Refreshed on demand** via `russell bootstrap` (which also
   renews the honeymoon window) or on user request via the
   `bootstrap_probe` / `bootstrap_apply` MCP tools.

Top-level shape (abbreviated; full JSON Schema under
`crates/russell-core/src/profile/schema/`):

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
    { "pci": "c5:00.0", "vendor_id": "0x1002", "name": "Radeon 890M",     "gfx": "gfx1150", "role": "display" }
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

Invariants:

- **Schema version in the file.** Readers that do not
  recognize the version refuse to load and log
  `profile.schema.unknown`. Russell does not silently
  downgrade.
- **`profile_id` is stable** across bootstrap runs on the
  same physical machine (derived from a stable fingerprint:
  BIOS serial + primary disk serial + MAC of the primary
  NIC, hashed). A fresh `profile_id` means "this is a
  different machine" and triggers a honeymoon reset.
- **`capabilities`** is a closed vocabulary managed in
  `russell-core::profile::capabilities`. New capabilities
  require an ADR.

Skill manifests reference the profile via the
`applies_when` block; the dispatcher evaluates those clauses
against the loaded profile at skill-load time, not at
intervention-time.

## Consequences

### Positive

- One well-known path any component can read.
- JSON Schema gives us validation, proptest inputs, and
  snapshot stability.
- Capability flags let us gate features cleanly:
  "this skill needs `rocm`" instead of "grep for rocminfo
  output."

### Negative / accepted costs

- A stale profile is possible; the Bootstrap must run on
  significant hardware changes. The quarterly
  `quarterly/firmware-audit` module nudges a re-run if
  DMI data changes.
- JSON is not the most ergonomic schema format, but it is
  the one every consumer — including the MCP
  `profile_read` tool — can handle without extra
  dependencies.

### Neutral

- The file is human-readable; `cat profile.json | jq .`
  is a first-line triage tool.

## Alternatives considered

### TOML

Rejected. JSON is a better MCP wire-format match (same
encoding as tool I/O), and nested lists/objects in TOML
become cumbersome for the GPU / storage sections.

### YAML

Rejected. YAML's complexity (anchors, tags, implicit
types) does not pay for itself in a single-file
single-writer artifact.

### Multiple files (one per subsystem)

Rejected. Creates coordination problems (who writes what
when?) and makes the honeymoon clock harder to pin to a
single atomic write.

### A table in `journal.db`

Rejected. The profile is a snapshot, not a time series;
keeping it out of the DB lets us `cat` and `jq` without
opening SQLite.

## Implementation notes

- The Bootstrap writes atomically: write to `profile.json.tmp`,
  `fsync`, `rename` over `profile.json`.
- A `profile_id` change is an "important event": emit a
  `profile.bootstrap_id_changed` journal event with the old
  and new IDs.
- `profile.json` is included in evidence bundles by
  reference (the bundle stores the `profile_id`, and the
  renderer looks the file up).

## References

- JSON Schema: https://json-schema.org
- [`cybernetic-health-harness.md` §13](../../cybernetic-health-harness.md)
- [`../architecture/overview.md`](../architecture/overview.md) §3.1
