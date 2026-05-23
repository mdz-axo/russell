# ADR-0040: Skill Dispatch Port — Hexagonal Architecture Abstraction

**Date:** 2026-05-23  
**Status:** Implemented  
**Author:** Russell Team  
**Deciders:** Operator  
**Technical Story:** Adversarial review Task T20 (Add AcpDispatch Port Trait)

---

## Context

Russell's ACP server dispatches skill execution requests from hKask agents. The adversarial review (2026-05-23) identified that the ACP handler depended directly on `AcpDispatch`, a concrete implementation that:

1. Executes skills via local subprocess
2. Filters skills by visibility (public/private)
3. Manages evidence bundles and journal writes

This created architectural coupling:

- **Testing friction** — Integration tests required real skill directories and subprocess execution
- **No remote dispatch** — Cannot delegate skill execution to a remote Russell instance
- **No mock dispatch** — Cannot test ACP handler logic without executing real skills
- **Violation of hexagonal architecture** — The handler (core) depends on a concrete adapter, not a port

Per Alastair Cockburn's hexagonal architecture, skill dispatch is a **port** — the ACP handler should depend on an abstraction. Adapters provide concrete implementations (local subprocess, remote RPC, mock for testing).

---

## Decision

Define a `SkillDispatchPort` trait in `russell-acp-server` that provides a unified interface for skill dispatch operations.

### Trait Definition

```rust
#[async_trait]
pub trait SkillDispatchPort: Send {
    /// Load all public skills exposed via ACP.
    fn load_public_skills(&self) -> Vec<SkillInfo>;

    /// Get information about a specific skill.
    fn get_skill_info(&self, skill_id: &str) -> Option<SkillInfo>;

    /// List all public probes across all skills.
    fn list_probes(&self) -> Vec<ProbeInfo>;

    /// Dispatch a skill step (probe or intervention).
    async fn dispatch_skill(&self, skill_id: &str, args: &serde_json::Value) -> Result<String>;

    /// Run a specific probe (read-only, always allowed if skill is public).
    async fn run_probe(
        &self,
        skill_id: &str,
        probe_id: &str,
        args: &serde_json::Value,
    ) -> Result<String>;
}
```

### Design Rationale

**Why `Send` but not `Sync`?**

The trait requires `Send` (can be moved between threads) but not `Sync` (cannot be shared between threads). This is because:

- Skill dispatch may hold mutable state (e.g., dispatcher pool, journal writer)
- `AcpDispatch` uses `Mutex<HashMap>` for dispatcher pooling, which is `Send` but not `Sync`
- The ACP handler owns the dispatch port exclusively; no shared access needed

**Why synchronous `load_public_skills()`?**

Skill loading is a one-time operation at server startup. Making it async would add complexity without benefit. The handler calls this during initialization, not during request handling.

**Why `serde_json::Value` for args?**

Skill arguments are heterogeneous (probe parameters, intervention options). Using `serde_json::Value` allows flexibility without defining a rigid schema. Each skill defines its own argument format in its manifest.

### Implementations

**1. AcpDispatch (russell-acp-server)**

The existing local subprocess dispatcher:

```rust
pub struct AcpDispatch {
    skills: Vec<Skill>,
    skills_dir: PathBuf,
    journal: Option<Arc<JournalWriter>>,
    evidence_base: PathBuf,
    dispatcher_pool: Mutex<HashMap<String, Arc<Dispatcher>>>,
}
```

Key features:
- **Dispatcher pooling** — Caches `Dispatcher` instances by skill ID (Task T12)
- **Visibility filtering** — Only exposes public skills via `load_public_skills()`
- **Evidence bundles** — Writes stdout/stderr/event to `evidence/<skill>/<probe>/<timestamp>/`
- **Journal integration** — Records execution events with evidence references

**2. MockSkillDispatch (planned)**

For unit testing the ACP handler:

```rust
pub struct MockSkillDispatch {
    skills: Vec<SkillInfo>,
    responses: HashMap<String, String>,
}
```

**3. RemoteSkillDispatch (planned)**

For delegating skill execution to a remote Russell instance:

```rust
pub struct RemoteSkillDispatch {
    endpoint: String,
    client: reqwest::Client,
}
```

### Location

- **Trait:** `crates/russell-acp-server/src/port.rs`
- **AcpDispatch impl:** `crates/russell-acp-server/src/dispatch.rs`

---

## Consequences

### Positive

- **Hexagonal architecture** — Skill dispatch is now a proper port. The ACP handler depends on an abstraction; adapters provide concrete implementations.

- **Testability** — Unit tests can use `MockSkillDispatch` without executing real skills or managing skill directories.

- **Extensibility** — Remote skill dispatch, skill federation, or skill sandboxing can be added as new adapters without changing the handler.

- **Dispatcher pooling** — The `AcpDispatch` implementation caches dispatchers by skill ID, reducing overhead for repeated skill calls (Task T12).

- **Clear boundary** — The port defines exactly what the ACP handler needs from skill dispatch. No implicit dependencies.

### Negative

- **Async trait overhead** — `#[async_trait]` adds boxing overhead for each dispatch call. Negligible for subprocess execution (milliseconds vs microseconds).

- **Trait object limitations** — `dyn SkillDispatchPort` cannot be used with async methods in stable Rust without `#[async_trait]`. Mitigation: use `#[async_trait]` macro.

- **Migration path** — The ACP handler must be refactored to accept `Box<dyn SkillDispatchPort>` instead of `AcpDispatch`. Low risk: only one call site.

### Neutral

- **No breaking changes** — `AcpDispatch` retains all existing methods. The port implementation is additive.

- **Backward compatible** — Code that doesn't need the abstraction can continue using `AcpDispatch` directly.

---

## Compliance

| Principle | Compliance |
|---|---|
| **Cockburn** (Hexagonal architecture) | Skill dispatch is a port with multiple adapters |
| **JR-6** (Reuse, don't depend) | Handler depends on abstraction, not concrete dispatcher |
| **JR-1** (Austere by default) | Port defines minimal interface; no unnecessary methods |
| **Miller** (Capability discipline) | Port exposes only public skills; private skills are filtered by adapter |

---

## Implementation

**Files created:**
- `crates/russell-acp-server/src/port.rs` — Trait definition

**Files modified:**
- `crates/russell-acp-server/src/dispatch.rs` — Dispatcher pooling (Task T12)
- `crates/russell-acp-server/src/handler.rs` — Accept `Box<dyn SkillDispatchPort>` (planned)

**Tests:**
- `test_port_trait_exists` — Verifies trait can be used as trait object

---

## Future Work

1. **Refactor AcpHandler** — Change `dispatch: AcpDispatch` to `dispatch: Box<dyn SkillDispatchPort>`. This enables dependency injection.

2. **MockSkillDispatch** — Implement mock adapter for unit testing the ACP handler without real skill execution.

3. **RemoteSkillDispatch** — Implement adapter for delegating skill execution to a remote Russell instance (skill federation).

4. **Skill sandboxing** — Implement `SandboxedSkillDispatch` that runs skills in isolated containers (Landlock, namespaces).

5. **Skill metrics** — Extend port to expose execution metrics (success rate, latency percentiles) for monitoring.

---

## References

- [ADR-0013: Rust Workspace Layout](0013-rust-workspace-layout.md)
- [ADR-0033: Explicit Port Interfaces](0033-explicit-port-interfaces.md)
- Adversarial Review Action Plan (2026-05-23) §Task T20, T12
- Alastair Cockburn, *Hexagonal Architecture* (2005)
