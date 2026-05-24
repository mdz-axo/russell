---
title: "ACP Server Systemd Deployment"
audience: [operators]
last_updated: 2026-05-24
togaf_phase: "H"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Technology — Deployment -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-24 -->

---
title: "ACP Server Systemd Deployment"
audience: [operators]
last_updated: 2026-05-24
togaf_phase: "H"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Technology — Deployment -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-24 -->

# ACP Server Systemd Deployment

This document describes how to deploy the Russell ACP server as a systemd service for integration with hKask.

## Overview

The Russell ACP (Agent Client Protocol) server:
- Communicates with hKask via stdio JSON-RPC
- Exposes Russell's skills to hKask agents
- Enforces capability-based access control (OCAP)
- Runs as a separate systemd service from the Sentinel

## Service Architecture

```
┌─────────────┐     stdio      ┌──────────────────┐
│   hKask     │ ◄────────────► │ russell-acp-server │
│   (CNS)     │   JSON-RPC     │   (systemd svc)   │
└─────────────┘                └────────┬─────────┘
                                        │
                                        │
                               ┌────────▼─────────┐
                               │   Journal DB     │
                               │ (~/.local/state/ │
                               │  harness/journal │
                               │      .db)        │
                               └──────────────────┘
```

## Installation

### 1. Build Russell

```bash
cd ~/Clones/russell
cargo build --release
```

### 2. Install Binary

```bash
# Copy to user bin directory
cp target/release/russell-acp-server ~/.local/bin/

# Ensure it's in PATH
export PATH="$HOME/.local/bin:$PATH"
```

### 3. Create Systemd Service Unit

Create `~/.config/systemd/user/russell-acp-server.service`:

```ini
[Unit]
Description=Russell ACP Server (hKask integration)
Documentation=file:///home/mdz-axolotl/Clones/russell/docs/README.md
After=network.target

[Service]
Type=simple
ExecStart=%h/.local/bin/russell-acp-server
Restart=on-failure
RestartSec=5
TimeoutStopSec=30

# Environment
Environment=RUST_LOG=info
Environment=HKASK_CNS_ENDPOINT=http://localhost:8080/cns

# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=read-only
PrivateTmp=true
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6
RestrictNamespaces=true
RestrictRealtime=true
RestrictSUIDSGID=true
MemoryDenyWriteExecute=true
LockPersonality=true

# Allow write access to state directory
ReadWritePaths=%h/.local/state/harness
ReadWritePaths=%h/.local/share/harness

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier=russell-acp

[Install]
WantedBy=default.target
```

### 4. Create Sentinel Timer (if not already installed)

Create `~/.config/systemd/user/russell-sentinel.service`:

```ini
[Unit]
Description=Russell Sentinel (health observation)
Documentation=file:///home/mdz-axolotl/Clones/russell/docs/README.md

[Service]
Type=oneshot
ExecStart=%h/.local/bin/russell sentinel-once
Environment=RUST_LOG=info

# Security hardening (same as ACP server)
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=read-only
PrivateTmp=true
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6
RestrictNamespaces=true
RestrictRealtime=true
RestrictSUIDSGID=true
MemoryDenyWriteExecute=true
LockPersonality=true

ReadWritePaths=%h/.local/state/harness
ReadWritePaths=%h/.local/share/harness

StandardOutput=journal
StandardError=journal
SyslogIdentifier=russell-sentinel
```

Create `~/.config/systemd/user/russell-sentinel.timer`:

```ini
[Unit]
Description=Russell Sentinel Timer (5-min cadence)
Documentation=file:///home/mdz-axolotl/Clones/russell/docs/README.md

[Timer]
OnBootSec=1min
OnUnitActiveSec=5min
AccuracySec=10sec
Persistent=true

[Install]
WantedBy=timers.target
```

### 5. Enable and Start Services

```bash
# Reload systemd to pick up new units
systemctl --user daemon-reload

# Enable services (start on boot)
systemctl --user enable russell-acp-server.service
systemctl --user enable russell-sentinel.timer
systemctl --user enable russell-sentinel.service

# Start services now
systemctl --user start russell-acp-server.service
systemctl --user start russell-sentinel.timer

# Check status
systemctl --user status russell-acp-server.service
systemctl --user status russell-sentinel.timer
```

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `RUST_LOG` | Logging level | `info` |
| `HKASK_CNS_ENDPOINT` | hKask CNS endpoint | (none, local-only) |
| `RUSSELL_ACP_MACAROON_KEY` | Macaroon root key for OCAP | (none, dev mode) |
| `HOME` | User home directory | (system default) |

### Macaroon Setup (Production)

For production deployment with capability-based access control:

```bash
# Generate a secure root key
openssl rand -base64 32 > ~/.config/harness/macaroon.key
chmod 600 ~/.config/harness/macaroon.key

# Set environment variable
export RUSSELL_ACP_MACAROON_KEY=$(cat ~/.config/harness/macaroon.key)

# Add to systemd service
Environment=RUSSELL_ACP_MACAROON_KEY=your-key-here
```

## Monitoring

### Check Service Status

```bash
# ACP server status
systemctl --user status russell-acp-server.service

# Sentinel timer status
systemctl --user status russell-sentinel.timer

# View recent logs
journalctl --user -u russell-acp-server.service -n 50
journalctl --user -u russell-sentinel.timer -n 50
```

### Follow Logs in Real-Time

```bash
journalctl --user -u russell-acp-server.service -f
journalctl --user -u russell-sentinel.timer -f
```

### Test ACP Server

```bash
# Test stdio JSON-RPC interface
echo '{"jsonrpc":"2.0","id":1,"method":"acp/capabilities","params":{}}' | \
  russell-acp-server
```

## Troubleshooting

### Service Won't Start

```bash
# Check for syntax errors in unit file
systemd-analyze verify ~/.config/systemd/user/russell-acp-server.service

# Check detailed error logs
journalctl --user -u russell-acp-server.service -n 100 --no-pager
```

### Binary Not Found

```bash
# Verify binary location
ls -la ~/.local/bin/russell-acp-server

# Check PATH in service
systemctl --user show-environment | grep PATH
```

### Permission Denied

```bash
# Check directory permissions
ls -la ~/.local/state/harness/
ls -la ~/.local/share/harness/

# Fix permissions if needed
chmod 755 ~/.local/state/harness
chmod 755 ~/.local/share/harness
```

### CNS Connection Issues

```bash
# Test CNS endpoint
curl -v http://localhost:8080/cns

# Check if hKask is running
systemctl --user status hkask.service
```

## Uninstall

```bash
# Stop and disable services
systemctl --user stop russell-acp-server.service
systemctl --user stop russell-sentinel.timer
systemctl --user disable russell-acp-server.service
systemctl --user disable russell-sentinel.timer

# Remove unit files
rm ~/.config/systemd/user/russell-acp-server.service
rm ~/.config/systemd/user/russell-sentinel.service
rm ~/.config/systemd/user/russell-sentinel.timer

# Reload systemd
systemctl --user daemon-reload

# Remove binary
rm ~/.local/bin/russell-acp-server
```

## Security Notes

1. **Capability Tokens**: In production, always configure `RUSSELL_ACP_MACAROON_KEY`
2. **Network Restrictions**: The service only needs localhost access
3. **Filesystem Access**: Limited to `~/.local/state/harness` and `~/.local/share/harness`
4. **No Privilege Escalation**: `NoNewPrivileges=true` prevents privilege escalation
5. **Read-Only Home**: `ProtectHome=read-only` prevents modification of user files

## Integration with hKask

The ACP server communicates with hKask via stdio JSON-RPC. hKask should:

1. Spawn `russell-acp-server` as a subprocess
2. Send JSON-RPC requests over stdin
3. Read JSON-RPC responses from stdout
4. Handle capability negotiation during startup

Example hKask integration:

```python
import subprocess
import json

# Start ACP server
acp = subprocess.Popen(
    ['russell-acp-server'],
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    text=True
)

# Send capability request
request = {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "acp/capabilities",
    "params": {}
}

acp.stdin.write(json.dumps(request) + "\n")
acp.stdin.flush()

# Read response
response = json.loads(acp.stdout.readline())
print(response)
```

## See Also

- [`docs/architecture/PRINCIPLES_CATALOG.md`](../architecture/PRINCIPLES_CATALOG.md) — Jack Russell principles
- [`docs/standards/safety.md`](../standards/safety.md) — IDRS contract
- [`crates/russell-acp-server/`](../../crates/russell-acp-server/) — ACP server source
