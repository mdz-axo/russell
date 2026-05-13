---
title: "ADR-0003: MCP Transport (Deferred)"
audience: [developers, architects]
last_updated: 2026-04-18
togaf_phase: "H"
version: "1.0.0"
status: "Deprecated"
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
audience: MCP server contributors
last-reviewed: 2026-04-17
-->

# ADR-0003: MCP transport — stdio only, in v1

- **Status:** Accepted
- **Date:** 2026-04-17
- **Deciders:** Project founders
- **Tags:** `mcp`, `transport`, `security`

## Context

The Model Context Protocol supports multiple transports: stdio,
HTTP+SSE, and others via the spec's transport negotiation. Agent
frontends Russell targets — Claude Desktop, Roo/Cline in
VSCodium, Zed — all support stdio with a local-process spawn
model. That model also:

- runs under the same UID as the user,
- requires no network listener,
- delegates lifecycle (kill, restart) to the frontend,
- logs naturally to stdout/stderr + journald.

See [ADR-0001](../0001-scope-and-charter.md): Russell is a
single-host, single-operator tool.

## Decision

For v1:

1. **stdio is the only supported transport.**
2. The binary `russell` exposes the server via `russell mcp`
   (CLI subcommand) and via the long-running
   `russell mcp-probe` inspector.
3. No `--listen` flag, no `--http` flag, no Unix socket,
   no TCP.
4. The server reads MCP JSON-RPC frames on stdin and writes
   responses on stdout. Logs go to stderr and journald via
   `tracing-journald`.

## Consequences

### Positive

- Zero network attack surface.
- Frontend owns lifecycle; if the frontend exits, the
  server exits with it.
- Trivial to run in ad-hoc contexts via
  `npx @modelcontextprotocol/inspector` or a shell pipe.
- No port-binding conflicts, no certificate management.

### Negative / accepted costs

- Multiple frontends cannot share one Russell process; each
  spawns its own. For a single-host tool with cheap startup
  this is a feature, not a bug — each frontend sees a
  clean session.
- Cross-machine agent frontends (e.g. a phone acting as a
  remote operator) cannot talk to Russell in v1. A future
  ADR may add a transport under a Policy-gated opt-in.

### Neutral

- stdio adds no dependencies beyond what the MCP SDK
  already requires.

## Alternatives considered

### stdio + local HTTP on 127.0.0.1

Rejected for v1. Introduces a listener we'd have to secure
(auth, localhost-binding verification), and the target
frontends do not need it.

### Unix socket

Rejected for v1 for the same reason. We may reconsider in a
future ADR if we need process-to-process sharing on the same
host.

### stdio + named-pipe fallback for Windows

Rejected: Windows is out of charter per ADR-0001.

## Implementation notes

- `russell-mcp` uses the official Rust MCP SDK (or a
  well-maintained compatible crate); the exact choice is a
  library selection note, not an ADR-level decision.
- The server's tool surface is documented in
  [`../../archive/mcp-surface.md`](../../archive/mcp-surface.md).
- Any future transport must arrive via a new ADR that
  addresses:
  1. Threat model (who can connect?).
  2. Authentication (how does Russell know the caller?).
  3. Rate limiting / abuse resistance.
  4. Whether the charter (ADR-0001) needs revision.

## References

- MCP specification: https://modelcontextprotocol.io
- [`CONTRIBUTING.md` §8](../../../CONTRIBUTING.md) — how to wire
  frontends to the stdio binary.
- [ADR-0001](../0001-scope-and-charter.md) — single-host scope.
