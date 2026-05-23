---
title: "Contributing to Russell"
audience: [developers, contributors]
last_updated: 2026-05-14
togaf_phase: "Preliminary"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Governance -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-14 -->

# Contributing to Russell

This document covers **mechanics**: dev environment, tests, local MCP
wiring. For content-of-contributions rules (medical metaphor, IDRS,
adding a skill, adding an MCP tool, commits, LLM-never-emits-shell),
see [`AGENTS.md`](AGENTS.md).

## 1. Prerequisites

- **Rust toolchain** via `rustup`. The workspace pins in a committed
  `rust-toolchain.toml`; do not `rustup override`.
- **Components:** `rustfmt`, `clippy`, `rust-src`.
- **System packages:** `pkg-config`; `libsqlite3-dev` only if you
  disable the bundled `rusqlite` feature (default is bundled — see
  ADR-0004). `libdbus-1-dev` when PolKit lands (ADR-0005).
- **Host OS:** Ubuntu 25.10 primary. Other Linux builds fine;
  integration tests that shell out skip if the tool is missing.

## 2. First-time setup

```bash
git clone <remote> russell
cd russell
cargo fetch && cargo build && cargo test
cargo install cargo-insta cargo-deny cargo-nextest --locked
```

## 3. Formatting and linting

- `cargo fmt` — honors `rustfmt.toml`.
- `cargo clippy --workspace --all-targets -- -D warnings`. Narrow
  `#[allow(clippy::xxx)]` with a comment is OK; project-wide goes in
  `clippy.toml` with an ADR reference.

See [`docs/standards/coding-rust.md`](docs/standards/coding-rust.md).

## 4. Cargo aliases

`.cargo/config.toml` (shipped with the skeleton) provides:

| Alias | Expansion |
|---|---|
| `cargo ci` | `check --workspace --all-targets` |
| `cargo fix-all` | `fmt && clippy --workspace --all-targets --fix --allow-staged -- -D warnings` |
| `cargo t` | `nextest run --workspace` (falls back to `test --workspace`) |
| `cargo snap` | `insta test --workspace --review` |
| `cargo deny-check` | `deny check` |

## 5. Testing

- Unit tests: colocated `mod tests`.
- Integration: `tests/` per crate. `russell-mcp` drives stdio via a
  synthetic client.
- Snapshots: `insta`. Review with `cargo insta review`; never
  `accept` blindly.
- Property: `proptest` for manifest/rules parsers and MCP wire.
- Coverage: not CI-gated; `cargo llvm-cov` locally.

VM-based e2e is Phase-3-plus; not PR-gating.

## 6. Pre-commit

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
```

## 7. Journal schema regeneration

SQLite migrations live under
`crates/russell-core/src/journal/migrations/`.

1. Create the next zero-padded `.sql`.
2. Update the migration runner per ADR-0004.
3. Add a test that opens a fresh DB, runs all migrations, and
   snapshots `PRAGMA table_info(...)` per table.
4. Never edit a merged migration.

## 8. Running the ACP server locally

```bash
cargo run -p russell-acp-server
```

stdio is the only v1 transport (ADR-0003). See
[`AGENTS.md`](AGENTS.md) §8 for ACP server commands.

### 8.1 hKask Integration

Configure hKask to connect via ACP. See
[`docs/deployment/acp-integration.md`](docs/deployment/acp-integration.md).

### 8.2 Ad-hoc

Logs reach journald via `tracing-subscriber`; tail:

```bash
journalctl --user -t russell -f
```

## 9. Running tiers and modules ad-hoc

```bash
cargo run -p russell-cli -- sentinel-once
cargo run -p russell-cli -- status
cargo run -p russell-cli -- jack --note "something seems off"
cargo run -p russell-cli -- skill list
cargo run -p russell-cli -- skill run <id>
cargo run -p russell-cli -- proprio
cargo run -p russell-cli -- self-triage
```

All honor `RUSSELL_DRY_RUN=1` globally.

## 10. Filing issues

- **Bugs:** include `russell --version`, relevant journal excerpt
  (`russell journal --since 1h --format json`), and the evidence
  bundle path if a SOAP was produced.
- **Design proposals:** file as a *Proposed* ADR.
- **Skill requests:** open an issue with the symptom to cover, then
  follow [`AGENTS.md`](AGENTS.md) §6.

## 11. PR checklist

- [ ] `cargo fmt --check`, `cargo clippy -- -D warnings`,
      `cargo test --workspace` pass.
- [ ] New public APIs have rustdoc with `# Errors` / `# Panics` /
      `# Examples` where relevant.
- [ ] Snapshot changes reviewed with `cargo insta review`.
- [ ] Mutating actions satisfy IDRS
      ([`docs/standards/safety.md`](docs/standards/safety.md)).
- [ ] Commits follow
      [`docs/standards/commits.md`](docs/standards/commits.md).
- [ ] Locked decisions include an ADR.
