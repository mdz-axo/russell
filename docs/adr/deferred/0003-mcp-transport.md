---
title: "ADR-0003: MCP Transport"
audience: [developers, architects]
last_updated: 2026-05-14
togaf_phase: "H"
version: "2.0.0"
status: "Implemented"
---

<!-- TOGAF_DOMAIN: Change Management -->
<!-- VERSION: 2.0.0 -->
<!-- STATUS: Implemented -->
<!-- LAST_UPDATED: 2026-05-14 -->

> **Implemented.** The native MCP server shipped in v0.20.0 via
> `russell mcp` (stdio transport). Deferral lifted.

<!--
audience: MCP server contributors
last-reviewed: 2026-05-14
-->

# ADR-0003: MCP transport — stdio only, in v1

- **Status:** Implemented
- **Date:** 2026-04-17 (accepted), 2026-05-14 (implemented)
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

See [ADR-0001](0001-scope-and-charter.md): Russell is a
single-host, single-operator tool.

## Decision

For v1:

1. **stdio is the only supported transport.**
2. The binary `russell` exposes the server via `russell mcp`
   (CLI subcommand).
3. No `--listen` flag, no `--http` flag, no Unix socket,
   no TCP.
4. The server reads MCP JSON-RPC frames on stdin and writes
   responses on stdout. Logs go to stderr.

## Implementation (v0.20.0)

- **Crate:** `russell-mcp-server` (new workspace member).
- **SDK:** `rmcp` 1.7 (official Rust MCP SDK from
  `modelcontextprotocol/rust-sdk`).
- **CLI:** `russell mcp` subcommand launches the stdio server.
- **Tools exposed (6, all risk:none):**

| Tool | Description |
|------|-------------|
| `russell_host_snapshot` | Per-probe last/min/avg/max + p95 baselines |
| `russell_recent_events` | Last N journal events |
| `russell_journal_query` | Time-range + severity/scope filter |
| `russell_probe_history` | Per-probe sample statistics |
| `russell_health_summary` | Severity breakdown + staleness + deviations |
| `russell_run_sentinel` | Run Sentinel once, return fresh samples |

### Frontend configuration

**Zed** (`~/.config/zed/settings.json`):

```json
{
  "context_servers": {
    "russell": {
      "command": {
        "path": "russell",
        "args": ["mcp"]
      }
    }
  }
}
```

**Claude Desktop** (`~/.config/claude/claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "russell": {
      "command": "russell",
      "args": ["mcp"]
    }
  }
}
```

**Kilo / Cline / Roo** (`kilo.json` or equivalent):

```json
{
  "mcpServers": {
    "russell": {
      "command": "russell",
      "args": ["mcp"],
      "disabled": false
    }
  }
}
```

**MCP Inspector** (ad-hoc testing):

```bash
npx @modelcontextprotocol/inspector russell mcp
```

## Consequences

### Positive

- Zero network attack surface.
- Frontend owns lifecycle; if the frontend exits, the
  server exits with it.
- Trivial to run in ad-hoc contexts via the MCP inspector.
- No port-binding conflicts, no certificate management.
- Russell can be used standalone (without Kask) by any
  MCP-speaking agent.

### Negative / accepted costs

- Multiple frontends cannot share one Russell process; each
  spawns its own. For a single-host tool with cheap startup
  this is a feature, not a bug — each frontend sees a
  clean session.
- Cross-machine agent frontends cannot talk to Russell in v1.

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

- `russell-mcp-server` uses `rmcp` 1.7 (the official Rust
  MCP SDK from `modelcontextprotocol/rust-sdk`).
- Any future transport must arrive via a new ADR that
  addresses:
  1. Threat model (who can connect?).
  2. Authentication (how does Russell know the caller?).
  3. Rate limiting / abuse resistance.
  4. Whether the charter (ADR-0001) needs revision.

## References

- MCP specification: https://modelcontextprotocol.io
- [ADR-0001](0001-scope-and-charter.md) — single-host scope.
- [ADR-0025](../0025-hkask-mcp-client-trusted-relationship.md) —
  Russell as MCP *client* to Kask (orthogonal to this ADR).
