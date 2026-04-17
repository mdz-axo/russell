<!--
audience: everyone writing tests
last-reviewed: 2026-04-17
-->

# ADR-0011: Testing strategy — unit, snapshot, property, integration

- **Status:** Accepted
- **Date:** 2026-04-17
- **Deciders:** Project founders
- **Tags:** `testing`, `ci`

## Context

Russell mutates host state. Each mutation must satisfy IDRS
([`safety.md`](../standards/safety.md)). A test strategy that
only exercises happy paths would let the hard parts — rollback,
dry-run fidelity, proposal lifecycle — regress silently.

## Decision

Four test layers, each with a defined scope:

### 1. Unit tests

- Colocated `mod tests` at the bottom of each `.rs` file.
- Use the standard `#[test]` / `#[tokio::test]` attributes.
- Exercise a single function or type in isolation; no
  filesystem writes outside `tempfile`, no subprocess
  spawns outside a mockable trait.
- **Gate:** every public fallible function has at least one
  test covering the error path.

### 2. Snapshot tests (`insta`)

- For MCP tool input / output schemas, rendered SOAP
  bundles, manifest-validation error messages, journal
  `PRAGMA table_info` after all migrations.
- Review with `cargo insta review`; never `accept`
  blindly. Reviewed snapshots become part of the PR.
- **Gate:** every new MCP tool ships a snapshot of its
  schema and a representative response shape.

### 3. Property tests (`proptest`)

- For parsers and schema validators: skill manifest,
  rules TOML, MCP JSON-RPC frames.
- Strategies live next to the parser; minimize-on-
  failure is expected to produce human-readable counter-
  examples.
- **Gate:** any parser in a library crate has at least
  one property test that round-trips valid inputs.

### 4. Integration tests

- Under each crate's `tests/` directory.
- `russell-mcp` uses a synthetic stdio client to drive
  the server end-to-end (spawn the binary, feed JSON-RPC
  frames, assert responses).
- `russell-skills` uses a fake `SubprocessRunner` trait
  implementation to verify dispatcher behaviour without
  invoking real shell commands.
- `russell-doctor` uses a recorded LLM transcript
  fixture to verify triage flow is deterministic.
- **Gate:** every skill ships at least one integration
  test that exercises `dry_run: true` through
  `skill_dry_run` and snapshots the resulting SOAP.

### Non-goals (for now)

- **VM-based end-to-end tests** exist but are not PR-
  gating. They belong to Phase-3+ of the roadmap
  ([`cybernetic-health-harness.md` §20](../../cybernetic-health-harness.md)).
- **Fuzzing** (`cargo-fuzz`) is welcome on parsers but
  is not CI-gated.
- **Coverage thresholds** — we run `cargo llvm-cov`
  locally to spot holes but do not enforce a
  percentage. Rationale: enforcing a coverage floor
  tends to produce unit tests that do not test
  behaviour.

### CI surface (per PR)

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace` (via `nextest` when
  available)
- `cargo deny check`

## Consequences

### Positive

- Each test layer has a job; no "where does this go?"
  debate.
- Snapshot tests pin the MCP and journal surfaces,
  which are the contracts with agents and the
  operator.
- Property tests catch parser edge cases that unit
  tests miss.

### Negative / accepted costs

- Snapshot review is a habit; blind-accept is a
  review failure.
- `proptest` adds a small crate-load cost to CI.

### Neutral

- The split mirrors common Rust-project conventions.

## Alternatives considered

### Coverage-gated CI

Rejected. Produces tests that chase lines, not
behaviour.

### VM-based e2e as the primary test layer

Rejected for v1. The feedback loop is too slow for
PRs; remains valuable for phase milestones.

### Mutation testing (`cargo-mutants`)

Deferred. Interesting but operationally expensive;
may be adopted later with an ADR amendment.

## Implementation notes

- Fake subprocess runner trait:
  ```rust
  pub trait SubprocessRunner: Send + Sync {
      fn spawn(&self, argv: &[String], env: &Env) -> BoxFuture<'_, Result<SpawnResult>>;
  }
  ```
  Production impl wraps `tokio::process::Command`;
  test impl returns scripted results.
- LLM fixture files live under
  `crates/russell-doctor/tests/fixtures/llm/`; each
  is named for the symptom it exercises.
- Migration round-trip tests snapshot
  `PRAGMA table_info(...)` for every table, plus
  `sqlite_master` indices.

## References

- insta: https://insta.rs
- proptest: https://proptest-rs.github.io/proptest/
- nextest: https://nexte.st
- cargo-deny: https://embarkstudios.github.io/cargo-deny/
- [`../standards/safety.md`](../standards/safety.md) §7
