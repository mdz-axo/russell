# Magna Carta Verifier — KNOWLEDGE.md
# Context for Jack when running and interpreting Magna Carta verification.

## Purpose

This skill verifies the four Magna Carta principles (P1–P4) by running
structured assertion manifests against the Russell codebase and runtime.
Each assertion declares a claim, a verification method, and target code
paths. The verifier script runs each assertion and reports pass / fail /
gap status.

## Principles at a Glance

| Principle | Name | Core Claim |
|-----------|------|------------|
| P1 | Operator Sovereignty | The operator controls their data; every sovereign path is gated |
| P2 | Affirmative Consent | Default deny; consent is scoped, versioned, expiring, hierarchical |
| P3 | Generative Space | All generative settings exposed; operators curate, not the system |
| P4 | Clear Boundaries (OCAP) | Every access path goes through dual gates; tokens are unforgeable |

## How Assertions Work

Each principle manifest (e.g. `manifests/p1-operator-sovereignty.yaml`)
contains a list of assertions. Every assertion has:

- **id** — Short identifier (p1a, p1b, …)
- **name** — Human-readable assertion name
- **claim** — The invariant being verified (a single declarative sentence)
- **method** — How the verifier checks it (see below)
- **targets** — Code paths: crate, module, methods, and gate

The verifier script iterates assertions and runs the appropriate check
for each method.

## Verification Methods

| Method | What It Does |
|--------|-------------|
| `structural_audit` | Searches source code for the presence of gate calls (e.g. `require_sovereignty`) on target methods. Pass = gate found on every target method. |
| `behavioral_probe` | Attempts access without consent and checks that it is denied. Pass = access denied. |
| `absence_check` | Searches for prohibited patterns (admin overrides, hidden gates, engineer-mode toggles). Pass = pattern not found. |
| `resource_verification` | Checks that resource categories (sovereign / shared / public) are correctly declared and match the runtime configuration. Pass = categories consistent. |

Assertions that combine methods (e.g. `structural + behavioral`) must
pass both checks independently.

## How Jack Should Interpret Results

The verifier emits one JSON object per assertion:

```json
{
  "assertion_id": "p1a",
  "status": "pass" | "fail" | "gap",
  "method": "structural_audit",
  "findings": ["..."],
  "gaps": ["..."]
}
```

- **pass** — Assertion verified. No action needed.
- **fail** — Assertion violated. Escalate to operator. The finding
  describes what went wrong (e.g. "gate missing on method X").
- **gap** — Target code does not exist yet. This is expected during
  development; log it but do not escalate unless the operator asks.

When any assertion **fails**, Jack should:

1. Report the failure in chat: *"Magna Carta assertion p1a (sovereign_data_gated) failed: no require_sovereignty gate found on russell-journal::store_sample."*
2. Ask the operator how to proceed: *"Want me to open the file, or should we schedule a fix?"*
3. Never attempt to fix the code autonomously — this is a verification skill, not an intervention.

## When Verification Runs

| Trigger | When |
|---------|------|
| Start-up | Verification runs when Russell starts |
| Expiration | Consent grants expire → re-verification scheduled |
| Operator change | New consent, settings change, new LLM provider |
| Resource/service change | New skill version, inference provider, or model |

## Templates

Three Jinja2 templates support documentation and test generation:

- `verification-procedure.md.j2` — Describes how to verify each assertion type
- `verification-report.md.j2` — Reports findings, gaps, and status per principle
- `test-case.rs.j2` — Renders Rust test cases as code blocks for CI integration

Use `russell skill run magna-carta-verifier` to invoke the probes.