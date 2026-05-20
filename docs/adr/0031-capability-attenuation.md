---
title: "ADR-0031 — Capability Attenuation for Skills"
audience: [developers, architects, security reviewers]
last_updated: 2026-05-19
togaf_phase: "G"
version: "1.0.0"
status: "Active"
---

# ADR-0031 — Capability Attenuation for Skills

<!-- TOGAF_DOMAIN: Governance — Security -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-19 -->

## Context

The adversarial multi-perspective review (2026-05-19) identified weakness S1:

> **S1 — No capability attenuation** — skills have full access to env, no sandboxing.
> Subprocess dispatcher trusts manifest authors. JR-6 reuse over dependency.

Russell skills execute as subprocesses with access to environment variables inherited from the parent process. This creates a security risk:

1. **Secret exfiltration** — A malicious skill could access `RUSSELL_DOCTOR_API_KEY`, `OPENROUTER_API_KEY`, or other secrets from the environment and exfiltrate them via network calls or log output.

2. **Environment poisoning** — Skills could modify environment variables that affect other skills or the harness itself.

3. **Trust assumption** — The dispatcher currently trusts all manifest authors, but skills may be contributed by third parties or compromised after installation.

The safety scanner runs at install time but doesn't enforce runtime capability boundaries.

## Decision

Implement capability attenuation at the skill manifest level:

1. **`allowed_env_keys` field** — Add `allowed_env_keys: Vec<String>` to the `Safety` struct in skill manifests. Skills declare which environment variables they need.

2. **`needs_network` flag** — Add `needs_network: bool` to the `Safety` struct. Skills declare whether they require network access.

3. **Dispatcher enforcement** — The `Dispatcher` combines the default `ENV_ALLOWLIST` with skill-specific `allowed_env_keys` before spawning subprocesses. Only declared variables are passed.

4. **Default: empty** — Skills without `allowed_env_keys` get no environment variables (except the restrictive PATH).

5. **Load at dispatch** — The CLI loads the skill manifest before dispatch and sets `dispatcher.allowed_env_keys` from the skill's `Safety` config.

## Consequences

### Positive

- **Secret protection** — Skills cannot access secrets they don't declare, preventing exfiltration.

- **Defense in depth** — Runtime enforcement complements install-time safety scanning (Schneier principle).

- **Capability separation** — Skills only get capabilities they explicitly declare (Miller principle).

- **Audit trail** — Manifest declares what env vars the skill needs, making security review easier.

- **Air-gapped enforcement** — `needs_network: true` skills can be blocked in air-gapped environments.

### Negative

- **Manifest verbosity** — Skills must explicitly declare env vars, adding manifest lines.

- **Migration burden** — Existing skills that rely on inherited env vars must update their manifests.

- **False negatives** — Skills may forget to declare needed env vars, causing runtime failures.

### Neutral

- **Backward compatible** — Skills without `allowed_env_keys` get empty list (secure default).

- **No breaking changes** — Existing skills continue to work with minimal environment.

## Implementation

### Schema Changes

```yaml
safety:
  max_auto_risk: low
  require_human_for: []
  allowed_env_keys: ["HOME", "LANG"]  # Task 3.1: new field
  needs_network: false                 # Task 3.1: new field
```

### Code Changes

| File | Change |
|---|---|
| `russell-skills/src/lib.rs` | Add `allowed_env_keys`, `needs_network` to `Safety` and `RawSafety` structs |
| `russell-skills/src/dispatch.rs` | Add `allowed_env_keys` field to `Dispatcher`, combine with `ENV_ALLOWLIST` at dispatch |
| `russell-cli/src/commands/help.rs` | Load skill with `load_single()`, set dispatcher's `allowed_env_keys` |
| `russell-cli/src/commands/chat/execute.rs` | Load skill with `load_single()`, set dispatcher's `allowed_env_keys` |

### Exported Functions

- `russell_skills::load_single(skill_dir: &Path) -> Result<SSkill, LoadError>` — Load a single skill manifest for CLI use.

### Test Coverage

- All existing tests pass with new fields initialized to empty defaults
- Env filtering verified in dispatch tests

## Compliance

| Principle | Compliance |
|---|---|
| **JR-2** (Observe > Recommend > Act) | Dispatcher observes skill's declared env needs, enforces at runtime |
| **JR-6** (Reuse over dependency) | Env filtering reuses existing dispatcher infrastructure |
| **Schneier** (Defense in depth) | Runtime capability attenuation complements install-time safety scanner |
| **Miller** (Capability separation) | Skills only get explicitly declared capabilities (env vars) |

## Future Work

- **Network enforcement** — Implement actual network blocking for `needs_network: false` skills (requires sandboxing or firewall rules).

- **Env validation** — Warn if skill tries to access undeclared env vars (requires monitoring).

- **Skill migration** — Provide tool to audit existing skills and suggest `allowed_env_keys` based on actual usage.

- **Safety scanner integration** — Update safety scanner to flag skills with unrestricted env access patterns.

## References

- Adversarial Review Action Plan §3.1 (Task S1)
- `docs/standards/safety.md` §8 (LLM and safety)
- `crates/russell-skills/src/dispatch.rs` (subprocess dispatcher)