# ADR-0044: CNS Span Emission in ACP Server

**Date:** 2026-05-24  
**Status:** Implemented  
**Author:** Russell Team  
**Deciders:** Operator  
**Technical Story:** Tier 2 recommendation — CNS span emission wiring

---

## Context

Russell's agent pod (`russell-agent`) defines a `CnsEmitter` that sends structured observability spans to hKask's Central Nervous System (CNS) endpoint. However, the ACP server (`russell-acp-server`) had no CNS emission, creating an observability gap:

- Session creation events not visible to hKask
- Skill dispatch events not tracked
- LLM escalation events not monitored
- Consent decisions not auditable

The adversarial review (2026-05-23) identified this as incomplete observability: "CNS span emission wiring into ACP/Nurse pipeline" was listed as Tier 2 architectural debt.

---

## Decision

Implement `AcpCnsEmitter` in `russell-acp-server` that emits structured spans for key ACP events.

### Span Types

| Event | Span Name | Attributes |
|-------|-----------|------------|
| Session created | `cns.russell.acp.session.created` | `session_id`, `persona` |
| Skill dispatched | `cns.russell.acp.skill.dispatch` | `skill_id`, `action` |
| LLM escalation | `cns.russell.acp.llm.escalation` | `backend`, `model`, `latency_ms` |
| Consent decision | `cns.russell.acp.consent.decision` | `action_id`, `decision` |

### Implementation

**File:** `crates/russell-acp-server/src/cns.rs`

```rust
pub struct AcpCnsEmitter {
    source: String,                    // "russell-acp-server"
    cns_endpoint: Option<String>,      // from $HKASK_CNS_ENDPOINT
    http_client: Option<reqwest::Client>,
}

impl AcpCnsEmitter {
    pub fn new(source: impl Into<String>) -> Self {
        let cns_endpoint = std::env::var("HKASK_CNS_ENDPOINT").ok();
        let http_client = cns_endpoint
            .as_ref()
            .and_then(|_| reqwest::Client::builder().build().ok());
        Self { source: source.into(), cns_endpoint, http_client }
    }

    pub fn emit_session_created(&self, session_id: &str, persona: &str) {
        let span = AcpCnsSpan {
            name: "cns.russell.acp.session.created".to_string(),
            timestamp: Utc::now(),
            source: self.source.clone(),
            attributes: json!({ "session_id": session_id, "persona": persona }),
        };
        self.emit(span);
    }

    pub fn emit_skill_dispatched(&self, skill_id: &str, action: &str) { /* ... */ }
    pub fn emit_llm_escalation(&self, backend: &str, model: Option<&str>, latency_ms: u64) { /* ... */ }
    pub fn emit_consent_decision(&self, action_id: &str, decision: &str) { /* ... */ }
}
```

### Emission Mechanism

- **Async fire-and-forget** — `tokio::spawn` sends span via HTTP POST
- **5-second timeout** — Prevents blocking on slow CNS endpoint
- **Graceful degradation** — If no endpoint configured, logs locally (JR-2 compliance)

### Wiring

**File:** `crates/russell-acp-server/src/main.rs`

```rust
let cns = AcpCnsEmitter::new("russell-acp-server");
let handler = AcpHandler::new(persona, dispatch, auth, rate_limiter)
    .with_cns(cns)
    .with_inference(inference);
```

**File:** `crates/russell-acp-server/src/handler.rs`

```rust
// In create_session()
if let Some(ref cns) = self.cns {
    cns.emit_session_created(&session_id, &req.persona);
}

// In run_skill()
if let Some(ref cns) = self.cns {
    cns.emit_skill_dispatched(&skill_id, "run");
}

// In session_message() after inference
if let Some(ref cns) = self.cns {
    cns.emit_llm_escalation(&resp.backend, resp.model.as_deref(), resp.latency_ms.unwrap_or(0));
}

// In consent_respond()
if let Some(ref cns) = self.cns {
    cns.emit_consent_decision(&req.action_id, decision_text);
}
```

---

## Consequences

### Positive

- **Complete observability** — hKask CNS receives all ACP events
- **Structured spans** — Consistent format with `russell-agent` CNS emission
- **Graceful degradation** — Works without CNS endpoint (local logging)
- **Async emission** — Non-blocking, 5s timeout prevents stalls

### Negative

- **Network dependency** — Requires HTTP connectivity to CNS endpoint
- **Fire-and-forget** — No delivery confirmation (acceptable for observability)
- **Span volume** — High-traffic ACP servers may generate many spans

### Neutral

- **No breaking changes** — CNS emission is additive, optional
- **Opt-in** — Only active if `HKASK_CNS_ENDPOINT` is configured

---

## Compliance

| Principle | Compliance |
|---|---|
| **JR-2** (Observe > Recommend > Act) | Observability is read-only, no mutations |
| **JR-5** (Proprioception) | ACP events contribute to Russell's self-observation |
| **Schneier** (Defense in depth) | Async emission with timeout prevents DoS |

---

## Implementation

**Files created:**
- `crates/russell-acp-server/src/cns.rs` — `AcpCnsEmitter`, `AcpCnsSpan`

**Files modified:**
- `crates/russell-acp-server/src/lib.rs` — Export `AcpCnsEmitter`
- `crates/russell-acp-server/src/handler.rs` — Wire CNS emission into 4 methods
- `crates/russell-acp-server/src/main.rs` — Initialize `AcpCnsEmitter`
- `crates/russell-acp-server/Cargo.toml` — Add `reqwest` dependency

**Tests:**
- Existing ACP server tests continue to pass (290 total)
- Manual verification: spans emitted to CNS endpoint when configured

---

## Future Work

1. **Span batching** — Batch multiple spans into single HTTP request to reduce overhead
2. **Span sampling** — Sample low-severity spans to reduce volume
3. **Span correlation** — Add `trace_id` to correlate spans across session lifecycle
4. **CNS health check** — Emit `cns.russell.acp.health` span periodically

---

## References

- [ADR-0027: hKask ACP Integration](0027-acp-integration.md)
- [ADR-0041: ACP Consent Protocol](0041-acp-consent-protocol.md)
- Adversarial Review Action Plan (2026-05-23) §Tier 2 recommendations
- `crates/russell-agent/src/cns.rs` — Agent pod CNS emission (reference implementation)
