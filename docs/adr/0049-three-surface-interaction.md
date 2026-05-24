---
title: "ADR-0049: Three-Surface Interaction Requirement (CLI, API, ACP)"
audience: [architects, developers, operators]
last_updated: 2026-05-24
togaf_phase: "G"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Governance — Architecture Correction -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-24 -->

# ADR-0049: Three-Surface Interaction Requirement (CLI, API, ACP)

## Context

The adversarial review of 2026-05-23 incorrectly removed the CLI Chat REPL,
claiming it had been "absorbed into the ACP session interface." This was a
factual error: the Chat REPL was a working, tested subsystem that provided
the interactive Jack interface on the CLI surface. Its removal violated
the three-surface requirement — that Jack's interactive session must be
available on three functionally equivalent surfaces:

1. **CLI** — `russell chat` (direct operator interaction)
2. **API** — HTTP REST endpoints (`russell-api-server`)
3. **ACP** — JSON-RPC over stdio (`russell-acp-server`, hKask integration)

The CHANGELOG entry "Chat REPL removed" and the AGENTS.md vocabulary entry
"Chat REPL — Removed" were both incorrect documentation changes that
codified the adversarial review's mistake.

---

## Decision

### 1. Restore the Chat REPL

The CLI chat module (`crates/russell-cli/src/commands/chat/`) is restored
from git history (commit `ae3296c`). The original implementation is preserved
as-is — it was the one subsystem that worked correctly.

### 2. Create shared session engine

A new `russell-session` crate provides `SessionEngine`, `Session`,
`SessionManager`, and consent types shared by the ACP server. The CLI chat
uses its own direct implementation (the original restored module), while
the ACP server delegates to `SessionEngine`.

### 3. Create API server

A new `russell-api-server` crate provides HTTP REST endpoints that are
functionally equivalent to the CLI and ACP surfaces. The API server uses
its own `ApiSession`/`AppState` types (not `russell-session::SessionEngine`)
because `SessionEngine` is `!Send` (due to `?Send` `InterventionPort`),
which is incompatible with axum's `Send` requirement for shared state.

### 4. Correct documentation

- AGENTS.md vocabulary: "Chat REPL — Removed" → active definition
- CHANGELOG: "Chat REPL removed" → corrected to reflect restoration
- This ADR supersedes any prior claim that the Chat REPL was intentionally removed

### 5. Three-surface equivalence

All three surfaces MUST support:

| Capability | CLI | API | ACP |
|------------|-----|-----|-----|
| Create session | `russell chat` | `POST /sessions` | `acp/session.create` |
| Send message | Interactive REPL | `POST /sessions/{id}/messages` | `acp/session.message` |
| Consent flow | `/approve`, `/deny` | `POST /sessions/{id}/consent` | `acp/consent.respond` |
| Close session | `/quit`, Ctrl-D | `DELETE /sessions/{id}` | `acp/session.close` |
| Session status | `/status` | `GET /sessions/{id}` | `acp/session.status` |

---

## Consequences

### Positive

- Operator can interact with Jack from any surface
- CLI remains available for direct local use without HTTP or ACP overhead
- API enables web-based frontends and scripting via curl
- ACP enables hKask agent integration
- Each surface uses implementation appropriate to its concurrency model

### Negative

- Three code paths to maintain (mitigated by shared session types where possible)
- API server cannot reuse `SessionEngine` directly due to `!Send` constraint

### Accepted

- `SessionEngine` is `!Send` because `InterventionPort` uses `?Send` async trait
  (required for ACP's single-threaded stdio server). This is fine for CLI and
  ACP but requires the API server to use its own `Send`-safe session types.

---

## Supersedes

- CHANGELOG entry "Chat REPL removed" (2026-05-23)
- AGENTS.md vocabulary "Chat REPL — Removed" (2026-05-23)
- Any claim in ADR-0042 or other documents that Chat REPL removal was intentional

---

## References

- [ADR-0027: hKask ACP Integration](0027-acp-integration.md)
- [ADR-0041: ACP Consent Protocol](0041-acp-consent-protocol.md)
- [ADR-0042: Adversarial Review Remediation Plan](0042-adversarial-review-remediation-plan.md)
- `crates/russell-cli/src/commands/chat/` — restored CLI chat module
- `crates/russell-session/src/` — shared session engine
- `crates/russell-api-server/src/` — HTTP REST API server
- `crates/russell-acp-server/src/` — ACP JSON-RPC server
