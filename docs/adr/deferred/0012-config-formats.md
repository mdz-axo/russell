---
title: "ADR-0012: Config Formats (Deferred)"
audience: [developers, architects]
last_updated: 2026-04-18
togaf_phase: "H — Change Management"
version: "1.0.0"
status: "Accepted — Deferred"
---

<!-- TOGAF_DOMAIN: Change Management -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Accepted — Deferred -->
<!-- LAST_UPDATED: 2026-04-18 -->

> **Deferred.** This ADR's subject is outside the MVP boundary per
> [`../../specifications/MVP_SPEC.md`](../../specifications/MVP_SPEC.md) §5.
> It remains **Accepted** — when its phase opens, it ships this way.
> See [`README.md`](README.md) for the deferral register.

<!--
audience: config-file authors and parsers
last-reviewed: 2026-04-17
-->

# ADR-0012: Config formats — TOML for operator config, YAML for skills, JSON for profile/wire

- **Status:** Accepted
- **Date:** 2026-04-17
- **Deciders:** Project founders
- **Tags:** `config`, `schema`

## Context

Russell has three categories of config-like content:

1. **Operator-editable config** — knobs, rule overrides,
   per-module pauses: `~/.config/harness/...`.
2. **Skill manifests** — data describing probes,
   interventions, and safety constraints: `skills/<id>/
   manifest.yaml`.
3. **Machine / runtime artifacts** — `profile.json`,
   MCP wire, evidence bundle metadata.

A single format for all three has real ergonomic costs.
Mixing formats arbitrarily has review costs. We need a
declared rule.

## Decision

| Category | Format | Why |
|---|---|---|
| Operator config | **TOML** | Rust-ecosystem default; small, strict types, clean comments; Cargo-familiar. |
| Skill manifests | **YAML** | Design document's choice; supports nested lists ergonomically; familiar to ops audiences. |
| Machine artifacts / MCP wire / evidence metadata | **JSON** | Matches MCP wire format; easily `jq`-able; schema-validatable. |

Concretely:

- `~/.config/harness/config.toml` — main operator config.
- `~/.config/harness/rules.d/*.toml` — per-probe rule
  overrides.
- `skills/<id>/manifest.yaml` — skill manifests.
- `~/.local/state/harness/profile.json` — machine profile
  (ADR-0006).
- Evidence bundle's per-step logs — JSON one-per-file.
- MCP tool I/O — JSON (non-negotiable; it is the wire).

Parser crates:

- `serde` + `toml` for TOML.
- `serde` + `serde_yaml` for YAML.
- `serde` + `serde_json` for JSON.

## Consequences

### Positive

- Each consumer reaches for the format that suits its
  review style.
- Schema-validation tooling (JSON Schema for JSON,
  typed `serde` structs for all) is available
  uniformly.
- The operator never writes YAML to tune Russell; YAML
  is reserved for skill authoring, which has a
  steeper review process.

### Negative / accepted costs

- Three formats in one repo. Mitigated by the
  category table above and by review.
- `serde_yaml` has had maintenance concerns in the
  past; if the ecosystem consolidates on a successor
  we will migrate with an amending ADR.

### Neutral

- All three formats are first-class in `serde`.

## Alternatives considered

### TOML everywhere

Rejected for skill manifests. TOML's weaker support
for deeply nested structures (the manifest has probes
→ intervention-plan → evaluation → after) would hurt
readability.

### YAML everywhere

Rejected for operator config. YAML's implicit typing
and indentation sensitivity are footguns the operator
does not need. TOML's explicit types are a better
fit.

### JSON everywhere

Rejected for humans. JSON has no comments (JSON5
notwithstanding) and is visually noisy for
ops-facing files.

### Custom DSL

Rejected. See [ADR-0007](0007-yaml-manifest-subprocess-skill-model.md)
for the longer argument.

## Implementation notes

- Config / rule / manifest schemas each live in a
  `schema/` module of their owning crate.
- Every schema version is string-tagged at the top of
  the file (`schema = "russell.rule.v1"` for TOML,
  `$schema: "russell.skill.v1"` for YAML,
  `"schema": "russell.profile.v1"` for JSON).
- Unknown schema versions refuse to load; there is no
  silent downgrade.

## References

- `toml`, `serde_yaml`, `serde_json` docs.
- [ADR-0006](../0006-profile-abstraction.md) — JSON for
  profile.
- [ADR-0007](0007-yaml-manifest-subprocess-skill-model.md)
  — YAML for manifests.
