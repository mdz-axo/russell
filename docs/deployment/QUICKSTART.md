# Russell ACP Quickstart

**5-minute setup for hKask integration**

---

## Prerequisites

- [x] Rust toolchain installed (`cargo --version`)
- [x] Okapi running (`systemctl --user status okapi`)
- [x] Skills populated (`ls ~/.local/share/harness/skills/`)

---

## Step 1: Install (2 min)

```bash
cd ~/Clones/russell
./docs/deployment/install.sh
```

---

## Step 2: Configure Macaroon (1 min)

```bash
./docs/deployment/macaroon-setup.sh
source ~/.bashrc  # or restart shell
```

---

## Step 3: Enable Services (1 min)

```bash
systemctl --user daemon-reload
systemctl --user enable --now russell-sentinel.timer
systemctl --user enable --now russell-acp-server.service
```

---

## Step 4: Verify (1 min)

```bash
# Check ACP server is running
systemctl --user status russell-acp-server.service

# Check sentinel timer is active
systemctl --user list-timers | grep russell

# View logs
journalctl --user -u russell-acp-server -n 20
```

Expected output:
```
● russell-acp-server.service - Russell ACP Server
     Active: active (running)
     Logs: Loaded 14 skills
```

---

## Step 5: Test ACP Connection

```bash
# Create a test session (stdio transport)
echo '{"jsonrpc":"2.0","id":1,"method":"acp/capabilities","params":{}}' | \
  russell-acp-server
```

Expected: JSON response with 8 public skills listed.

---

## Next Steps

1. **Configure hKask:** Add Russell as ACP agent in hKask config
2. **Test bidirectional:** Run hKask agent that calls Russell probes
3. **Monitor:** `watch -n 60 'journalctl --user -u russell-sentinel -n 5'`

---

## Troubleshooting

| Problem | Solution |
|---------|----------|
| `Active: failed` | Check logs: `journalctl --user -u russell-acp-server -n 50` |
| `Okapi connection refused` | Start Okapi: `systemctl --user start okapi` |
| `Skills directory not found` | Populate skills: `cargo run -p russell-skills --example sync-bundled` |
| `Macaroon auth failed` | Re-run: `./docs/deployment/macaroon-setup.sh` |

---

## Reference

- [Full Installation Guide](INSTALL.md)
- [ACP Integration Guide](acp-integration.md)
- [ADR-0026](../adr/0026-acp-integration.md)
