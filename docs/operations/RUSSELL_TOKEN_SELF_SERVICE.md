# Russell Token Self-Service

## Overview

Russell can now check his own Kask MCP token status through the `russell_token_status` MCP tool. This eliminates the need for manual CLI commands — Jack can check token status and guide the operator through setup.

## Tool: `russell_token_status`

**Description:** Check Russell's Kask MCP token status — expiry, principal info, and time until rotation needed.

**Input:** None (empty arguments)

**Output Fields:**
- `status` — One of: `not_configured`, `valid`, `rotation_soon`, `rotation_needed`, `expired`
- `principal` — Service principal name (e.g., "russell")
- `scope` — Token scope (e.g., "user")
- `issued_at` — ISO 8601 timestamp
- `expires_at` — ISO 8601 timestamp
- `hours_until_rotation` — Hours until 24h pre-expiry buffer
- `is_expired` — Boolean
- `needs_rotation` — Boolean
- `token_path` — Path to token file
- `setup_command` — Command to create principal (if not configured)
- `rotation_command` — Command to rotate token (if configured)

## Usage

### Via Russell Chat

```
you → what's your token status?
Jack → Let me check...

ACTION: kask/russell_token_status

[Tool output shows status]

Jack → My token is valid for 156 more hours before rotation is needed. 
Everything looks good!
```

### Via curl

```bash
curl -s -X POST "http://127.0.0.1:8080/api/v1/tools/russell_token_status" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $KASK_MCP_TOKEN" \
  -d '{"arguments":{}}'
```

### Via Russell CLI

```bash
# Future enhancement: russell token-status command
```

## Status Values

| Status | Meaning | Action |
|--------|---------|--------|
| `not_configured` | Token file not found; using static env var | Run `setup_command` |
| `valid` | Token valid, >48h until rotation needed | None |
| `rotation_soon` | Token valid, <48h until rotation needed | Schedule rotation |
| `rotation_needed` | Within 24h buffer; rotation recommended | Run `rotation_command` |
| `expired` | Token expired | Rotate immediately |

## Example Outputs

### Not Configured

```json
{
  "status": "not_configured",
  "message": "Token file not found. Russell is using static token from KASK_MCP_TOKEN env var.",
  "token_path": "/home/mdz-axolotl/.local/state/kask/mcp-token.json",
  "setup_required": true,
  "setup_command": "stack-admin key create --for russell --type service --display 'Russell (Host Curator)' --ttl 168h"
}
```

### Valid

```json
{
  "status": "valid",
  "principal": "russell",
  "scope": "user",
  "issued_at": "2026-05-08T00:00:00Z",
  "expires_at": "2026-05-15T00:00:00Z",
  "hours_until_rotation": 156,
  "is_expired": false,
  "needs_rotation": false,
  "token_path": "/home/mdz-axolotl/.local/state/kask/mcp-token.json",
  "rotation_command": "~/.local/bin/rotate-russell-token.sh"
}
```

### Rotation Needed

```json
{
  "status": "rotation_needed",
  "principal": "russell",
  "scope": "user",
  "issued_at": "2026-05-01T00:00:00Z",
  "expires_at": "2026-05-08T00:00:00Z",
  "hours_until_rotation": -12,
  "is_expired": false,
  "needs_rotation": true,
  "token_path": "/home/mdz-axolotl/.local/state/kask/mcp-token.json",
  "rotation_command": "~/.local/bin/rotate-russell-token.sh"
}
```

## Implementation

### Files Modified

- `kask/arsenal/crates/arsenal-mcp-russell/src/tools.rs` — Added tool definition
- `kask/arsenal/crates/arsenal-mcp-russell/src/server.rs` — Added `handle_token_status()` handler
- `kask/arsenal/crates/arsenal-mcp-russell/Cargo.toml` — Added `chrono` dependency

### Code Location

```rust
// arsenal-mcp-russell/src/server.rs
async fn handle_token_status(&self, _args: &Value) -> Result<String, String> {
    // Reads token file at ~/.local/state/kask/mcp-token.json
    // Returns status, expiry info, and actionable commands
}
```

## Operational Flow

### Initial Setup

1. Russell installed, no token file exists
2. Jack runs `russell_token_status` → returns `not_configured`
3. Jack shows operator the `setup_command`
4. Operator runs command in Kask repo
5. Token file created
6. Russell automatically picks up new token via `FileTokenProvider`

### Ongoing Maintenance

1. Weekly timer (`kask-token-rotate.timer`) rotates token automatically
2. OR Jack runs `russell_token_status` periodically
3. If `status == "rotation_needed"`, Jack can prompt operator or auto-rotate
4. Token file updated, Russell picks up automatically

## Security

- Token file permissions: `0600` (owner read/write only)
- Token value NEVER exposed in tool output (only metadata)
- Rotation requires access to Kask's `stack-admin` (operator action or automated via systemd)
- 24-hour pre-expiry buffer prevents accidental expiry

## Related

- `docs/operations/KASK_TOKEN_ROTATION.md` — Token rotation operational guide
- `crates/russell-mcp/src/auth.rs` — Russell's token provider implementation
- ADR-0025 — Kask MCP Client trusted relationship
