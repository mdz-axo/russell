---
title: "ADR-0039: Inference Port — Unified LLM Backend Abstraction"
audience: [developers, architects]
last_updated: 2026-05-23
ddmvss_context: "jack"
ddmvss_artifact: "adr"
version: "1.0.0"
status: "Implemented"
---



---
title: "ADR-0039: Inference Port — Unified LLM Backend Abstraction"
audience: [developers, architects]
last_updated: 2026-05-23
togaf_phase: "D"
version: "1.0.0"
status: "Implemented"
---


# ADR-0039: Inference Port — Unified LLM Backend Abstraction

## Decision

Define an `InferencePort` trait in `russell-core` that provides a unified interface for LLM inference, with structured SOAP (Subjective-Objective-Assessment-Plan) context.

### Trait Definition

```rust
#[async_trait]
pub trait InferencePort: Send + Sync {
    /// Perform inference with a prompt and optional SOAP context.
    async fn infer(&self, prompt: &str, context: Option<&SoapBundle>) -> Result<InferenceResponse>;

    /// Check if the backend is available and healthy.
    async fn health_check(&self) -> Result<bool>;

    /// Get the backend identifier (e.g., "hkask", "okapi").
    fn backend_id(&self) -> &str;
}
```

### SOAP Bundle

The Nurse pipeline uses clinical-style SOAP formatting for structured context:

```rust
pub struct SoapBundle {
    pub subjective: String,              // Operator's description
    pub objective: Vec<SoapObservation>, // Telemetry data, metrics
    pub assessment: Option<String>,      // Preliminary analysis
    pub plan: Option<Vec<String>>,       // Proposed actions
}

pub struct SoapObservation {
    pub name: String,           // e.g., "cpu_usage"
    pub value: serde_json::Value,
    pub unit: Option<String>,   // e.g., "percent"
    pub severity: Option<String>, // e.g., "warn"
}
```

The `SoapBundle` provides a builder API:

```rust
let bundle = SoapBundle::new("High CPU usage")
    .with_observation("cpu_usage", json!(95.5))
    .with_full_observation("memory", json!(8192), Some("MB"), Some("warn"))
    .with_assessment("CPU saturation detected")
    .with_plan(vec!["Identify top processes".to_string()]);
```

### Response Format

```rust
pub struct InferenceResponse {
    pub text: String,                    // Generated response
    pub backend: String,                 // "hkask", "okapi", etc.
    pub model: Option<String>,           // Model identifier
    pub latency_ms: Option<u64>,         // Response time
    pub token_usage: Option<TokenUsage>, // Token statistics
}

pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}
```

### Implementations

**1. HkaskInferenceAdapter (russell-meta)**

Connects to hKask's REST API at `/api/llm/infer`:

```rust
pub struct HkaskInferenceAdapter {
    endpoint: String,
    capability_token: Option<String>,
    client: reqwest::Client,
}

impl HkaskInferenceAdapter {
    pub fn new(endpoint: impl Into<String>) -> Self;
    pub fn with_token(mut self, token: impl Into<String>) -> Self;
    pub fn with_token_from_file(mut self) -> Option<Self>;
}
```

**2. OkapiInferenceAdapter (planned)**

Local-first fallback using Okapi (Ollama-compatible endpoint). See "Future Work" section.

### Location

- **Trait:** `crates/russell-core/src/inference.rs`
- **HkaskInferenceAdapter:** `crates/russell-meta/src/hkask_adapter.rs`
- **OkapiInferenceAdapter:** Planned for `crates/russell-meta/src/okapi_adapter.rs`

---

## Consequences

### Positive

- **Hexagonal architecture** — Inference is now a proper port. The Nurse depends on an abstraction; adapters provide concrete implementations.

- **Resilience** — The Nurse can fall back to local Okapi when hKask is unavailable. Critical for single-host operation.

- **Testability** — Unit tests can use a mock `InferencePort` without network calls. Integration tests can verify adapter behavior.

- **Backend flexibility** — Adding Claude, GPT-4, or other backends requires only a new adapter. No changes to the Nurse pipeline.

- **Structured context** — The `SoapBundle` enforces clinical-style formatting, preventing ad-hoc prompt engineering.

- **Observability** — `InferenceResponse` includes latency and token usage for monitoring.

### Negative

- **Async trait overhead** — `#[async_trait]` adds boxing overhead for each inference call. Negligible for LLM latency (seconds vs microseconds).

- **SOAP formatting rigidity** — The `SoapBundle` structure may not fit all backends. Mitigation: adapters can ignore SOAP sections or reformat as needed.

- **Token usage parsing** — Different backends return token usage in different formats. Each adapter must normalize to `TokenUsage`.

### Neutral

- **No breaking changes** — Existing `call_hkask()` function remains for backward compatibility. The port is additive.

- **Backward compatible** — Code that doesn't need the abstraction can continue using direct HTTP calls.

---

## Compliance

| Principle | Compliance |
|---|---|
| **JR-6** (Reuse, don't depend) | Nurse depends on abstraction, not specific LLM backend |
| **Cockburn** (Hexagonal architecture) | Inference is a port with multiple adapters |
| **JR-4** (Small but present) | Nurse can function with local Okapi when hKask is unavailable |
| **Schneier** (Defense in depth) | Multiple backends provide redundancy against single points of failure |

---

## Implementation

**Files created:**
- `crates/russell-core/src/inference.rs` — Trait definition + `SoapBundle` + `InferenceResponse`
- `crates/russell-meta/src/hkask_adapter.rs` — `HkaskInferenceAdapter` implementation

**Files modified:**
- `crates/russell-acp-server/src/handler.rs` — Session messages use `InferencePort`
- `crates/russell-acp-server/src/main.rs` — Initialize `HkaskInferenceAdapter`

**Tests:**
- `test_soap_bundle_creation` — Verifies basic bundle construction
- `test_soap_bundle_builder` — Verifies builder API
- `test_soap_prompt_formatting` — Verifies prompt generation
- `test_adapter_creation` — Verifies adapter initialization
- `test_adapter_with_token` — Verifies token configuration

---

## Future Work

1. **OkapiInferenceAdapter** — Implement local-first fallback using Okapi (Ollama-compatible endpoint at `http://localhost:11434`). This is the next priority.

2. **Fallback chain** — Implement `FallbackInferenceAdapter` that tries hKask first, then Okapi, then offline mode.

3. **Model selection** — Extend `InferencePort` to support model selection (e.g., "use claude-3-opus for complex assessments").

4. **Streaming responses** — Add `infer_stream()` method for backends that support streaming (hKask, Claude).

5. **Prompt caching** — Cache SOAP bundle formatting to avoid redundant serialization.

---

## References

- [ADR-0016: MVP Doctor — Local-First Ollama with Opt-In OpenRouter](0016-doctor-and-llm-router.md)
- [ADR-0033: Explicit Port Interfaces](0033-explicit-port-interfaces.md)
- Adversarial Review Action Plan (2026-05-23) §Task T14
- Alastair Cockburn, *Hexagonal Architecture* (2005)
- SOAP note format (medical documentation standard)
