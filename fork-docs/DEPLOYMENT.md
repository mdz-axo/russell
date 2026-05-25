---
title: "Russell Deployment Guide"
audience: [operators, developers]
last_updated: 2026-05-25
version: "1.0.0"
status: "Active"
domain: "Cross-cutting"
ddmvss_categories: [lifecycle]
---

# Russell Deployment Guide

**Purpose:** Define how to install, configure, and operate Russell.

---

## 1. Prerequisites

### 1.1 System Requirements

- **OS:** Ubuntu 25.10 (primary), other Linux (best-effort)
- **Architecture:** x86_64
- **Memory:** 8 GB minimum
- **Disk:** 10 GB free space
- **Rust:** 1.94.1 or later (via rustup)

### 1.2 Dependencies

```bash
# Ubuntu
sudo apt install \
  build-essential \
  libsqlite3-dev \
  pkg-config \
  systemd

# macOS (not officially supported)
brew install sqlite pkg-config
```

---

## 2. Installation

### 2.1 From Source

```bash
# Clone repository
git clone https://github.com/your-org/russell.git
cd russell

# Build
cargo build --release

# Install
./packaging/bin/install.sh
```

### 2.2 From Release

```bash
# Download release
wget https://github.com/your-org/russell/releases/download/v1.0.0/russell-1.0.0-x86_64-linux.tar.gz

# Extract
tar -xzf russell-1.0.0-x86_64-linux.tar.gz

# Install
./install.sh
```

### 2.3 Installation Script

The `install.sh` script:

1. Copies binaries to `~/.local/bin/`
2. Creates `~/.local/state/harness/` directory
3. Creates `~/.config/harness/` directory
4. Installs systemd units to `~/.config/systemd/user/`
5. Enables systemd timers
6. Generates initial profile
7. Starts honeymoon period (30 days)

---

## 3. Configuration

### 3.1 Configuration Files

| File | Purpose |
|------|---------|
| `~/.config/harness/russell.env` | Environment variables |
| `~/.config/harness/disable` | Kill switch (empty file) |
| `~/.config/harness/rules.d/*.toml` | Probe rules |

### 3.2 Environment Variables

```bash
# ~/.config/harness/russell.env

# LLM backend (okapi or openrouter)
RUSSELL_DOCTOR_BACKEND=okapi

# OpenRouter API key (if using openrouter)
OPENROUTER_API_KEY=sk-or-...

# Okapi endpoint (if using okapi)
OKAPI_ENDPOINT=http://localhost:11434

# Log level (error, warn, info, debug, trace)
RUST_LOG=info
```

### 3.3 Probe Rules

```toml
# ~/.config/harness/rules.d/custom.toml

[[rules]]
probe = "mem_available_mib"
warn_below = 1024.0
alert_below = 512.0
crit_below = 256.0

[[rules]]
probe = "loadavg_1m"
warn_above = 8.0
alert_above = 12.0
crit_above = 16.0
```

---

## 4. Operation

### 4.1 Starting Russell

```bash
# Enable systemd timers
systemctl --user enable --now russell-sentinel.timer
systemctl --user enable --now russell-digest.timer

# Start ACP server (optional)
systemctl --user enable --now russell-acp-server.service
```

### 4.2 Checking Status

```bash
# Check timer status
systemctl --user list-timers 'russell-*'

# Check service status
systemctl --user status russell-sentinel.service

# View logs
journalctl --user -u russell-sentinel.service -f
```

### 4.3 Manual Operations

```bash
# Run sentinel once
russell sentinel-once

# Query journal
russell list --limit 20

# Consult Jack
russell jack --note "What's wrong with my machine?"

# Run skill
russell skill run okapi-watcher/probe-health

# Self-triage
russell self-triage

# Verify journal
russell verify-journal
```

### 4.4 Stopping Russell

```bash
# Stop timers
systemctl --user stop russell-sentinel.timer
systemctl --user stop russell-digest.timer

# Stop ACP server
systemctl --user stop russell-acp-server.service
```

---

## 5. Maintenance

### 5.1 Updates

```bash
# Pull latest code
cd ~/Clones/russell
git pull

# Rebuild
cargo build --release

# Reinstall
./packaging/bin/install.sh
```

### 5.2 Backups

```bash
# Backup state
tar -czf russell-state-$(date +%Y%m%d).tar.gz \
  ~/.local/state/harness/ \
  ~/.config/harness/ \
  ~/.local/share/harness/skills/
```

### 5.3 Reset

```bash
# Stop Russell
systemctl --user stop russell-sentinel.timer
systemctl --user stop russell-digest.timer
systemctl --user stop russell-acp-server.service

# Delete state
rm -rf ~/.local/state/harness/

# Reinstall
./packaging/bin/install.sh
```

### 5.4 Uninstall

```bash
# Stop services
systemctl --user stop russell-sentinel.timer
systemctl --user stop russell-digest.timer
systemctl --user stop russell-acp-server.service

# Disable services
systemctl --user disable russell-sentinel.timer
systemctl --user disable russell-digest.timer
systemctl --user disable russell-acp-server.service

# Remove binaries
rm ~/.local/bin/russell
rm ~/.local/bin/russell-acp-server

# Remove systemd units
rm ~/.config/systemd/user/russell-*.service
rm ~/.config/systemd/user/russell-*.timer

# Remove state (optional)
rm -rf ~/.local/state/harness/
rm -rf ~/.config/harness/
rm -rf ~/.local/share/harness/
```

---

## 6. Troubleshooting

### 6.1 Common Issues

**Issue:** Sentinel not running  
**Solution:** Check timer status: `systemctl --user list-timers 'russell-*'`

**Issue:** Journal corruption  
**Solution:** Verify journal: `russell verify-journal`. If corrupt, reset: `rm -rf ~/.local/state/harness/journal.db`

**Issue:** Jack not responding  
**Solution:** Check LLM backend: `curl http://localhost:11434/api/tags` (Okapi) or check OpenRouter API key

**Issue:** Skill execution fails  
**Solution:** Check skill logs: `journalctl --user -u russell-sentinel.service | grep <skill-id>`

### 6.2 Debug Mode

Enable debug logging:

```bash
# Set log level
export RUST_LOG=debug

# Run sentinel
russell sentinel-once
```

### 6.3 Support

- **Documentation:** `docs/` directory
- **Issues:** https://github.com/your-org/russell/issues
- **Discussions:** https://github.com/your-org/russell/discussions

---

## 7. Security

### 7.1 Kill Switch

```bash
# Disable all mutations
touch ~/.config/harness/disable

# Re-enable
rm ~/.config/harness/disable
```

### 7.2 Permissions

```bash
# Check permissions
ls -la ~/.local/state/harness/
ls -la ~/.config/harness/

# Fix permissions (if needed)
chmod 700 ~/.local/state/harness/
chmod 700 ~/.config/harness/
```

### 7.3 Network

Russell makes no network connections by default. To enable:

```bash
# Enable LLM egress
echo "RUSSELL_DOCTOR_BACKEND=openrouter" >> ~/.config/harness/russell.env
echo "OPENROUTER_API_KEY=sk-or-..." >> ~/.config/harness/russell.env
```

---

## 8. References

- Installation script: `packaging/bin/install.sh`
- Systemd units: `packaging/systemd/`
- Configuration: `~/.config/harness/`
- State: `~/.local/state/harness/`
