---
title: "Russell Macaroon Client Guide"
audience: [Russell operators, Russell developers]
last_updated: 2026-05-20
togaf_phase: "D"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Application -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-20 -->

# Russell Macaroon Client Guide

This guide describes how Russell ACP agent uses macaroons for authentication with hKask and Okapi.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│              Russell (Macaroon Holder + Attenuator)                 │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │  Macaroon Client:                                            │   │
│  │  - Stores macaroons in secure keychain                       │   │
│  │  - Attenuates macaroons per skill invocation                 │   │
│  │  - Requests discharge for Okapi access                       │   │
│  │  - Auto-refreshes before expiry                              │   │
│  └─────────────────────────────────────────────────────────────┘   │
└────────────────────────────┬────────────────────────────────────────┘
                             │
                             │ MCP tool invocation with macaroon:
                             │ Authorization: Bearer <macaroon>
                             ▼
┌─────────────────────────────────────────────────────────────────────┐
│                    hKask (Macaroon Verifier)                        │
│  Verifies macaroon, enforces caveats, routes to Okapi               │
└─────────────────────────────────────────────────────────────────────┘
```

## Macaroon Storage

### Keychain Storage (Production)

Russell stores macaroons in the OS keychain:

```yaml
# ~/.config/russell/config.yaml
macaroon:
  storage: keychain
  keychain_service: russell-macaroons
  auto_refresh: true
  refresh_before_expiry: 1h
```

### File Storage (Development)

For development, macaroons can be stored in encrypted files:

```yaml
# ~/.config/russell/config.yaml
macaroon:
  storage: file
  file_path: ~/.config/russell/macaroons.json
  encryption: aes-256-gcm
  auto_refresh: true
```

```json
// ~/.config/russell/macaroons.json (encrypted)
{
  "macaroons": [
    {
      "agent_id": "russell-prod-1",
      "skill": "evolution-watcher",
      macaroon": "<base64-encoded-macaroon>",
      "discharge": "<base64-encoded-discharge>",
      "issued_at": "2026-05-20T10:00:00Z",
      "expires_at": "2026-05-21T00:00:00Z"
    }
  ]
}
```

## Skill Registration

### 1. Register Skill with hKask

```bash
russell skill register evolution-watcher
```

Russell sends registration request:

```json
POST http://127.0.0.1:8080/api/v1/agents/russell/skills/register
{
  "agent_id": "russell-prod-1",
  "skill": "evolution-watcher",
  "version": "1.0.0",
  "endpoints_required": [
    "/api/evolution/scan",
    "/api/evolution/propose",
    "/api/evolution/execute"
  ],
  "models_required": ["qwen3:8b", "qKask:70b"],
  "okapi_access_required": true
}
```

### 2. Receive Macaroon from hKask

```json
{
  "status": "registered",
  "skill": "evolution-watcher",
  "macaroon": "<base64-encoded-primary-macaroon>",
  "discharge_endpoint": "http://127.0.0.1:8080/mcp/v1/discharge",
  "expires_at": "2026-05-21T00:00:00Z"
}
```

### 3. Store Macaroon

Russell stores macaroon in keychain:

```go
// Russell macaroon client
func (c *MacaroonClient) StoreMacaroon(m *Macaroon, discharge *Macaroon) error {
    key := fmt.Sprintf("%s:%s", m.Identifier, m.ParseCaveats().Skill)
    
    entry := &MacaroonEntry{
        AgentID:    m.ParseCaveats().IID,
        Skill:      m.ParseCaveats().Skill,
        Macaroon:   m.Serialize(),
        Discharge:  discharge.Serialize(),
        IssuedAt:   time.Now(),
        ExpiresAt:  m.ParseCaveats().Before,
    }
    
    return c.keychain.Set(key, entry)
}
```

## Skill Invocation

### 1. Load Macaroon

```bash
russell skill invoke evolution-watcher --action probe-health
```

Russell loads macaroon from storage:

```go
func (c *MacaroonClient) LoadMacaroon(agentID, skill string) (*Macaroon, error) {
    key := fmt.Sprintf("%s:%s", agentID, skill)
    entry, err := c.keychain.Get(key)
    if err != nil {
        return nil, err
    }
    
    if time.Now().After(entry.ExpiresAt) {
        return nil, fmt.Errorf("macaroon expired")
    }
    
    return macaroon.Deserialize(entry.Macaroon)
}
```

### 2. Attenuate Macaroon

Russell attenuates macaroon for specific endpoint:

```go
func (c *MacaroonClient) Attenuate(m *Macaroon, endpoint string) (*Macaroon, error) {
    attenuated := *m
    attenuated.AddFirstPartyCaveat(fmt.Sprintf("endpoint:%s", endpoint))
    return &attenuated, nil
}
```

### 3. Request Discharge (for Okapi access)

```go
func (c *MacaroonClient) RequestDischarge(m *Macaroon) (*Macaroon, error) {
    caveats := m.ParseCaveats()
    
    req := DischargeRequest{
        PrimaryMacaroon: m.Serialize(),
        Location:        "okapi-access",
        AgentID:         caveats.IID,
        Skill:           caveats.Skill,
    }
    
    resp, err := c.httpClient.Post(c.dischargeEndpoint, req)
    if err != nil {
        return nil, err
    }
    
    return macaroon Deserialize(resp.DischargeMacaroon)
}
```

### 4. Bind Discharge to Primary

```go
func (c *MacaroonClient) BindDischarge(primary, discharge *Macaroon) *Macaroon {
    return primary.Bind(discharge)
}
```

### 5. Invoke MCP Tool

```go
func (c *MacaroonClient) InvokeMCPTool(tool string, args any, m *Macaroon) (any, error) {
    req, err := http.NewRequest("POST", c.mcpEndpoint+"/tools/"+tool, args)
    if err != nil {
        return nil, err
    }
    
    req.Header.Set("Authorization", "Bearer "+m.Serialize())
    req.Header.Set("Content-Type", "application/json")
    
    resp, err := c.httpClient.Do(req)
    if err != nil {
        return nil, err
    }
    
    return resp.Body, nil
}
```

## Auto-Refresh

Russell automatically refreshes macaroons before expiry:

```go
type MacaroonClient struct {
    keychain       *Keychain
    httpClient     *http.Client
    dischargeEndpoint string
    mcpEndpoint    string
    refreshTicker  *time.Ticker
    refreshBefore  time.Duration
}

func (c *MacaroonClient) StartAutoRefresh() {
    c.refreshTicker = time.NewTicker(c.refreshBefore)
    
    go func() {
        for range c.refreshTicker.C {
            entries, err := c.keychain.List()
            if err != nil {
                continue
            }
            
            for _, entry := range entries {
                if time.Now().Add(c.refreshBefore).After(entry.ExpiresAt) {
                    c.refreshMacaroon(entry)
                }
            }
        }
    }()
}

func (c *MacaroonClient) refreshMacaroon(entry *MacaroonEntry) {
    // Request new macaroon from hKask
    resp, err := c.registerSkill(entry.AgentID, entry.Skill)
    if err != nil {
        slog.Warn("macaroon refresh failed", "error", err)
        return
    }
    
    // Store new macaroon
    newM, _ := macaroon.Deserialize(resp.Macaroon)
    newD, _ := macaroon Deserialize(resp.DischargeMacaroon)
    c.StoreMacaroon(newM, newD)
    
    slog.Info("macaroon refreshed", "skill", entry.Skill)
}
```

## Error Handling

### Macaroon Expired

```
Error: macaroon expired
Caveat: before
Expired at: 2026-05-19T23:59:59Z
```

**Russell action:** Automatically refresh macaroon before expiry.

### Endpoint Not Allowed

```
Error: endpoint not allowed
Caveat: endpoint
Expected: ["/api/evolution/scan"]
Received: "/api/embed"
```

**Russell action:** Attenuate macaroon with correct endpoint caveat.

### Discharge Required

```
Error: third-party caveat requires discharge
Location: okapi-access
```

**Russell action:** Request discharge from hKask MCP discharge endpoint.

### Quota Exceeded

```
Error: quota exceeded
Code: quota_exceeded
Quota:
  tokens_per_day: 1000000
  remaining: 0
  reset_at: 2026-05-21T00:00:00Z
```

**Russell action:** Wait for quota reset or alert operator.

## Configuration Reference

```yaml
# ~/.config/russell/config.yaml
hKask:
  mcp_endpoint: http://127.0.0.1:8080/mcp
  agent_id: russell-prod-1

macaroon:
  storage: keychain  # or "file"
  keychain_service: russell-macaroons
  file_path: ~/.config/russell/macaroons.json
  encryption: aes-256-gcm
  
  auto_refresh: true
  refresh_before_expiry: 1h
  
  okapi:
    discharge_endpoint: http://127.0.0.1:8080/mcp/v1/discharge
    endpoint: http://127.0.0.1:11435
  
  audit:
    enabled: true
    log_file: ~/.config/russell/macaroon_audit.log
```

## CLI Commands

### List Stored Macaroons

```bash
russell macaroon list
```

Output:
```
AGENT ID          SKILL              EXpires AT          STATUS
russell-prod-1    evolution-watcher  2026-05-21T00:00:00Z  active
russell-prod-1    rdf-embedding      2026-05-21T00:00:00Z  active
```

### Refresh Macaroon

```bash
russell macaroon refresh --skill evolution-watcher
```

### Revoke Macaroon

```bash
russell macaroon revoke --skill evolution-watcher
```

### Verify Macaroon

```bash
russell macaroon verify --skill evolution-watcher
```

## Audit Logging

Russell logs macaroon operations:

```json
{
  "timestamp": "2026-05-20T10:30:00Z",
  "event": "macaroon_stored",
  "agent_id": "russell-prod-1",
  "skill": "evolution-watcher",
  "expires_at": "2026-05-21T00:00:00Z"
}
```

```json
{
  "timestamp": "2026-05-20T10:31:00Z",
  "event": "macaroon_attenuated",
  "agent_id": "russell-prod-1",
  "skill": "evolution-watcher",
  "endpoint": "/api/evolution/scan"
}
```

```json
{
  "timestamp": "2026-05-20T10:32:00Z",
  "event": "discharge_requested",
  "agent_id": "russell-prod-1",
  "skill": "evolution-watcher",
  "location": "okapi-access"
}
```

```json
{
  "timestamp": "2026-05-20T10:33:00Z",
  "event": "mcp_tool_invoked",
  "agent_id": "russell-prod-1",
  "skill": "evolution-watcher",
  "tool": "inference/generate",
  "model": "qwen3:8b"
}
```

## Security Best Practices

| Practice | Recommendation |
|----------|---------------|
| Storage | Use OS keychain (not file) in production |
| Encryption | AES-256-GCM for file storage |
| Refresh | Auto-refresh 1 hour before expiry |
| Revocation | Revoke macaroons on skill deregistration |
| Audit | Log all macaroon operations |
| TLS | Mandatory for all macaroon transmission |

## References

- `fork-docs/AUTH_SPEC.md` — Okapi macaroon authentication
- `fork-docs/MACAROON_SPEC.md` — Macaroon caveat vocabulary
- `fork-docs/MACAROON_DEPLOYMENT.md` — Deployment guide
- `hKask/docs/integrations/macaroon-issuer.md` — hKask macaroon issuer
- `server/macaroon_auth.go` — Okapi macaroon middleware
- `cmd/cmd_macaroon.go` — Macaroon CLI commands

---

*Russell v0.22.0 — Macaroon client for hKask ACP agent*