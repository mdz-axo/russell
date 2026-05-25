---
title: "ADR-0043: Proprioception ACP Notification Protocol"
audience: [developers, architects]
last_updated: 2026-05-24
ddmvss_context: "proprioception"
ddmvss_artifact: "adr"
version: "1.0.0"
status: "Implemented"
---



---
title: "ADR-0043: Proprioception ACP Notification Protocol"
audience: [developers, architects]
last_updated: 2026-05-24
togaf_phase: "H"
version: "1.0.0"
status: "Implemented"
---


# ADR-0043: Proprioception ACP Notification Protocol

## Decision

Implement `acp/notifications.list` JSON-RPC method that returns recent proprioception breach events as structured notifications.

### Protocol

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "acp/notifications.list",
  "params": {
    "hours": 24
  },
  "auth": { "auth_type": "macaroon", "token": "..." }
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "notifications": [
      {
        "id": "evt_123",
        "vital": "hkask_mcp_reachable_ms",
        "severity": "warn",
        "value": "2500",
        "threshold": "2000",
        "summary": "hkask_mcp_reachable_ms = 2500 (threshold: 2000)",
        "timestamp": "2026-05-24T10:30:00Z"
      }
    ],
    "total": 1
  }
}
```

### Parameters

- **`hours`** (optional, default: 24, max: 168) — Time window to query (in hours)

### Notification Structure

```rust
pub struct ProprioNotification {
    pub id: String,              // Event ID from journal
    pub vital: String,           // Vital name (e.g., "hkask_mcp_reachable_ms")
    pub severity: String,        // "warn", "alert", "critical"
    pub value: serde_json::Value,
    pub threshold: serde_json::Value,
    pub summary: String,         // Human-readable summary
    pub timestamp: String,       // ISO 8601
}
```

### Implementation

**File:** `crates/russell-acp-server/src/handler.rs`

```rust
async fn list_notifications(
    &self,
    params: Option<serde_json::Value>,
) -> Result<serde_json::Value> {
    let hours = params
        .as_ref()
        .and_then(|p| p.get("hours"))
        .and_then(|v| v.as_i64())
        .unwrap_or(24)
        .min(168); // Cap at 7 days

    let reader = self.journal_reader.as_ref()
        .ok_or_else(|| AcpError::Internal("journal reader not configured".into()))?;

    let since_unix = russell_core::time::now_unix() - (hours * 3600);

    // Query self_vital_breach events from journal
    let events = reader
        .list_events_by_action("self_vital_breach", since_unix, i64::MAX)
        .map_err(|e| AcpError::Internal(format!("journal query failed: {e}")))?;

    // Parse events into structured notifications
    let notifications: Vec<ProprioNotification> = events
        .iter()
        .filter_map(|row| {
            let summary = row.summary.as_deref()?;
            // Parse "vital = value (threshold: threshold)" format
            // ...
        })
        .collect();

    Ok(json!(NotificationsResponse {
        notifications,
        total: notifications.len(),
    }))
}
```

### Security Model

- **Authentication required** — Only authenticated hKask agents can query notifications
- **Read-only** — No mutations, no consent required
- **Bounded query** — Max 7-day window to prevent excessive data retrieval
- **Journal reader** — Uses read-only `JournalReader`, not `JournalWriter`

---

## Consequences

### Positive

- **Proactive monitoring** — hKask agents can query Russell's health status
- **Structured data** — Notifications include vital name, value, threshold, severity
- **Bounded queries** — Time window prevents excessive data retrieval
- **Audit trail** — All queries logged via ACP request logging

### Negative

- **Polling model** — Agents must poll for notifications (no push)
- **Parse fragility** — Relies on summary format "vital = value (threshold: threshold)"
- **Journal dependency** — Requires journal reader to be configured

### Neutral

- **No breaking changes** — New method, existing methods unchanged
- **Opt-in** — Agents only query if they need health monitoring

---

## Compliance

| Principle | Compliance |
|---|---|
| **JR-5** (Proprioception) | Self-vitals exposed via ACP for agent monitoring |
| **JR-2** (Observe > Recommend > Act) | Read-only observation, no mutations |
| **Schneier** (Defense in depth) | Bounded query window, authentication required |

---

## Implementation

**Files modified:**
- `crates/russell-acp-server/src/types.rs` — Added `ProprioNotification`, `NotificationsResponse`
- `crates/russell-acp-server/src/handler.rs` — Added `list_notifications()`, `with_journal_reader()`
- `crates/russell-acp-server/src/main.rs` — Initialize `JournalReader` and wire into handler

**Tests:**
- Existing journal query tests continue to pass
- Manual verification: query returns structured notifications

---

## Future Work

1. **Push notifications** — Implement `acp/notifications.subscribe` for real-time push via WebSocket or SSE
2. **Notification filtering** — Allow agents to filter by vital name or severity
3. **Notification acknowledgment** — Allow agents to acknowledge notifications (clear from queue)
4. **Notification aggregation** — Group related breaches (e.g., "LLM degradation" = high latency + high error rate)

---

## References

- [ADR-0021: Proprioception Phase 2A](0021-proprioception-phase2-reflex-arcs.md)
- [ADR-0027: hKask ACP Integration](0027-acp-integration.md)
- Adversarial Review Action Plan (2026-05-23) §Tier 2 recommendations
