---
title: "Kask Token Rotation for Russell"
audience: [operators, developers]
last_updated: 2026-05-14
togaf_phase: "G"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Governance -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-14 -->

# Kask Token Rotation for Russell

## Overview

This document describes the token rotation mechanism for Russell's Kask MCP client integration.

## Architecture

```
┌─────────────────┐      ┌──────────────────┐      ┌─────────────────┐
│  stack-keystore │─────▶│  token endpoint  │─────▶│  Russell MCP    │
│  (Kask side)    │      │  (file/socket)   │      │  client         │
└─────────────────┘      └──────────────────┘      └─────────────────┘
        │                        │                         │
        │ 1. Issue/rotate token  │                         │
        │───────────────────────▶│                         │
        │                        │                         │
        │                        │ 2. Poll for updates     │
        │                        │◀────────────────────────┤
        │                        │                         │
        │                        │ 3. Return current token │
        │                        │────────────────────────▶│
```

## Token File Format

Location: `~/.local/state/kask/mcp-token.json`

```json
{
  "token": "eyJhbGc...",
  "issued_at": "2026-05-15T00:00:00Z",
  "expires_at": "2026-05-22T00:00:00Z",
  "scope": "user",
  "principal": "russell"
}
```

## Russell Client Integration

The `russell-mcp` crate provides a `TokenProvider` trait with two implementations:

1. **StaticTokenProvider** — Uses `KASK_MCP_TOKEN` env var (backward compatible)
2. **FileTokenProvider** — Reads from token file with automatic refresh

### Usage

```rust
use russell_mcp::auth::{FileTokenProvider, TokenProvider};

// File-based token provider (recommended)
let provider = FileTokenProvider::new(
    PathBuf::from("~/.local/state/kask/mcp-token.json")
);

// Token is automatically refreshed before expiry
let token = provider.get_token().await?;
```

## Kask-Side Setup

### 1. Create Russell Service Principal

```bash
stack-admin key create --for russell --type service \
  --display "Russell (Host Curator)" \
  --ttl 168h  # 7 days
```

### 2. Grant Capabilities

```bash
stack-admin key grant --for russell --capability mcp:tools/list --scope "*"
stack-admin key grant --for russell --capability mcp:tools/call --scope "russell_*"
stack-admin key grant --for russell --capability mcp:tools/call --scope "okapi_*"
```

### 3. Install Token File

```bash
stack-admin key get --for russell --format json \
  > ~/.local/state/kask/mcp-token.json
chmod 600 ~/.local/state/kask/mcp-token.json
```

### 4. Auto-Rotation (cron/systemd timer)

Create `~/.config/systemd/user/kask-token-rotate.timer`:

```ini
[Unit]
Description=Rotate Russell MCP token weekly
OnCalendar=weekly
Persistent=true

[Service]
Type=oneshot
ExecStart=%h/.local/bin/rotate-russell-token.sh
```

Create `~/.local/bin/rotate-russell-token.sh`:

```bash
#!/usr/bin/env bash
stack-admin key rotate --for russell --format json \
  > ~/.local/state/kask/mcp-token.json
chmod 600 ~/.local/state/kask/mcp-token.json
```

## Security Considerations

- Token file permissions: `0600` (owner read/write only)
- Token expiry: 7 days (balance between security and operational overhead)
- Rotation happens before expiry (24h buffer)
- Failed rotation attempts are logged but don't break existing token
