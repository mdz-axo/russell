---
title: "ADR-0032 — Evidence Bundle Sealing"
audience: [developers, architects, security reviewers, auditors]
last_updated: 2026-05-19
ddmvss_context: "journal"
ddmvss_artifact: "adr"
version: "1.0.0"
status: "Active"
---


# ADR-0032 — Evidence Bundle Sealing


## Context

The adversarial multi-perspective review (2026-05-19) identified weakness S3:

> **S3 — No cryptographic evidence sealing** — evidence bundles are plain JSON.
> IDRS requires structured log, not signatures. MVP scope.

Russell's IDRS (Idempotent/Dry-run/Rollback/Structured-log) contract requires every skill execution to write an evidence bundle containing:
- `stdout.txt` — captured standard output
- `stderr.txt` — captured standard error (if non-empty)
- `event.json` — the journal event record

However, these bundles have no integrity protection. An attacker with filesystem access could:
1. Modify evidence files to hide skill failures
2. Plant false evidence to frame legitimate skills
3. Tamper with audit trails without detection

This violates the "Structured log" requirement of IDRS — the log is structured but not tamper-evident.

## Decision

Implement evidence bundle sealing with SHA-256 hashes:

1. **Hash computation** — When writing evidence bundles, compute SHA-256 hashes of `stdout.txt` and `stderr.txt`.

2. **Hash injection** — Add `stdout_sha256` and `stderr_sha256` fields to the event's `outputs` map before writing `event.json`.

3. **Manifest file** — Write `manifest.json` containing:
   - Version identifier
   - Creation timestamp (RFC 3339)
   - Per-file metadata: SHA-256 hash, size in bytes

4. **Journal audit** — Hashes in `event.json` provide journal-level audit trail; filesystem evidence can be verified against hashes.

5. **Verification deferred** — On-read verification (`evidence_tampered` event) is deferred to Phase 3; foundation is built.

## Consequences

### Positive

- **Tamper evidence** — Any modification to evidence files is detectable via hash mismatch.

- **Audit trail** — Hashes in journal events provide immutable record even if filesystem evidence is deleted.

- **Forensic integrity** — Evidence bundles can be used in post-mortem analysis with cryptographic assurance.

- **IDRS compliance** — The "Structured log" requirement now includes integrity protection.

- **Schneier principle** — Defense in depth: journal hashes protect even if evidence directory is compromised.

### Negative

- **Storage overhead** — Each evidence bundle now includes `manifest.json` (~500 bytes) plus hash fields in `event.json`.

- **Write latency** — SHA-256 computation adds ~1-2ms per evidence bundle (negligible for typical skill runs).

- **No encryption** — Hashes provide integrity, not confidentiality. Evidence contents remain plaintext.

### Neutral

- **Backward compatible** — Existing evidence bundles without hashes continue to work; new bundles include hashes.

- **No breaking changes** — Verification is opt-in; existing code paths continue to function.

## Implementation

### Evidence Bundle Structure

```
evidence/skills/<skill-id>/<step-id>/<timestamp>/
├── stdout.txt          # Captured stdout
├── stderr.txt          # Captured stderr (if non-empty)
├── event.json          # Journal event with hash outputs
└── manifest.json       # NEW: File manifest with hashes
```

### Manifest Format

```json
{
  "version": "1.0",
  "created_at": "2026-05-19T18:30:00.000000000Z",
  "files": {
    "stdout.txt": {
      "sha256": "abc123...",
      "size_bytes": 1024
    },
    "stderr.txt": {
      "sha256": "def456...",
      "size_bytes": 512
    },
    "event.json": {
      "note": "Self-hash not included"
    }
  }
}
```

### Code Changes

| File | Change |
|---|---|
| `russell-skills/src/dispatch.rs` | Add `sha2`, `hex` dependencies; modify `write_evidence()` to compute hashes and write manifest |
| `russell-skills/Cargo.toml` | Add `sha2 = "0.10"`, `hex = "0.4"` |

### Dependencies

- `sha2` (0.10) — SHA-256 hash computation
- `hex` (0.4) — Hash encoding for JSON storage

### Test Coverage

- All existing dispatch tests pass with new hash fields
- Evidence bundles now include `manifest.json` and hash outputs

## Compliance

| Principle | Compliance |
|---|---|
| **JR-2** (Observe > Recommend > Act) | Evidence sealing observes execution output, produces tamper-evident record |
| **JR-6** (Reuse over dependency) | Uses workspace `sha2`/`hex` crates, no new dependencies |
| **JR-7** (Persistence auditable) | Evidence bundles now have cryptographic integrity protection |
| **Schneier** (Defense in depth) | Hashes in journal protect even if evidence directory is compromised |

## Future Work

- **On-read verification** — Implement `verify_evidence(evidence_dir: &Path) -> Result<bool>` function that:
  - Reads `manifest.json`
  - Recomputes hashes of `stdout.txt`, `stderr.txt`
  - Returns `false` if any hash mismatches
  - Emits `evidence_tampered` event on mismatch

- **Chain of custody** — Extend manifest to include operator identity, session ID, and parent event hash for chain-of-custody tracking.

- **Signature support** — Optional GPG/ed25519 signing of manifest for non-repudiation (requires key management).

- **PERSISTENCE_CATALOG.md update** — Document evidence bundle format in §2.3.

## References

- Adversarial Review Action Plan §3.3 (Task S3)
- `docs/standards/safety.md` §6 (IDRS contract)
- `docs/specifications/PERSISTENCE_CATALOG.md` §2 (Evidence bundles)
- `crates/russell-skills/src/dispatch.rs:903-962` (Evidence sealing implementation)
