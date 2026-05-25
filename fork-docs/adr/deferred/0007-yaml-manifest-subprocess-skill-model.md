---
title: "ADR-0007: YAML Manifest + Subprocess Skills (Deferred)"
audience: [developers, architects]
last_updated: 2026-04-18
ddmvss_context: "skill"
ddmvss_artifact: "adr"
version: "1.0.0"
status: "Deferred"
---



> **Deferred.** This ADR's subject is outside the MVP boundary per
> [`../../specifications/MVP_SPEC.md`](../../specifications/MVP_SPEC.md) §5.
> It remains **Accepted** — when its phase opens, it ships this way.


<!--
audience: skill authors and dispatcher implementers
last-reviewed: 2026-04-17
-->

# ADR-0007: Skill model — YAML manifest + referenced subprocess scripts

- **Status:** Accepted
- **Date:** 2026-04-17
- **Deciders:** Project founders
- **Tags:** `skills`, `manifest`, `dispatcher`, `safety`

## Context

[`cybernetic-health-harness.md` §12.1](../../../cybernetic-health-harness.md)
sketches skills as YAML manifests that describe probes and
interventions, with referenced scripts under
`~/.local/share/harness/skills/<id>/scripts/`. The design
document's explicit intent is **"skills are data, not code"** —
core is small, the catalog can grow.

Possible skill models:

1. YAML manifest + shell/Python/binary subprocess scripts.
2. Rust plugins (dylib / wasm) loaded at runtime.
3. Embedded scripting language (Lua, Starlark).
4. Pure-data manifest with an opinionated DSL covering all
   allowed actions.

## Decision

**YAML manifest + referenced subprocess scripts.** Each skill
lives under `skills/<id>/` with:

```
skills/<id>/
├── manifest.yaml
├── scripts/            (optional; referenced by manifest)
│   ├── probe-foo.sh
│   └── intervene-bar.py
├── README.md           (human documentation)
└── LICENSE             (SPDX tag; see ADR-0014)
```

The manifest schema is defined by
[`../../templates/skill-manifest.yaml`](../../templates/skill-manifest.yaml);
canonical parsing lives in `russell-skills::manifest`.

Execution model:

1. The dispatcher (`russell-skills::dispatch`) loads all
   manifests at startup (poka-yoke: schema violations abort
   the load, with a rendered error naming the file and
   field).
2. Each probe or intervention's `cmd` is an **argv list**,
   not a shell string. Shell interpolation is explicit:
   `["bash", "-c", "dmesg -T | grep -i amdgpu"]`.
3. The dispatcher spawns the subprocess via
   `tokio::process::Command`, enforcing:
   - working directory = the skill's directory;
   - stdin = `/dev/null`;
   - stdout/stderr captured into the evidence bundle,
     truncated if they exceed configured limits;
   - environment scrubbed to a small allow-list (PATH,
     HOME, LANG, RUSSELL_DRY_RUN, and the dispatcher's
     own probe context vars);
   - timeout per step (default 120 s; overridable per
     step);
   - `CAP_*` unchanged, `NoNewPrivileges` inherited from
     the parent systemd unit.
4. The dispatcher translates LLM-selected **IDs** to manifest
   entries; the LLM never supplies commands
   ([ADR-0008](../0008-llm-triage-never-emits-shell.md)).

Manifest invariants (all enforced at load time):

- `id` is kebab-case, matches the containing directory name.
- Every probe has `risk: none`; every intervention has
  `risk: low | medium | high | critical`.
- Every intervention names exactly one of `rollback_id`,
  `rollback: none_needed`, or `rollback: reboot`.
- Rollback IDs must resolve within the same manifest.
- `applies_when` clauses use only keys / capabilities that
  the profile knows about (ADR-0006).
- `symptoms` are listed in the symptom catalog
  (`russell-skills::symptoms::CATALOG`). Extending the
  catalog requires a short ADR.

## Consequences

### Positive

- Skills are shareable as plain directories; a registry is
  a Git repo of them.
- No plugin loader, no FFI, no runtime code loading — the
  core stays small.
- Languages for scripts are the author's choice (bash,
  python, pre-built binary) as long as the subprocess
  contract is satisfied.
- Review diffs are human-readable YAML + ordinary scripts.

### Negative / accepted costs

- Process-spawn overhead per step. Acceptable at Russell's
  cadences; the dispatcher can batch probes within a SOAP
  run.
- Skill authors must follow per-language hygiene; there is
  no framework-level sandbox beyond systemd's. We accept
  that skill scripts can, in principle, do arbitrary
  things within the user's authority — the mitigations are
  the risk-band cap, the IDRS contract, review, and the
  honeymoon window.

### Neutral

- YAML was chosen because it matches the design document
  and is familiar to ops audiences. For future
  hermetic-schema work we may gain via a typed
  intermediate (TOML? JSON?); that is a later ADR.

## Alternatives considered

### Rust plugins (dylib)

Rejected. Dylib plugins bring ABI compatibility pain, lose
the "skills are data" property, and are harder to review
(binary diff). Would also push the charter toward shipping
compilers to skill authors.

### WASM components

Interesting but premature. Would require a runtime (wasmtime),
a component-model ABI, and a host-call surface for systemd /
subprocess semantics. The complexity cost exceeds the
benefit at v1.

### Starlark / Lua DSL

Rejected. A DSL is a code-loading mechanism in disguise
without the review benefits of plain shell/Python. It also
requires sandboxing work that subprocess isolation gives us
for free.

### Pure-data action DSL (no scripts)

Rejected. The action surface Russell needs is open-ended
(parse `rocm-smi` JSON, grep dmesg, compare two file
fingerprints). Encoding all of that in YAML would reinvent
shell poorly.

## Implementation notes

- Manifest validation uses `serde_yaml` into typed structs
  plus a post-parse validator.
- The dispatcher refuses to load a manifest whose scripts
  directory contains executable files not referenced by
  any `cmd:`. Rationale: discourages dead code in skills
  and keeps review surface honest.
- A `skills/self/` subdirectory hosts proprioception
  skills (`scope: self`); same schema, same dispatcher,
  but journaled into `proprio_events`
  ([ADR-0015](../0015-proprioception-self-health.md)).

## References

- [`cybernetic-health-harness.md` §12](../../../cybernetic-health-harness.md)
- [`../../templates/skill-manifest.yaml`](../../templates/skill-manifest.yaml)
- [`../../standards/safety.md`](../../standards/safety.md)
- [ADR-0008](../0008-llm-triage-never-emits-shell.md)
- [ADR-0014](0014-skill-manifest-licensing.md)
