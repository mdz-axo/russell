---
title: "ADR-0023: Lift ADR-0007 Deferral — Phase 3 Skills and Dispatch"
audience: [developers, architects]
last_updated: 2026-05-09
togaf_phase: "H — Change Management"
version: "1.0.0"
status: "Accepted"
---

<!-- TOGAF_DOMAIN: Change Management -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Accepted -->
<!-- LAST_UPDATED: 2026-05-09 -->

# ADR-0023: Lift ADR-0007 Deferral — Phase 3 Skills and Dispatch

- **Status:** Accepted
- **Date:** 2026-05-09
- **Deciders:** Project operator
- **Tags:** `skills`, `manifest`, `dispatcher`, `phase-3`

## Context

[ADR-0007](deferred/0007-yaml-manifest-subprocess-skill-model.md) was
accepted but deferred on 2026-04-17. Its subject — YAML manifest +
subprocess skill model — was explicitly outside the MVP boundary.

Phase 0 through Phase 2 are now complete:

- Phase 0: Skeleton, read-only observation loop
- Phase 1: MVP Doctor (`russell jack`) with LLM triage
- Phase 1b: Install artifacts + systemd units
- Phase 1c: 20-day soak (closed per ADR-0018)
- Phase 2: Observation sharpened — rule engine, EWMA baselines,
  Markdown memory layer, proprioception, sample summaries

All success criteria for phases 0–2 are met. The observation
foundation is stable. Phase 2's rule engine (ADR-0020, TOML
thresholds) and EWMA baselines (30-day percentiles) prove that
Russell can detect anomalies — but he still cannot respond.

Per JR-2: "Observe > Recommend > Act." Phase 3 enables the third
rung of the ladder.

### Evidence that Phase 2 is stable enough to lift the deferral

- 20+ days of unattended Sentinel operation with ~99.95%
  reliability (ADR-0018).
- Rule engine emits `warn`/`alert`/`crit` events on threshold
  breaches.
- EWMA baselines provide 30-day rolling p50/p95/p99 per probe.
- Markdown memory layer gives a human-readable audit surface.
- 81 tests pass; clippy clean.

### What Phase 3 does NOT change

- **JR-3 still holds.** The LLM selects manifest IDs; it never
  emits shell commands. Poka-yoke rejects unknown IDs.
- **IDRS is non-negotiable.** Every skill intervention satisfies
  Idempotent / Dry-run / Rollback / Structured-log.
- **JR-1 still holds.** The first skill is deliberately tiny.
  No skill registry, no remote loading, no dynamic discovery
  beyond `~/.local/share/harness/skills/`.

## Decision

Lift ADR-0007's deferral. Implement Phase 3 in three increments:

### Phase 3A — Manifest loader + dispatcher (this ADR)

1. `russell-skills::manifest` — YAML manifest parser with typed
   structs and post-parse validation per ADR-0007 §Implementation.
2. `russell-skills::dispatch` — subprocess runner with env
   scrubbing, timeout, stdout/stderr capture.
3. `russell skill list` — CLI verb that enumerates loaded skill
   manifests.
4. `russell skill run <id>` — CLI verb that runs a probe or
   intervention. Dry-run mode (`--dry-run`) prints what would
   happen without executing.

### Phase 3B — First skill: `gpu-doctor`

1. A skill manifest at `skills/gpu-doctor/manifest.yaml`.
2. A probe script `rocm-smi-probe.sh` that parses
   `rocm-smi --showmeminfo vram --json` and `rocm-smi -t --json`.
3. An intervention script `reset-gpu.sh` that resets the GPU via
   `sudo rocm-smi --reset` (risk: medium, requires confirmation).

### Phase 3C — Doctor integration

1. `russell jack` consults the skill manifest catalog for
   symptom-to-skill mapping.
2. When the LLM identifies a symptom (e.g., "GPU VRAM at 98%"),
   Jack can suggest: "I can run `gpu-doctor.assess` to check.
   `russell skill run gpu-doctor/assess`."
3. The LLM never dispatches — it only proposes IDs. The operator
   runs the ID or not.

### What this ADR does NOT authorise

- Auto-dispatch from the LLM (requires ADR-0008 amendment).
- Remote skill registry (deferred ADR-0007 part).
- `skills/self/` proprioception skills (deferred until the
  meta-Sentinel is fully designed).

## Consequences

### Positive

- Russell can finally respond to detected problems — completing
  the JR-2 ladder.
- Skills are shareable; the `gpu-doctor` skill immediately
  benefits any ROCm workstation.
- The skill catalog separates data from code; new skills don't
  require Russell releases.

### Negative / accepted costs

- Subprocess spawns add latency per probe. Acceptable given
  cadences (5 min+).
- Skill scripts are user-authored; Russell cannot guarantee
  their safety beyond the IDRS contract and risk-band caps.
- The `serde_yaml` dependency adds ~200 KB to the binary.
  Acceptable per JR-1 — the manifest loader earns its place
  by enabling the entire skill ecosystem.

### Neutral

- The `russell-skills` crate graduates from Phase-0 stub to
  active implementation. Its dependency DAG was already correct
  (depends on `russell-core` only).

## Implementation Notes

1. Manifest types live in `russell-skills/src/manifest.rs`.
2. Dispatch types live in `russell-skills/src/dispatch.rs`.
3. CLI verbs: `russell skill list`, `russell skill run <id>`.
4. Skills directory: `~/.local/share/harness/skills/`.
5. First skill: `skills/gpu-doctor/` with `manifest.yaml` and
   `scripts/rocm-smi-probe.sh`.
6. Validation: `validate()` on every manifest after parsing,
   returning `Vec<ValidationError>` (empty = valid).
7. Dry-run: `--dry-run` prints the command that would run without
   spawning. `RUSSELL_DRY_RUN=1` does the same globally.

## References

- [ADR-0007](deferred/0007-yaml-manifest-subprocess-skill-model.md) — the skill model
- [ADR-0008](0008-llm-triage-never-emits-shell.md) — JR-3 mechanism
- [JR-2](../architecture/PRINCIPLES_CATALOG.md) — Observe > Recommend > Act
- [IDRS](../standards/safety.md) — the mutation contract
- [`MVP_SPEC.md`](../specifications/MVP_SPEC.md) §5 — what MVP does NOT do (now updated)
