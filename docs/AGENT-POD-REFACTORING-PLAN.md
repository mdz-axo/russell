# Russell Agent Pod Refactoring Plan

**Date:** 2026-05-22  
**Goal:** Maximize alignment with hKask Agent Pod specification while remaining an external ACP agent  
**Principle:** Russell is a first-class agent entity with its own crate, lifecycle, and artifacts

---

## Architectural Clarification

**Key Insight:** The Agent Pod structure is NOT about internal vs external deployment. It's about:

1. **Agent as Entity** — Russell is a sovereign agent with its own identity, charter, and lifecycle
2. **Proper Crate Structure** — `russell-agent` crate owns artifacts, memories, and pod lifecycle
3. **Lifecycle States** — Populated → Registered → Activated → Deactivated (not just systemd on/off)
4. **Persona-Driven** — Agent persona YAML defines charter, capabilities, rights, responsibilities
5. **CNS Integration** — Russell emits CNS spans, not just journal events
6. **Template Crate Skills** — Skills are template crates with Jinja2 prompts, not just bash scripts

**Deployment Model:** Russell remains an **external ACP agent** (deployed separately from hKask), but implements the full agent pod interface for proper integration.

---

## Refactoring Phases

### Phase 1: Agent Crate Structure (Priority: Critical)

**Goal:** Create `russell-agent` crate as the canonical agent entity.

**New Structure:**
```
crates/
  russell-agent/          # NEW — Agent pod implementation
    ├── src/
    │   ├── pod.rs        # AgentPod implementation
    │   ├── persona.rs    # Agent persona YAML parsing
    │   ├── lifecycle.rs  # State machine implementation
    │   ├── cns.rs        # CNS span emission
    │   └── lib.rs
    ├── agent_persona.yaml   # Russell's charter
    ├── dispatch_manifest.yaml
    ├── hlexicon.yaml
    └── Cargo.toml

  russell-acp-server/     # Refactored — ACP transport layer only
  russell-sentinel/       # Unchanged — Probe collection
  russell-skills/         # Refactored — Template crate loader
```

**Agent Persona YAML:**
```yaml
agent:
  name: "russell"
  type: "Bot"
  version: "0.20.0"
  webid: "did:web:russell.local:russell"
  
charter:
  description: "Cybernetic health harness for Linux AI/ML workstation"
  editor: "operator"
  
capabilities:
  - "tool:system:probe"
  - "tool:journal:query"
  - "tool:skill:dispatch"
  - "tool:llm:escalate"
  
rights:
  - read: "host_telemetry"
  - write: "own_journal"
  - read: "own_episodic_memory"
  
responsibilities:
  - "observe: 5-minute cadence"
  - "report: via ACP"
  - "escalate: via Jack persona"
  - "emit: cns.russell.*"
  
visibility:
  default: "private"
  journal: "operator-only"
```

**Effort:** 8-12 hours  
**LOC:** ~500 new LOC in `russell-agent/src/pod.rs`

---

### Phase 2: Lifecycle State Machine (Priority: Critical)

**Goal:** Implement proper pod lifecycle states.

**State Machine:**
```
— → Populated → Registered → Activated → Deactivated —→
      │              │            │             │
      │              │            │             └─→ Cleanup complete
      │              │            └─→ ACP serving, sentinel running
      │              └─→ ACP runtime registered, capabilities granted
      └─→ Crate loaded, persona validated
```

**Implementation:**
```rust
// crates/russell-agent/src/lifecycle.rs

pub enum PodLifecycleState {
    Populated,      // Crate loaded, persona validated
    Registered,     // ACP runtime registration complete
    Activated,      // Sentinel running, ACP serving
    Deactivated,    // Capabilities revoked, cleanup pending
}

pub struct RussellPod {
    id: PodID,
    persona: AgentPersona,
    state: PodLifecycleState,
    acp_server: Option<AcpServer>,
    sentinel: Option<SentinelHandle>,
    cns_emitter: CnsEmitter,
}

impl RussellPod {
    pub fn new(template_crate: TemplateCrate) -> Result<Self> {
        // Validate persona, populate pod
    }
    
    pub async fn register(&mut self, runtime: &AcpRuntime) -> Result<()> {
        // Register with hKask ACP runtime
        // Receive capability token
        // Emit cns.russell.registered span
    }
    
    pub async fn activate(&mut self) -> Result<()> {
        // Start sentinel timer
        // Start ACP server
        // Emit cns.russell.activated span
    }
    
    pub async fn deactivate(&mut self) -> Result<()> {
        // Stop sentinel
        // Stop ACP server
        // Revoke capabilities
        // Emit cns.russell.deactivated span
    }
    
    pub fn delegate(&self, attenuation: Attenuation) -> Result<CapabilityToken> {
        // Create attenuated child token
        // Max 7 levels
    }
}
```

**CNS Spans Emitted:**
| Event | Span |
|-------|------|
| Pod populated | `cns.russell.populated` |
| Pod registered | `cns.russell.registered` |
| Pod activated | `cns.russell.activated` |
| Pod deactivated | `cns.russell.deactivated` |
| Probe executed | `cns.russell.probe.executed` |
| Skill dispatched | `cns.russell.skill.dispatch` |
| LLM escalation | `cns.russell.llm.escalation` |

**Effort:** 6-8 hours  
**LOC:** ~300 new LOC in `russell-agent/src/lifecycle.rs`

---

### Phase 3: Template Crate Skills (Priority: High)

**Goal:** Convert skills from bash scripts to template crates.

**Current Structure:**
```
~/.local/share/harness/skills/journal-viewer/
  ├── manifest.yaml
  └── scripts/
      └── show-host-samples.sh
```

**New Structure:**
```
~/.local/share/harness/skills/journal-viewer/
  ├── Cargo.toml              # Rust package metadata
  ├── agent_persona.yaml      # Skill agent identity
  ├── dispatch_manifest.yaml  # Dispatch routing
  ├── templates/
  │   ├── selector.j2         # Template selection logic
  │   └── show_host_samples.j2  # Jinja2 prompt template
  ├── hlexicon.yaml           # hLexicon terms
  └── scripts/                # Optional: bash helpers
      └── show-host-samples.sh
```

**Template Example:**
```jinja2
{# templates/show_host_samples.j2 #}
{% extends "base.j2" %}

{% block cognition %}
You are Russell, the cybernetic health harness.
The operator is requesting host samples from the journal.

Current state:
- Journal has {{ journal.event_count }} events
- Last sample: {{ journal.last_sample.timestamp }}
- Threshold breaches: {{ journal.last_24h.breaches }}

Select the appropriate response based on severity.
{% endblock %}

{% block prompt %}
Show me the last {{ limit | default(20) }} host samples.
{% endblock %}
```

**Dispatch Manifest:**
```yaml
id: journal-viewer
version: 0.1.0

dispatch:
  - name: show-host-samples
    template: templates/show_host_samples.j2
    input:
      - name: limit
        type: integer
        default: 20
    output:
      type: markdown
    risk: none
    
hlexicon:
  primary: flowdef
  terms: ["sequence", "filter", "route"]
```

**Effort:** 12-16 hours (for all 14 skills)  
**LOC:** ~200 LOC template conversions

---

### Phase 4: CNS Integration (Priority: High)

**Goal:** Russell emits CNS spans, not just journal events.

**Current:**
```rust
// russell-core/src/journal.rs
journal.write_event(Event {
    id: uuid(),
    timestamp: now(),
    severity: Warn,
    subject: "CPU high",
    objective: "CPU at 85%",
    // ...
});
```

**New:**
```rust
// russell-agent/src/cns.rs
cns_emitter.emit(Span {
    name: "cns.russell.probe.executed",
    phase: Phase::Observe,
    nu_event: NuEvent {
        severity: Severity::Info,
        subject: "probe.executed",
        objective: json!({
            "probe_id": "cpu-usage",
            "value": 85,
            "threshold": 80,
            "breach": true
        }),
    },
    agent_id: "russell",
    pod_id: pod_id.clone(),
    trace_id: trace_id,
});

// ALSO write to local journal (for independence)
journal.write_event(...);
```

**Dual-Write Strategy:**
| Destination | Purpose | When |
|-------------|---------|------|
| **Local Journal** | Russell independence, offline operation | Always |
| **CNS Spans** | hKask integration, ensemble visibility | When hKask reachable |

**Effort:** 4-6 hours  
**LOC:** ~150 new LOC in `russell-agent/src/cns.rs`

---

### Phase 5: Memory Artifact Storage (Priority: Medium)

**Goal:** Russell owns its artifacts and memories.

**Structure:**
```
~/.local/state/russell/
  ├── journal.db              # SQLite journal (existing)
  ├── artifacts/              # NEW — Memory artifacts
  │   ├── semantic/           # Semantic memory triples
  │   │   └── YYYY-MM-DD.triples
  │   ├── episodic/           # Episodic memory (private)
  │   │   └── YYYY-MM-DD.episodes
  │   └── evidence/           # Evidence bundles
  │       └── YYYY-MM-DD/
  │           └── bundle.json
  └── cache/                  # Template crate cache
      └── skills/
```

**Artifact Types:**
| Type | Location | Visibility |
|------|----------|------------|
| Semantic triples | `artifacts/semantic/` | Public (hKask ensemble) |
| Episodic episodes | `artifacts/episodic/` | Private (Russell-only) |
| Evidence bundles | `artifacts/evidence/` | Operator-only |
| Skill artifacts | `artifacts/skills/{id}/` | Per-skill visibility |

**Effort:** 6-8 hours  
**LOC:** ~200 new LOC for artifact storage

---

### Phase 6: ACP Server Refactoring (Priority: High)

**Goal:** ACP server becomes transport layer for agent pod.

**Current:**
```
russell-acp-server/
  ├── src/
  │   ├── handler.rs    # ACP request handling
  │   ├── dispatch.rs   # Skill dispatch
  │   ├── persona.rs    # Jack persona
  │   └── main.rs       # Entry point
```

**New:**
```
russell-acp-server/
  ├── src/
  │   ├── transport.rs  # ACP transport (stdio/TCP)
  │   ├── handler.rs    # ACP request routing
  │   └── main.rs       # Entry point (thin wrapper)
  
russell-agent/
  ├── src/
  │   ├── pod.rs        # AgentPod (moved from above)
  │   ├── acp.rs        # ACP integration with pod
  │   └── ...
```

**Key Change:** `russell-acp-server` becomes a thin transport wrapper around `russell-agent` pod.

**Effort:** 4-6 hours  
**LOC:** Refactor ~300 LOC

---

### Phase 7: CLI Refactoring (Priority: Medium)

**Goal:** CLI becomes pod management interface.

**New Commands:**
```bash
# Pod lifecycle
russell pod status      # Show pod state
russell pod register    # Register with hKask ACP
russell pod activate    # Activate pod
russell pod deactivate  # Deactivate pod

# Persona
russell persona show    # Show agent persona
russell persona update  # Update persona (hot reload)

# Artifacts
russell artifacts list  # List memory artifacts
russell artifacts export # Export artifacts

# Existing commands (unchanged)
russell status
russell list
russell jack
russell chat
russell skill list
russell sentinel-once
```

**Effort:** 3-4 hours  
**LOC:** ~150 new LOC in `russell-cli/src/commands/pod.rs`

---

## Implementation Order

| Phase | Dependencies | Effort | Priority |
|-------|--------------|--------|----------|
| 1. Agent Crate | None | 8-12h | Critical |
| 2. Lifecycle | Phase 1 | 6-8h | Critical |
| 3. Template Skills | Phase 1 | 12-16h | High |
| 4. CNS Integration | Phase 2 | 4-6h | High |
| 5. Memory Artifacts | Phase 2 | 6-8h | Medium |
| 6. ACP Refactoring | Phase 1, 2 | 4-6h | High |
| 7. CLI Refactoring | Phase 2 | 3-4h | Medium |
| **Total** | | **43-60h** | |

---

## Testing Strategy

### Unit Tests (New)
```rust
// crates/russell-agent/src/pod.rs
#[test]
fn test_pod_lifecycle() {
    let mut pod = RussellPod::new(template_crate)?;
    assert_eq!(pod.state(), PodLifecycleState::Populated);
    
    pod.register(&runtime).await?;
    assert_eq!(pod.state(), PodLifecycleState::Registered);
    
    pod.activate().await?;
    assert_eq!(pod.state(), PodLifecycleState::Activated);
    
    pod.deactivate().await?;
    assert_eq!(pod.state(), PodLifecycleState::Deactivated);
}

#[test]
fn test_capability_attenuation() {
    let pod = RussellPod::new(template_crate)?;
    let token = pod.capability_token();
    
    let child = token.attenuate(Attenuation::new("skill:journal-viewer"))?;
    assert_eq!(child.attenuation_level(), 1);
    
    let grandchild = child.attenuate(Attenuation::new("probe:show-host-samples"))?;
    assert_eq!(grandchild.attenuation_level(), 2);
    
    // Max 7 levels
}
```

### Integration Tests
```bash
# Full lifecycle test
./tests/test-pod-lifecycle.sh

# CNS span emission test
./tests/test-cns-integration.sh

# Template skill execution test
./tests/test-template-skills.sh
```

---

## Migration Path

### For Existing Russell Installation

```bash
# 1. Backup existing data
cp -r ~/.local/state/harness ~/.local/state/harness.backup

# 2. Install new russell-agent crate
cargo install --path crates/russell-agent

# 3. Migrate skills to template format
russell skill migrate-all

# 4. Register with hKask ACP
russell pod register

# 5. Activate pod
russell pod activate

# 6. Verify
russell pod status
```

### Backward Compatibility

| Feature | Compatibility | Notes |
|---------|---------------|-------|
| Existing skills | ✅ Supported | Auto-migrated on first run |
| Journal database | ✅ Compatible | Schema unchanged |
| ACP protocol | ✅ Compatible | Extended with pod endpoints |
| CLI commands | ✅ Compatible | New commands added |
| Systemd services | ✅ Compatible | Unit files updated |

---

## Success Criteria

| Criterion | Metric | Target |
|-----------|--------|--------|
| **Lifecycle states** | Pod implements all 4 states | ✅ 100% |
| **Persona YAML** | Russell has valid persona | ✅ Validated |
| **Template skills** | All 14 skills converted | ✅ 14/14 |
| **CNS integration** | Spans emitted for key events | ✅ 100% |
| **Memory artifacts** | Artifacts stored properly | ✅ Verified |
| **Tests passing** | Unit + integration tests | ✅ 30+ tests |
| **Documentation** | All docs updated | ✅ Complete |
| **Backward compat** | Existing installs migrate | ✅ Smooth |

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| **Breaking changes** | Backward compatibility layer |
| **Data loss** | Backup before migration |
| **Downtime** | Zero-downtime migration (dual-write) |
| **Complexity** | Phased rollout, test each phase |
| **CNS unreachable** | Graceful degradation (local journal only) |

---

## Timeline

| Week | Phases | Deliverable |
|------|--------|-------------|
| Week 1 | Phase 1, 2 | Agent crate + lifecycle |
| Week 2 | Phase 3, 4 | Template skills + CNS |
| Week 3 | Phase 5, 6, 7 | Artifacts + ACP + CLI |
| Week 4 | Testing, docs | Full integration tested |

---

## Verification

```bash
# After all phases complete:

# 1. Pod lifecycle
russell pod status
# Expected: Shows state (Populated/Registered/Activated/Deactivated)

# 2. Persona
russell persona show
# Expected: Shows agent persona YAML

# 3. CNS spans
journalctl --user -u russell | grep cns.russell
# Expected: CNS spans visible

# 4. Template skills
russell skill list --format template
# Expected: Shows template crate structure

# 5. Integration tests
./tests/test-full-pod-integration.sh
# Expected: All tests pass
```

---

**Plan Date:** 2026-05-22  
**Status:** Ready for implementation  
**Next Action:** Begin Phase 1 (Agent Crate Structure)

---

**References:**
- [hKask Agent Pod Implementation](../hKask/docs/architecture/AGENT_POD_IMPLEMENTATION.md)
- [Russell Agent Pod Alignment](RUSSELL-AGENT-POD-ALIGNMENT.md)
- [ACP Integration Complete](ACP-INTEGRATION-COMPLETE.md)
