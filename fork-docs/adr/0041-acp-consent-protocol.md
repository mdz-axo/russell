---
title: "ADR-0041: ACP Consent Protocol"
audience: [developers, architects, operators]
last_updated: 2026-05-23
ddmvss_context: "acp"
ddmvss_artifact: "adr"
version: "1.0.0"
status: "Implemented"
---



# ADR-0041: ACP Consent Protocol

## Context

Russell's ACP server exposes skills to hKask agents. Some skills contain interventions that require explicit operator consent before execution (per JR-2: Observe > Recommend > Act). The adversarial review (2026-05-23) identified that interventions requiring consent had no mechanism for hKask agents to:

1. Surface a `PendingAction` to the operator
2. Receive consent (approve/deny) in a subsequent message
3. Execute the approved action with the original arguments

This created a gap: the consent gate was CLI-only (`russell chat`), not available via ACP. hKask agents could not complete the full observe → recommend → consent → act loop.

---

## Decision

Implement a consent protocol over ACP with two new JSON-RPC methods:

### 1. `acp/consent.respond`

Allows hKask agents to respond to a pending consent request.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "acp/consent.respond",
  "params": {
    "session_id": "sess_abc123",
    "action_id": "action_xyz789",
    "decision": "approve",
    "reason": "Operator approved cache cleanup"
  },
  "auth": { "auth_type": "macaroon", "token": "..." }
}
```

**Response (approved):**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "session_id": "sess_abc123",
    "action_id": "action_xyz789",
    "decision": "approve",
    "result": "Intervention executed successfully",
    "error": null
  }
}
```

**Response (denied):**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "session_id": "sess_abc123",
    "action_id": "action_xyz789",
    "decision": "deny",
    "result": null,
    "error": "Action denied by operator"
  }
}
```

### 2. Session State Machine

Sessions now track consent state:

```
Active → InputRequired (pending action) → Active (after consent)
```

When a session message triggers an intervention requiring consent:
1. Session state transitions to `InputRequired`
2. `pending_action` field populated with action details
3. Response includes `pending_action` in `SessionMessageResponse`

### 3. PendingAction Structure

```rust
pub struct PendingAction {
    pub action_type: String,        // "intervention"
    pub skill_id: String,           // "sysadmin"
    pub intervention_id: String,    // "sweep-caches"
    pub risk: RiskBand,             // "medium"
    pub requires_consent: bool,     // true
    pub action_id: String,          // UUID v4
    pub args: serde_json::Value,    // Original arguments
}
```

### 4. ConsentDecision Enum

```rust
pub enum ConsentDecision {
    Approve,
    Deny,
}
```

### 5. Security Model

- **Session ownership verification** — Only the token that created the session can respond to its consent requests
- **Action ID matching** — Consent response must match the pending action's `action_id` (prevents replay attacks)
- **State validation** — Consent only accepted when session is in `InputRequired` state
- **Audit trail** — Consent decisions recorded as conversation turns with operator attribution

### Location

- **Types:** `crates/russell-acp-server/src/types.rs` (ConsentRequest, ConsentResponse, ConsentDecision, PendingAction)
- **Session state:** `crates/russell-acp-server/src/session.rs` (pending_action field)
- **Handler:** `crates/russell-acp-server/src/handler.rs` (consent_respond method)

---

## Consequences

### Positive

- **Complete consent loop** — hKask agents can now surface interventions, receive consent, and execute approved actions via ACP
- **Security** — Session ownership + action ID matching prevents unauthorized consent responses
- **Audit trail** — All consent decisions recorded in session history with operator attribution
- **State clarity** — `InputRequired` state makes it explicit when a session is waiting for operator input
- **Backward compatible** — Existing sessions without pending actions continue to work unchanged

### Negative

- **State complexity** — Sessions now have three states (Active, InputRequired, Closed) instead of two
- **Blocking behavior** — Sessions in `InputRequired` state cannot accept new messages until consent is resolved
- **Timeout risk** — If operator never responds, session remains blocked indefinitely (mitigation: session GC after 1 hour)

### Neutral

- **No breaking changes** — `acp/consent.respond` is a new method; existing methods unchanged
- **Opt-in** — Only interventions with `requires_consent: true` trigger the consent flow

---

## Compliance

| Principle | Compliance |
|---|---|
| **JR-2** (Observe > Recommend > Act) | Interventions require explicit consent before execution |
| **JR-3** (Consent gate) | Shell commands and interventions go through the consent gate; destructive commands blocked; operator's consent is sovereign |
| **Schneier** (Defense in depth) | Session ownership + action ID matching + state validation |
| **Miller** (Capability discipline) | Consent is scoped to specific action_id, not blanket approval |

---

## Implementation

**Files created:**
- None (all additions to existing files)

**Files modified:**
- `crates/russell-acp-server/src/types.rs` — Added ConsentRequest, ConsentResponse, ConsentDecision, extended PendingAction with `args` field
- `crates/russell-acp-server/src/session.rs` — Added `pending_action` field to Session
- `crates/russell-acp-server/src/handler.rs` — Added `consent_respond` method, wired into JSON-RPC dispatcher

**Tests:**
- Existing session tests continue to pass (290 total tests passing)
- Consent protocol tested via integration tests (manual verification)

---

## Future Work

1. **Consent timeout** — Auto-deny pending actions after configurable timeout (e.g., 5 minutes)
2. **Batch consent** — Allow operator to approve/deny multiple pending actions in one response
3. **Consent history** — Query past consent decisions via `acp/consent.history` method
4. **Consent policies** — Pre-approve specific interventions (e.g., "always allow cache cleanup")
5. **Consent notifications** — Push notification to operator when consent is required (via hKask UI)

---

## References

- [ADR-0027: hKask ACP Integration](0027-acp-integration.md)
- [ADR-0036: Andon Cord for Reflex Arcs](deferred/0036-andon-cord-reflex-arcs.md) (deferred, consent absorbed here)
- Adversarial Review Action Plan (2026-05-23) §Tier 1 recommendations
- JR-2: Observe > Recommend > Act
