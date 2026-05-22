# Phase 0.3: ACP Server Interface Design

**Date:** 2026-05-22  
**Status:** Proposed  
**ADR:** [ADR-0026: hKask ACP Integration](../adr/0026-acp-integration.md)  
**Reference:** hKask's [`stack-acp-server`](../../hKask/stack/crates/stack-acp-server/)

---

## Overview

This document defines the interface surface for `russell-acp-server` — the ACP session-oriented wrapper over Russell's capabilities (public skills, host probes, Jack persona).

The design follows hKask's `stack-acp-server` pattern:
- **Session-oriented** (multi-turn conversations with accumulated context)
- **Persona-projected** (Jack's nurse persona from `russell-meta`)
- **Capability-gated** (only public skills exposed; visibility enforced)
- **Macaroon-authenticated** (OCAP capability tokens with attenuation)

---

## JSON-RPC Methods

Russell ACP server implements these methods over stdio JSON-RPC:

### Session Methods

| Method | Description | Input | Output |
|--------|-------------|-------|--------|
| `acp/session.create` | Start new conversation session | `{persona?: string}` | `{session_id, created_at}` |
| `acp/session.message` | Send message in session | `{session_id, message, correlation_id}` | `{session_id, response, turns}` |
| `acp/session.close` | End session (cleanup) | `{session_id}` | `{session_id, closed_at}` |
| `acp/session.status` | Get session state | `{session_id}` | `{session_id, turn_count, last_activity, persona}` |

### Capability Methods

| Method | Description | Input | Output |
|--------|-------------|-------|--------|
| `acp/capabilities` | List public skills + probes | `{}` | `{skills: [...], probes: [...]}` |
| `acp/skill/info` | Get skill metadata | `{skill_id}` | `{skill: SkillInfo}` |
| `acp/skill/run` | Run public skill (via session) | `{session_id, skill_id, args}` | `{result, evidence_bundle}` |
| `acp/probe/run` | Run host probe (read-only) | `{probe_id}` | `{probe_result}` |

### A2A Gateway Methods

| Method | Description | Input | Output |
|--------|-------------|-------|--------|
| `acp/a2a.dispatch` | Dispatch A2A envelope | `{envelope: A2aEnvelope}` | `{response: A2aMessage}` |
| `acp/a2a.delegate` | Delegate goal to hKask agent | `{goal, priority, constraints}` | `{delegation_id, status}` |
| `acp/a2a.result` | Share result with hKask | `{delegation_id, artifacts}` | `{acknowledged}` |

---

## Type Definitions

### Session Types

```rust
/// ACP session — multi-turn conversation with Jack persona.
pub struct Session {
    pub id: String,                     // UUID v4
    pub persona: JackPersonaProjection, // from russell-meta
    pub turns: Vec<Turn>,
    pub created: Timestamp,
    pub last_activity: Timestamp,
    pub state: SessionState,
}

pub enum SessionState {
    Active,
    InputRequired,  // Jack waiting for operator consent
    Completed,
    Closed,
}

pub struct Turn {
    pub id: String,
    pub role: TurnRole,
    pub content: String,
    pub tool_calls: Vec<ToolCallRecord>,
    pub timestamp: Timestamp,
}

pub enum TurnRole {
    User,       // Operator or hKask agent
    Assistant,  // Jack persona
    Tool,       // MCP tool response
}

pub struct ToolCallRecord {
    pub skill_id: String,
    pub intervention_id: Option<String>,
    pub probe_id: Option<String>,
    pub args: serde_json::Value,
    pub result: String,
    pub visibility: Visibility,  // Public | Private (for audit)
}
```

### Capability Types

```rust
/// Public skill metadata (exposed via ACP).
pub struct SkillInfo {
    pub id: String,
    pub version: String,
    pub description: String,
    pub visibility: Visibility,
    pub lexicon: LexiconCategorization,
    pub symptoms: Vec<String>,
    pub probes: Vec<ProbeInfo>,
    pub interventions: Vec<InterventionInfo>,
    pub safety: SafetyInfo,
}

pub struct ProbeInfo {
    pub id: String,
    pub description: String,
    pub timeout: Duration,
    pub risk: RiskLevel,  // Always "none" for probes
}

pub struct InterventionInfo {
    pub id: String,
    pub description: String,
    pub risk: RiskLevel,
    pub needs_sudo: bool,
    pub rollback: RollbackInfo,
}

pub enum RiskLevel {
    None,
    Low,
    Medium,
    High,
    Critical,
}

pub struct LexiconCategorization {
    pub primary: LexiconDomain,
    pub terms: Vec<String>,
}

pub enum LexiconDomain {
    WordAct,
    FlowDef,
    KnowAct,
}
```

### A2A Types

```rust
/// A2A envelope (from hKask's protocol layer).
pub struct A2aEnvelope {
    pub sender: AgentId,
    pub receiver: AgentId,
    pub message: A2aMessage,
    pub timestamp: HlcTimestamp,
    pub correlation_id: String,
}

/// A2A message payload (task delegation, result sharing, impasse).
pub enum A2aMessage {
    DelegateGoal { goal: Goal, priority: u8, constraints: Vec<Constraint> },
    DelegationResult { goal_id: String, outcome: Phase, artifacts: Vec<String> },
    ShareResult { content: String, facts: Vec<Fact> },
    EscalateImpasse { description: String, attempted: Vec<String> },
    ImpasseResolution { strategy: String, explanation: String },
    JoinEnsemble { session_id: String, role: String, capabilities: Vec<String> },
    Contribute { content: String, speech_act: SpeechAct, session_id: String },
    Depart { session_id: String, reason: String },
}
```

### Authentication Types

```rust
/// Macaroon capability token (OCAP).
pub struct CapabilityToken {
    pub token: String,
    pub capabilities: Vec<String>,  // ["acp:session", "skill:web-search", ...]
    pub attenuations: Vec<Attenuation>,
    pub expires_at: Timestamp,
    pub issuer: String,
}

pub struct Attenuation {
    pub kind: AttenuationKind,
    pub value: String,
}

pub enum AttenuationKind {
    SkillRestriction(String),     // Only this skill
    RateLimit(u32),               // Max calls per minute
    TimeBound(Timestamp),         // Expires at
    DischargeChain(Vec<String>),  // Third-party discharge chain
}
```

---

## Error Taxonomy

```rust
/// ACP server error types.
#[derive(Debug, thiserror::Error)]
pub enum AcpError {
    /// Session not found.
    #[error("session '{0}' not found")]
    SessionNotFound(String),

    /// Session already closed.
    #[error("session '{0}' is closed")]
    SessionClosed(String),

    /// Skill is private (not exposed via ACP).
    #[error("skill '{0}' is private and not exposed via ACP")]
    SkillNotExposed(String),

    /// Skill not found in registry.
    #[error("skill '{0}' not found in registry")]
    SkillNotFound(String),

    /// Probe not found.
    #[error("probe '{0}' not found")]
    ProbeNotFound(String),

    /// Macaroon authentication failed.
    #[error("macaroon authentication failed: {0}")]
    AuthFailed(String),

    /// Capability token expired.
    #[error("capability token expired at {0}")]
    TokenExpired(Timestamp),

    /// Capability not granted (skill not in token's attenuation list).
    #[error("capability '{0}' not granted by token")]
    CapabilityNotGranted(String),

    /// Rate limit exceeded.
    #[error("rate limit exceeded: {0} calls/minute")]
    RateLimitExceeded(u32),

    /// Invalid JSON-RPC request.
    #[error("invalid JSON-RPC request: {0}")]
    InvalidRequest(String),

    /// Internal dispatch error (IDRS failure, probe timeout, etc.).
    #[error("dispatch error: {0}")]
    DispatchError(String),

    /// A2A envelope parse error.
    #[error("A2A envelope parse error: {0}")]
    A2aParseError(String),

    /// hKask MCP client error (transport, auth, timeout).
    #[error("hKask MCP error: {0}")]
    McpError(String),
}

/// JSON-RPC error response format.
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

impl From<AcpError> for JsonRpcError {
    fn from(err: AcpError) -> Self {
        let (code, data) = match &err {
            AcpError::SessionNotFound(_) => (404, None),
            AcpError::SkillNotExposed(_) => (403, Some(json!({"visibility": "private"}))),
            AcpError::AuthFailed(_) => (401, None),
            AcpError::TokenExpired(_) => (401, Some(json!({"expired": true}))),
            AcpError::CapabilityNotGranted(_) => (403, Some(json!({"capability": "not_granted"}))),
            AcpError::RateLimitExceeded(_) => (429, Some(json!({"retry_after": 60}))),
            _ => (500, None),
        };
        Self {
            code,
            message: err.to_string(),
            data,
        }
    }
}
```

---

## Message Formats

### Session Create Request

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "acp/session.create",
  "params": {
    "persona": "jack"  // Optional; defaults to "jack"
  }
}
```

### Session Create Response

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "session_id": "sess_abc123xyz",
    "created_at": "2026-05-22T17:30:00Z",
    "persona": "jack"
  }
}
```

### Session Message Request

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "acp/session.message",
  "params": {
    "session_id": "sess_abc123xyz",
    "message": "Check Okapi health and restart if needed",
    "correlation_id": "corr_xyz789"
  }
}
```

### Session Message Response

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "session_id": "sess_abc123xyz",
    "response": "I'll check Okapi's health now.",
    "turns": [
      {
        "id": "turn_1",
        "role": "user",
        "content": "Check Okapi health and restart if needed",
        "timestamp": "2026-05-22T17:30:15Z"
      },
      {
        "id": "turn_2",
        "role": "assistant",
        "content": "I'll check Okapi's health now.",
        "tool_calls": [
          {
            "skill_id": "okapi-watcher",
            "probe_id": "probe-health",
            "args": {},
            "result": "Okapi running, p95 latency 250ms"
          }
        ],
        "timestamp": "2026-05-22T17:30:20Z"
      }
    ],
    "state": "input_required",
    "pending_action": {
      "type": "intervention",
      "skill_id": "okapi-watcher",
      "intervention_id": "restart-okapi",
      "risk": "low",
      "requires_consent": true
    }
  }
}
```

### Capabilities Request

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "acp/capabilities",
  "params": {}
}
```

### Capabilities Response

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "skills": [
      {
        "id": "web-search",
        "version": "1.0.0",
        "description": "Web search, fetch, and browse capabilities",
        "visibility": "public",
        "lexicon": {
          "primary": "WordAct",
          "terms": ["query", "probe", "report", "summon", "challenge"]
        },
        "symptoms": ["search_capability_needed", "web_knowledge_gap"],
        "probes": [],
        "interventions": [],
        "safety": {"max_auto_risk": "none"}
      },
      {
        "id": "journal-viewer",
        "version": "0.1.0",
        "description": "Simple probes for viewing journal-derived data",
        "visibility": "public",
        "lexicon": {
          "primary": "FlowDef",
          "terms": ["sequence", "filter", "route"]
        },
        "symptoms": ["skill_not_in_catalog"],
        "probes": [{"id": "show-host-samples", "timeout": "5s"}],
        "interventions": [],
        "safety": {"max_auto_risk": "none"}
      }
    ],
    "probes": [
      {
        "id": "memory",
        "description": "System memory usage %",
        "timeout": "5s"
      },
      {
        "id": "swap",
        "description": "Swap usage %",
        "timeout": "5s"
      },
      {
        "id": "gpu-vram",
        "description": "GPU VRAM usage %",
        "timeout": "5s"
      }
    ]
  }
}
```

---

## Integration Points

### russell-meta (Jack Persona)

```rust
// In russell-acp-server/src/persona.rs
use russell_meta::JackPersona;

pub struct JackPersonaProjection {
    inner: JackPersona,
    system_prompt: String,
}

impl JackPersonaProjection {
    pub fn new() -> Result<Self> {
        let inner = JackPersona::load()?;  // From russell-meta/prompts/jack.md
        let system_prompt = format!(
            "You are Jack, Russell's nurse persona. {}\n\
             You observe the host, run probes, and recommend actions.\n\
             You NEVER emit shell commands — you rank intervention IDs.",
            inner.prompt_template
        );
        Ok(Self { inner, system_prompt })
    }
}
```

### russell-skills (Visibility Enforcement)

```rust
// In russell-acp-server/src/dispatch.rs
use russell_skills::{SkillRegistry, Visibility};

impl AcpDispatch {
    pub fn load_public_skills(&self) -> Vec<Skill> {
        self.registry
            .all_skills()
            .filter(|s| s.visibility == Visibility::Public)
            .collect()
    }

    pub async fn dispatch_skill(&self, id: &str, args: &Value) -> Result<String> {
        let skill = self.registry.get_skill(id)?;
        match skill.visibility {
            Visibility::Public => self.dispatcher.run(id, args).await,
            Visibility::Private => Err(AcpError::SkillNotExposed(id.to_string())),
        }
    }
}
```

### russell-sentinel (Host Probes)

```rust
// In russell-acp-server/src/probe.rs
use russell_sentinel::{ProbeRegistry, ProbeResult};

pub struct ProbeRunner {
    registry: ProbeRegistry,
}

impl ProbeRunner {
    pub async fn run_probe(&self, id: &str) -> Result<ProbeResult> {
        let probe = self.registry.get(id)?;
        probe.execute().await  // From russell-sentinel
    }

    pub fn list_probes(&self) -> Vec<ProbeInfo> {
        self.registry.all_probes().map(|p| ProbeInfo {
            id: p.id.clone(),
            description: p.description.clone(),
            timeout: p.timeout,
            risk: RiskLevel::None,  // All probes are risk:none
        }).collect()
    }
}
```

---

## Security Considerations

### 1. Macaroon Authentication

Every ACP request must include a valid macaroon token:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "acp/session.create",
  "params": {
    "persona": "jack"
  },
  "auth": {
    "type": "macaroon",
    "token": "<base64-encoded-macaroon>"
  }
}
```

**Validation:**
- Token signature verified against hKask's root key
- Capabilities checked (must include `acp:session`)
- Attenuations enforced (skill restrictions, rate limits)
- Expiration checked (24h max)

### 2. Loopback Enforcement

ACP server listens only on localhost:

```rust
// In russell-acp-server/src/transport.rs
let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 18200);
let listener = TcpListener::bind(addr)?;
```

**Rationale:** Russell is attack surface for the host — never expose to network.

### 3. Rate Limiting

Per-token rate limit (100 calls/min default):

```rust
pub struct RateLimiter {
    calls: HashMap<String, Vec<Timestamp>>,  // token → call timestamps
    limit: u32,
    window: Duration,
}

impl RateLimiter {
    pub fn check(&mut self, token: &str) -> Result<()> {
        let now = Instant::now();
        let calls = self.calls.entry(token.to_string()).or_insert(Vec::new());
        calls.retain(|t| now - *t < self.window);
        
        if calls.len() >= self.limit {
            return Err(AcpError::RateLimitExceeded(self.limit));
        }
        calls.push(now);
        Ok(())
    }
}
```

### 4. Audit Trail

Every ACP call logged to Russell's journal:

```sql
INSERT INTO journal (event_type, acp_session_id, acp_correlation_id, skill_id, visibility, caller, result, timestamp)
VALUES ('acp_skill_call', 'sess_abc123', 'corr_xyz789', 'web-search', 'public', 'hkask-curator', 'success', '2026-05-22T17:30:00Z');
```

---

## Testing Strategy

### Unit Tests

```rust
#[test]
fn private_skill_rejected() {
    let dispatch = AcpDispatch::test_instance();
    let result = dispatch.dispatch_skill("okapi-watcher", &json!({}));
    assert!(matches!(result, Err(AcpError::SkillNotExposed(_))));
}

#[test]
fn public_skill_accepted() {
    let dispatch = AcpDispatch::test_instance();
    let result = dispatch.dispatch_skill("web-search", &json!({"query": "test"}));
    assert!(result.is_ok());
}

#[test]
fn macaroon_expired_token_rejected() {
    let auth = MacaroonAuth::test_instance();
    let token = CapabilityToken {
        expires_at: Timestamp::from_secs(100),  // Expired
        ..Default::default()
    };
    let result = auth.validate(&token);
    assert!(matches!(result, Err(AcpError::TokenExpired(_))));
}
```

### Integration Tests

1. **Session lifecycle:** create → message (multiple turns) → close
2. **Visibility boundary:** attempt private skill via ACP → rejected
3. **Macaroon auth:** expired token → rejected; valid token → accepted
4. **Rate limiting:** 101 calls in 1 minute → 429 error
5. **A2A delegation:** Russell → hKask agent → result returned

---

## Implementation Checklist

| Task | Crate | Status |
|------|-------|--------|
| Define `Session`, `Turn`, `ToolCallRecord` types | `russell-acp-server/src/session.rs` | ⏳ |
| Define `SkillInfo`, `ProbeInfo`, `InterventionInfo` types | `russell-acp-server/src/types.rs` | ⏳ |
| Define `AcpError` taxonomy | `russell-acp-server/src/error.rs` | ⏳ |
| Implement JSON-RPC transport (stdio) | `russell-acp-server/src/transport.rs` | ⏳ |
| Implement session manager | `russell-acp-server/src/session.rs` | ⏳ |
| Implement Jack persona projection | `russell-acp-server/src/persona.rs` | ⏳ |
| Implement visibility filter | `russell-acp-server/src/dispatch.rs` | ⏳ |
| Implement macaroon auth | `russell-acp-server/src/auth.rs` | ⏳ |
| Implement rate limiter | `russell-acp-server/src/rate_limit.rs` | ⏳ |
| Implement A2A gateway (hKask delegation) | `russell-acp-server/src/a2a.rs` | ⏳ |
| Write unit tests | `russell-acp-server/src/lib.rs` (tests module) | ⏳ |
| Write integration tests | `russell-acp-server/tests/acp-integration.rs` | ⏳ |

---

## Next Steps

1. **Phase 1.1:** Create `russell-acp-server` crate structure
2. **Phase 1.2:** Implement session manager + turn records
3. **Phase 1.3:** Implement visibility filter + macaroon auth
4. **Phase 1.4:** Implement Jack persona projection
5. **Phase 1.5:** Implement JSON-RPC transport (stdio)

---

**Design Complete.** Ready for Phase 1 (implementation).