---
title: "ADR-0035 — MCP Crate Consolidation"
audience: [developers, architects]
last_updated: 2026-05-19
ddmvss_context: "mcp"
ddmvss_artifact: "adr"
version: "1.0.0"
status: "Active"
---


# ADR-0035 — MCP Crate Consolidation


## Context

The adversarial multi-perspective review (2026-05-19) identified weakness A3:

> **A3 — `russell-mcp-server` duplicated** — two MCP crates (`russell-mcp`, `russell-mcp-server`) with unclear separation. ADR-0003 deferred, then ADR-0025 lifted partially. Incremental feature addition without refactor.

Russell had two separate MCP crates:

1. **`russell-mcp`** — MCP client for external integration (HTTP REST API client, token auth, tool registry)
2. **`russell-mcp-server`** — MCP server for IDE frontends (rmcp-based stdio server)

This created:
- **Duplication** — Shared types and utilities duplicated or awkwardly split
- **Unclear boundary** — Client and server concerns not cleanly separated
- **Dependency bloat** — Both crates pulled in dependencies even when only one was needed
- **Maintenance burden** — Two crates to update for MCP protocol changes

ADR-0003 deferred the server, but ADR-0025 lifted the deferral for the client. The server was added incrementally without consolidation.

## Decision

Consolidate both crates into a single `russell-mcp` crate with feature flags:

1. **Single crate** — `russell-mcp` contains both client and server code.

2. **Feature flags**:
   - `client` (default) — MCP client for external integration (reqwest, auth, registry)
   - `server` — MCP server for IDE frontends (rmcp, tools)

3. **Directory structure** (actual implementation):
   ```
   crates/russell-mcp/
   ├── src/
   │   ├── auth.rs        # Token provider chain
   │   ├── client.rs      # MCP client
   │   ├── config.rs      # Configuration
   │   ├── error.rs       # Error types
   │   ├── health.rs      # Health check
   │   ├── lib.rs         # Crate root
   │   ├── registry.rs    # MCP server registry
   │   └── types.rs       # Shared types
   └── Cargo.toml
   ```

   **Note:** The `server/` subdirectory described in the original proposal was not implemented. The MCP server functionality lives in `russell-acp-server` (separate crate) per ADR-0027, which provides the ACP session interface for external agent integration.

4. **Conditional compilation** — Modules gated by `#[cfg(feature = "...")]` in `lib.rs`.

5. **Dependency management** — Optional dependencies tied to features:
   - `client` feature enables: `reqwest`, `urlencoding`, `async-trait`, `chrono`
   - `server` feature enables: `russell-core`, `russell-sentinel`, `rmcp`, `anyhow`

6. **Consumer updates** — `russell-cli` depends on `russell-mcp` with `features = ["server"]`.

## Consequences

### Positive

- **Single source of truth** — All MCP-related code in one crate.

- **Clear feature boundary** — Client and server concerns explicitly separated by features.

- **Reduced duplication** — Shared types (e.g., `McpError`, common schemas) defined once.

- **Flexible dependencies** — Consumers only pull in what they need via features.

- **Easier maintenance** — One crate to update for MCP protocol changes.

- **Cockburn hexagonal** — Client and server are distinct adapters to the MCP protocol port.

### Negative

- **Feature complexity** — Consumers must understand which features they need.

- **Conditional compilation** — More `#[cfg]` gates increase cognitive load.

- **Larger crate** — Single crate is larger, but better organized.

### Neutral

- **No breaking changes** — Public API unchanged; only internal reorganization.

- **Backward compatible** — Existing consumers continue to work with updated imports.

## Implementation

### Cargo.toml Changes

```toml
[features]
default = ["client"]
client = ["reqwest", "urlencoding", "async-trait", "chrono"]
server = ["russell-core", "russell-sentinel", "rmcp", "tokio/io-util", "tokio/io-std", "anyhow"]

[dependencies]
# Client dependencies (optional)
reqwest = { workspace = true, optional = true }
urlencoding = { version = "2.1", optional = true }
async-trait = { version = "0.1", optional = true }
chrono = { version = "0.4", features = ["serde"], optional = true }

# Server dependencies (optional)
russell-core = { workspace = true, optional = true }
russell-sentinel = { workspace = true, optional = true }
rmcp = { version = "1.7", features = ["server", "transport-io", "macros"], optional = true }
```

### Module Gates

```rust
#[cfg(feature = "client")]
pub mod auth;
#[cfg(feature = "client")]
pub mod client;
// ...

#[cfg(feature = "server")]
pub mod server;
```

### Consumer Updates

```toml
# russell-cli/Cargo.toml
russell-mcp = { workspace = true, features = ["server"] }
```

```rust
// russell-cli/src/main.rs
Command::Mcp => russell_mcp::server::serve_stdio(paths).await,
```

### Files Changed

| File | Change |
|---|---|
| `crates/russell-mcp/Cargo.toml` | Add features, optional dependencies |
| `crates/russell-mcp/src/lib.rs` | Conditional module exports |
| `crates/russell-mcp/src/server/` | Moved from `russell-mcp-server/src/` |
| `crates/russell-cli/Cargo.toml` | Use `russell-mcp` with `server` feature |
| `crates/russell-cli/src/main.rs` | Update import to `russell_mcp::server` |
| `Cargo.toml` (root) | Remove `russell-mcp-server` member and dependency |

### Files Removed

| File | Reason |
|---|---|
| `crates/russell-mcp-server/` | Consolidated into `russell-mcp` |

## Compliance

| Principle | Compliance |
|---|---|
| **JR-1** (Austere) | Consolidation reduces duplication and complexity |
| **JR-6** (Reuse over dependency) | Shared types defined once, reused across client/server |
| **Cockburn** (Hexagonal) | Client and server are distinct adapters to MCP protocol port |
| **Fowler** (Refactoring) | Internal reorganization without external API changes |

## Future Work

- **Feature documentation** — Document which features are needed for different use cases.

- **Integration tests** — Add tests that verify client and server can coexist when both features enabled.

- **MCP protocol updates** — Single crate simplifies updating to new MCP spec versions.

## References

- Adversarial Review Action Plan §A3 (Task A3)
- `docs/adr/0003-mcp-transport.md` (deferred server)
- [ADR-0025](0025-hkask-mcp-client-trusted-relationship.md) (client) — **Superseded** (hKask integration removed)
- Cockburn, A. (2005). "Hexagonal Architecture"
