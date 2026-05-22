# Russell ACP Server Installation

## Quick Install

```bash
# Build and install binary
cargo install --path crates/russell-acp-server --root ~/.cargo

# Copy systemd units
cp docs/deployment/russell-acp-server.service ~/.config/systemd/user/
cp docs/deployment/russell-sentinel.service ~/.config/systemd/user/
cp docs/deployment/russell-sentinel.timer ~/.config/systemd/user/

# Reload systemd
systemctl --user daemon-reload

# Enable services
systemctl --user enable --now russell-sentinel.timer
systemctl --user enable --now russell-acp-server.service

# Verify
systemctl --user status russell-acp-server.service
journalctl --user -u russell-acp-server -f
```

## Macaroon Configuration

Generate a root key:

```bash
openssl rand -hex 32 > ~/.config/hkask/macaroon-root.key
chmod 600 ~/.config/hkask/macaroon-root.key
```

Create `~/.config/hkask/macaroon.yaml`:

```yaml
issuer:
  root_key: "$(cat ~/.config/hkask/macaroon-root.key)"
  capabilities:
    - name: russell-acp
      attenuations:
        - skill: web-search
        - skill: journal-viewer
        - skill: scenario-tester
        - rate_limit: 100/minute
      before: 24h
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RUSSELL_DOCTOR_BACKEND` | `okapi` | LLM backend (okapi/mock/offline) |
| `RUSSELL_DOCTOR_MODEL` | `qwen3.5:cloud` | Model name |
| `RUSSELL_ACP_MACAROON_KEY` | (empty) | Macaroon root key (hex) |
| `RUSSELL_ESCALATE_MIN` | `alert` | Minimum severity for escalation |

## Troubleshooting

### Check if Okapi is running

```bash
curl http://127.0.0.1:11435/api/tags
```

### View ACP server logs

```bash
journalctl --user -u russell-acp-server -n 50 -f
```

### Test skills directory

```bash
ls -la ~/.local/share/harness/skills/
cargo run -p russell-skills --example load-all
```

### Verify journal exists

```bash
ls -la ~/.local/state/harness/journal.db
```
