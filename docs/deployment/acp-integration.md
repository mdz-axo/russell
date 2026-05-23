# ACP Integration Guide

**Purpose:** Deploy Russell as an ACP agent for hKask integration.

**ADR Reference:** [ADR-0027](../adr/0027-acp-integration.md)

---

## Overview

Russell exposes public skills and host probes to hKask agents via the Agent Client Protocol (ACP), while maintaining a security boundary around private skills and proprioception data.

### Architecture

```
hKask Agent
   │ (ACP over JSON-RPC/stdio)
   ▼
russell-acp-server
   ├── Jack persona (LLM via Okapi)
   ├── Public skill dispatch (8 skills)
   ├── Macaroon auth (OCAP)
   └── Rate limiter (100/min)
   │
   ├── russell-sentinel (5-min cadence)
   │    └── SQLite journal
   │
   └── Private skills (Russell-only)
        └── okapi-watcher, sysadmin, etc.
```

### Security Boundaries

| Category | Exposure | Rationale |
|----------|----------|-----------|
| **Public skills** (8) | ACP-exposed | Read-only, informational |
| **Private skills** (6) | Russell-only | Host mutations, sudo operations |
| **Proprioception** | Never exposed | Self-vitals are security-sensitive |

---

## Deployment

### Prerequisites

1. **Russell installed:** `cargo install --path crates/russell-cli`
2. **Okapi running:** `systemctl --user start okapi`
3. **Skills directory:** `~/.local/share/harness/skills/` populated

### Step 1: Install systemd units

```bash
# Copy unit files
cp docs/deployment/russell-acp-server.service ~/.config/systemd/user/
cp docs/deployment/russell-sentinel.service ~/.config/systemd/user/
cp docs/deployment/russell-sentinel.timer ~/.config/systemd/user/

# Reload systemd
systemctl --user daemon-reload

# Enable and start
systemctl --user enable --now russell-sentinel.timer
systemctl --user enable --now russell-acp-server.service

# Verify status
systemctl --user status russell-acp-server.service
journalctl --user -u russell-acp-server -f
```

### Step 2: Configure macaroon authentication

Create `~/.config/hkask/macaroon.yaml`:

```yaml
issuer:
  root_key: "<32-byte-hex-random>"
  capabilities:
    - name: russell-acp
      attenuations:
        - skill: web-search
        - skill: journal-viewer
        - rate_limit: 100/minute
      before: 24h
```

Generate a root key:

```bash
openssl rand -hex 32
```

### Step 3: Register Russell in hKask

In hKask configuration, add Russell as an ACP agent:

```yaml
agents:
  russell:
    type: acp
    transport: stdio
    command: ["russell-acp-server"]
    auth:
      type: macaroon
      key_file: ~/.config/hkask/macaroon.yaml
```

---

## ACP Methods

Russell ACP server implements the following JSON-RPC methods:

### Session Management

| Method | Description |
|--------|-------------|
| `acp/session.create` | Create a new multi-turn session |
| `acp/session.message` | Send a message in a session |
| `acp/session.close` | Close a session |
| `acp/session.status` | Get session status |

### Capability Discovery

| Method | Description |
|--------|-------------|
| `acp/capabilities` | List public skills and probes |
| `acp/skill/info` | Get info about a specific skill |

### Execution

| Method | Description |
|--------|-------------|
| `acp/probe/run` | Run a read-only probe |
| `acp/skill/run` | Run a skill (probes only, interventions require consent) |

### Example: Run a probe

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "acp/probe/run",
  "params": {
    "skill_id": "web-search",
    "probe_id": "search-query",
    "args": {
      "query": "latest AI news"
    }
  }
}
```

---

## Public Skills

These 8 skills are exposed via ACP:

| Skill | Description | Lexicon |
|-------|-------------|---------|
| `journal-compactor` | Compact SQLite journal | FlowDef |
| `journal-viewer` | Query journal entries | KnowAct |
| `package-checker` | Check package versions | KnowAct |
| `pragmatic-cybernetics` | Cybernetics knowledge | KnowAct |
| `pragmatic-semantics` | Semantic theory | KnowAct |
| `scenario-tester` | Run scenario tests | WordAct |
| `ubuntu-jack` | Ubuntu integration | FlowDef |
| `web-search` | Brave web search | WordAct |

---

## Troubleshooting

### ACP server won't start

```bash
# Check logs
journalctl --user -u russell-acp-server -n 50

# Common issues:
# - Okapi not running: systemctl --user start okapi
# - Skills directory missing: ls ~/.local/share/harness/skills
# - Port conflict: lsof -i :11435
```

### Macaroon auth fails

```bash
# Verify key format (32-byte hex)
cat ~/.config/hkask/macaroon.yaml | grep root_key

# Regenerate key
openssl rand -hex 32
```

### Skills not loading

```bash
# Check skills directory
ls -la ~/.local/share/harness/skills/

# Validate manifest
cargo run --manifest-path crates/russell-skills/Cargo.toml -- test
```

---

## Security Considerations

### JR Principles Compliance

| Principle | Implementation |
|-----------|----------------|
| **JR-1** (Austere) | Minimal crate changes, no bloat |
| **JR-2** (Observe > Act) | Public skills are read-only |
| **JR-3** (No shell from LLM) | LLM ranks IDs, doesn't emit commands |
| **JR-4** (Nurse present) | Jack persona via ACP sessions |
| **JR-5** (Proprioception) | 5 vitals remain private |
| **JR-6** (Reuse) | Independent SQLite journal |
| **JR-7** (Auditable) | ACP calls logged to journal |

### Attack Surface Mitigation

| Risk | Mitigation |
|------|------------|
| **Macaroon leakage** | 24h expiration, 0600 permissions |
| **DoS** | Rate limiting (100/min) |
| **Private skill exposure** | Visibility filter at dispatch layer |
| **Proprioception leak** | Never added to capability registry |

---

## References

- [ADR-0027](../adr/0027-acp-integration.md) — Architecture decisions
- [ACP Specification](https://agentclientprotocol.com)
- hKask hLexicon — separate repository
- Macaroon Example Config — separate repository
