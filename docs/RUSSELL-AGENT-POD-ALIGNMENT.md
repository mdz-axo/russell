# Russell Agent Pod Alignment Analysis

**Date:** 2026-05-22  
**Purpose:** Map Russell's ACP implementation to hKask's Agent Pod specification

---

## Executive Summary

Russell **aligns well** with hKask's Agent Pod architecture but operates as a **specialized external agent** rather than a full pod instance. Key findings:

| Aspect | Alignment | Notes |
|--------|-----------|-------|
| **ACP Protocol** | ✅ Compatible | Russell implements ACP server |
| **Capability Tokens** | ✅ Compatible | Both use macaroon OCAP |
| **Lifecycle States** | ⚠️ Partial | Russell uses systemd, not pod states |
| **Persona YAML** | ⚠️ Gap | Russell uses skill manifests |
| **Template Crate** | ❌ Not applicable | Russell uses skill modules |
| **CNS Integration** | ⚠️ Partial | Russell has own journal, not CNS spans |
| **Delegation** | ✅ Compatible | Both support attenuation |

**Recommendation:** Russell remains an **external ACP agent** (not converted to pod). Integration via ACP protocol is cleaner than forcing pod model.

---

## hKask Agent Pod Model

### Pod Lifecycle States

```
— → Populated → Registered → Activated → Deactivated —→
```

| State | Description |
|-------|-------------|
| **Populated** | Template crate loaded, persona validated |
| **Registered** | ACP runtime registration complete |
| **Activated** | Ready for A2A communication |
| **Deactivated** | Capabilities revoked, cleanup pending |

### Pod Structure

```yaml
agent:
  name: "memory-bot"
  type: "Bot"  # or "Replicant"
  version: "0.1.0"
  
charter:
  description: "Expert bot for memory operations"
  editor: "curator"
  
capabilities:
  - "tool:memory:remember"
  - "tool:memory:recall"
  
rights:
  - read: "public_semantic_memory"
  - write: "own_episodic_memory"
  
responsibilities:
  - "respond_to: memory_tool_calls"
  - "emit: cns.agent_pod.*"
```

---

## Russell Current Model

### Russell Lifecycle

```
— → Installed → Enabled (systemd) → Running —→
                     ↓
              5-min cadence (sentinel)
```

| State | Description |
|-------|-------------|
| **Installed** | Binaries + skills in place |
| **Enabled** | Systemd services active |
| **Running** | Sentinel probing, ACP serving |

### Russell Structure

```yaml
# Skill manifest (manifest.yaml)
id: journal-viewer
version: 0.1.0
symptoms: ["skill_not_in_catalog"]
visibility: public
lexicon:
  primary: flowdef
  terms: ["sequence", "filter", "route"]

probes:
  - id: show-host-samples
    cmd: ["bash", "./scripts/show-host-samples.sh"]
    timeout: 5s

safety:
  max_auto_risk: none
```

---

## Alignment Analysis

### 1. ACP Protocol ✅ Compatible

| hKask Pod | Russell | Alignment |
|-----------|---------|-----------|
| `acp/runtime.register_agent()` | `acp/session.create` | ✅ Compatible |
| `acp/runtime.send_message()` | `acp/session.message` | ✅ Compatible |
| `acp/runtime.list_agents()` | `acp/capabilities` | ✅ Compatible |
| Capability tokens | Macaroon auth | ✅ Compatible |

**Gap:** Russell ACP is stdio-only; hKask pods may need TCP/WebSocket.

### 2. Capability Model ✅ Compatible

| hKask Pod | Russell | Alignment |
|-----------|---------|-----------|
| `CapabilityToken` with HMAC | Macaroon with caveats | ✅ Compatible |
| Attenuation on delegation | Rate limit + skill attenuation | ✅ Compatible |
| Max 7 levels | Configurable | ✅ Compatible |
| Expiration-based revocation | `before` caveat | ✅ Compatible |

**Gap:** Russell doesn't implement explicit revocation lists.

### 3. Lifecycle States ⚠️ Partial

| hKask Pod | Russell | Alignment |
|-----------|---------|-----------|
| Populated | Installed | ⚠️ Different model |
| Registered | Enabled | ⚠️ Systemd vs ACP runtime |
| Activated | Running | ⚠️ Implicit vs explicit |
| Deactivated | Disabled | ⚠️ Systemd vs ACP runtime |

**Gap:** Russell uses systemd lifecycle, not pod states. Conversion would require:
- Adding pod state machine to `russell-acp-server`
- Implementing `register()` / `activate()` / `deactivate()` methods
- CNS span emission for lifecycle events

**Recommendation:** Keep systemd model — it's simpler and operationally proven.

### 4. Persona YAML ⚠️ Gap

| hKask Pod | Russell | Alignment |
|-----------|---------|-----------|
| `agent_persona.yaml` | Skill manifests | ⚠️ Different schema |
| Charter + rights + responsibilities | Symptoms + probes + interventions | ⚠️ Different focus |
| Bot vs Replicant type | Public vs Private visibility | ⚠️ Different classification |

**Gap:** Russell's skill manifests don't map cleanly to pod persona schema.

**Recommendation:** Create adapter layer if hKask needs Russell persona info:
```yaml
# ~/.local/share/harness/skills/russell-agent/agent_persona.yaml
agent:
  name: "russell"
  type: "Bot"
  version: "0.20.0"
  
charter:
  description: "Cybernetic health harness for Linux AI/ML workstation"
  editor: "operator"
  
capabilities:
  - "tool:system:probe"
  - "tool:journal:query"
  - "tool:skill:dispatch"
  
rights:
  - read: "host_telemetry"
  - write: "own_journal"
  
responsibilities:
  - "observe: 5-minute cadence"
  - "report: via ACP"
  - "escalate: via Jack persona"
```

### 5. Template Crate ❌ Not Applicable

| hKask Pod | Russell | Alignment |
|-----------|---------|-----------|
| Git-based template crate | Skill directory | ❌ Different model |
| Jinja2 templates | Bash/Python scripts | ❌ Different execution |
| Dispatch manifest | Skill manifest | ⚠️ Similar purpose |

**Gap:** Russell skills are not template crates.

**Recommendation:** No conversion needed. Russell skills work as-is via ACP.

### 6. CNS Integration ⚠️ Partial

| hKask Pod | Russell | Alignment |
|-----------|---------|-----------|
| `cns.agent_pod.*` spans | Journal events | ⚠️ Different system |
| NuEvent structure | `harness.event.v1` | ⚠️ Different schema |
| CNS variety counters | EWMA baselines | ⚠️ Different approach |

**Gap:** Russell has own journal, not integrated with CNS.

**Recommendation:** Add CNS span emission for key Russell events:
```rust
// In russell-acp-server/src/handler.rs
fn emit_cns_span(event: &str, details: &serde_json::Value) {
    // Emit to hKask CNS via MCP client
    // cns.russell.acp_request, cns.russell.probe_executed, etc.
}
```

### 7. Delegation ✅ Compatible

| hKask Pod | Russell | Alignment |
|-----------|---------|-----------|
| `pod.delegate()` | Macaroon attenuation | ✅ Compatible |
| Max 7 levels | Configurable rate limit | ✅ Compatible |
| Child token creation | Caveat addition | ✅ Compatible |

**Gap:** None — models align well.

---

## Integration Options

### Option A: External ACP Agent (Recommended)

**Description:** Russell remains external, integrates via ACP protocol.

**Pros:**
- ✅ No code changes to Russell
- ✅ Operational independence maintained
- ✅ Graceful degradation during hKask outages
- ✅ Proven deployment model (systemd)

**Cons:**
- ⚠️ Not a "first-class" hKask pod
- ⚠️ No CNS span integration (can be added)
- ⚠️ Different lifecycle model

**Implementation:**
```yaml
# hKask config/agents/russell.yaml
agent:
  russell:
    type: external_acp
    transport:
      protocol: stdio
      command: ["russell-acp-server"]
    auth:
      type: macaroon
    capabilities:
      - "russell:probe/*"
      - "russell:skill/*"
```

### Option B: Full Pod Conversion

**Description:** Convert Russell to hKask agent pod.

**Pros:**
- ✅ First-class hKask citizen
- ✅ CNS span integration
- ✅ Unified lifecycle model

**Cons:**
- ❌ Major refactoring required
- ❌ Loses operational independence
- ❌ Breaks graceful degradation
- ❌ ~2000 LOC changes estimated

**Implementation:**
```rust
// New crate: russell-pod
pub struct RussellPod {
    pod: AgentPod,
    sentinel: Sentinel,
    acp_server: AcpServer,
}

impl RussellPod {
    pub async fn register(&mut self, runtime: &AcpRuntime) -> Result<()> {
        // Register with hKask ACP runtime
    }
    
    pub async fn activate(&mut self) -> Result<()> {
        // Start sentinel, ACP server
    }
}
```

### Option C: Hybrid (Recommended for v1.1)

**Description:** Russell remains external but emits CNS spans and accepts pod-like commands.

**Pros:**
- ✅ Minimal code changes (~200 LOC)
- ✅ CNS integration added
- ✅ Operational independence maintained
- ✅ Can be managed like a pod

**Implementation:**
1. Add CNS span emission to `russell-acp-server`
2. Add pod-like status endpoint: `acp/pod/status`
3. Create adapter persona YAML for Russell

---

## Recommendation

**Use Option A (External ACP Agent) for MVP, Option C (Hybrid) for v1.1.**

### Rationale

1. **Russell is already functional** — ACP server works, tests pass
2. **Operational independence is valuable** — Russell works during hKask outages
3. **Pod model adds complexity** — Lifecycle states, template crates, CNS integration
4. **ACP protocol is sufficient** — Bidirectional communication works

### v1.1 Enhancements (Option C)

| Enhancement | Effort | Benefit |
|-------------|--------|---------|
| CNS span emission | 2 hours | hKask observability |
| Pod status endpoint | 1 hour | Pod-like management |
| Adapter persona YAML | 30 min | hKask registry integration |
| **Total** | **~3.5 hours** | **High** |

---

## Conclusion

Russell **does not need to become an agent pod** to integrate with hKask. The ACP protocol provides clean integration while maintaining Russell's operational independence.

**Recommended path:**
1. ✅ Keep Russell as external ACP agent
2. ✅ Document integration points clearly
3. ⏳ Add CNS spans in v1.1 (optional)
4. ⏳ Create adapter persona YAML (optional)

---

**Analysis Date:** 2026-05-22  
**Analyst:** Russell Team  
**Status:** ✅ Complete — Recommendation: External ACP Agent

---

**References:**
- [hKask Agent Pod Implementation](../hKask/docs/architecture/AGENT_POD_IMPLEMENTATION.md)
- [Russell ACP Agent Architecture](../hKask/docs/archive/2026-05-22-documentation-refresh/russell-acp-agent.md)
- [ACP Integration Complete](ACP-INTEGRATION-COMPLETE.md)
