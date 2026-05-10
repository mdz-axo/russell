---
title: "Reuse Manifest — what Russell copied and from where"
audience: [developers, architects, contributors]
last_updated: 2026-04-18
togaf_phase: "D — Technology Architecture"
version: "1.0.0"
status: "Active"
---

# Reuse Manifest

<!-- TOGAF_DOMAIN: Technology Architecture -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-04-18 -->

Under JR-6 (reuse, don't depend), Russell **copies** code from
upstream workspaces rather than depending on them. This manifest
is the single register of every such copy.

Every row must name:

- Russell path.
- Upstream path (relative to `~/Clones/`).
- Upstream commit SHA at copy time.
- What Russell changed during the copy.
- The sync policy — what we do when upstream changes.

## 1. Why copy and not depend

See [`../architecture/PRINCIPLES_CATALOG.md`](../architecture/PRINCIPLES_CATALOG.md) §3 JR-6.
Summary: Russell must build offline, must survive unrelated
upstream breakage, and must not propagate dependencies across
workspaces. The cost is explicit synchronization; the buy is
resilience.

## 2. File-level discipline

Every copied file starts with a comment header of the form:

```rust
// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copied from: slate/stack/crates/stack-llm/src/openai.rs
// Upstream commit: <sha-at-copy-time>
// Russell changes:
//   - Removed streaming support (MVP calls are single round-trip)
//   - Removed tool-calling feature
//   - Retained: OpenAiCompatibleBackend, request/response types
// Sync policy: review on upstream bug fix; pull fixes, not features.
// See docs/operations/REUSE_MANIFEST.md row <N>.
```

The comment header **is not optional**. Missing header = review
blocker.

## 3. Manifest (current)

| # | Russell path | Upstream path | Upstream commit | Russell changes | Sync policy |
|---|---|---|---|---|---|
| 1 | `crates/russell-doctor/src/openrouter.rs` | `slate/stack/crates/stack-llm/src/openai.rs` + `src/wire.rs` | `67a13834d8af4efa8c330ce10ef1031bf2cdeee2` | Uses Russell's `DoctorError` + `SoapPrompt`/`LlmResponse` instead of `stack_types`. Drops streaming, tool-calling, structured-output, retry. Adds per-request ZDR enforcement. Keeps DeepSeek / Kimi `reasoning_details` content-normalisation. | Review on upstream bug fix; pull fixes, not features. Log changes in §6. |

## 4. Planned copies for Phase 1

These are the copies Russell will land when `russell help` is
implemented. This section is **forward-looking**; the actual
copies populate §3 at copy time.

### 4.1 `stack-llm` — the LLM router

**From:** `Clones/slate/stack/crates/stack-llm/`
**To:** `russell/crates/russell-doctor/src/llm/`
**Files planned:**

- `provider.rs` — the `Provider` trait
- `backend.rs` — the `AnyBackend` enum
- `openai.rs` — `OpenAiCompatibleBackend` (OpenRouter uses this)
- `mock.rs` — the mock backend for tests
- `wire.rs` — request/response wire types
- `retry.rs` — retry-with-backoff
- `circuit.rs` — circuit breaker

**Features dropped during copy:**

- Streaming (`streaming.rs` not copied — MVP is single round-trip).
- Structured output (`structured.rs` not copied — MVP returns plain
  text).
- Tool calling (`tools.rs` not copied — JR-3 forbids shell
  emission).
- Catalog (`catalog.rs` not copied — MVP uses one env-configured
  model).

**Russell additions:**

- A thin adapter `russell-doctor::llm::jack::JackClient` that
  wraps the copied types and adds Russell's persona/prompting
  discipline.

### 4.2 (Optional) `peripheral-store` SQLite patterns

**From:** `Clones/peripheral/crates/peripheral-store/src/sqlite.rs`
**Status:** reference only — Russell already has its own
SQLite layer in `russell-core::journal`. We may crib specific
patterns (e.g., the connection pool shape) but no code is
copied. This row exists so reviewers know we considered it.

## 5. Sync Cadence

Russell's operator (one person) is not running daily rebases. The
realistic cadence is:

| Trigger | Action |
|---|---|
| Upstream publishes a CVE-class bugfix | Port the fix within 7 days. Row gets a dated entry in §6. |
| Upstream changes an API Russell uses | Decide: port, refactor to avoid, or freeze. Document in §6. |
| Russell's own needs diverge | Update the row's "Russell changes" field; update the file header. |
| 90-day general review | Audit this file; confirm all copies still reflect current upstream OR note deliberate drift. |

## 6. Sync Log

| Date | Row | Action | Upstream commit | Notes |
|---|---|---|---|---|
| *(none yet)* | | | | |

## 7. Related

- [`../architecture/PRINCIPLES_CATALOG.md`](../architecture/PRINCIPLES_CATALOG.md) §3 JR-6.
- [`../adr/0013-rust-workspace-layout.md`](../adr/0013-rust-workspace-layout.md) — workspace DAG.
- `Clones/slate/stack/` — upstream `stack-llm`.
- `Clones/peripheral/` — upstream reference patterns.
