---
title: "Ecosystem Integration — hKask & Okapi Reference Model"
audience: [architects, developers]
last_updated: 2026-05-15
togaf_phase: "C"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Application Architecture -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-15 -->

# Ecosystem Integration — hKask & Okapi Reference Model

> Defines the interfaces that make Russell's skill registry a hKask ecosystem
> reference model: MCP tool exposure, Okapi LLM skill routing, cross-project
> skill portability, and OKH span bridging.
> Version: 1.0.0 | 2026-05-15

---

## 1. hKask MCP Tool Exposure

### New MCP Server: `arsenal-mcp-russell-skills`

Extends the existing `arsenal-mcp-russell` (7 tools reading Russell's journal)
with a new skill-inventory MCP server exposing skill metadata to hKask's
Curator (Duncan) and the `stack-control-plane`.

#### MCP Tool Definitions

```
arsenal-mcp-russell-skills (MCP server, localhost loopback)
├── skill_inventory          ← List all installed skills with health
├── skill_detail             ← Get one skill's manifest + health + lifecycle
├── skill_health             ← Get aggregated health for one or all skills
├── skill_lifecycle_history  ← Get lifecycle transition history from journal
├── skill_symptoms           ← List symptoms and coverage gaps
├── skill_export_bundle      ← Export a skill as .rsk.tar.gz (read-only to caller)
└── skill_evaluation_result  ← Get latest evaluation (scenario test results)
```

#### `skill_inventory` Tool Schema

```json
{
  "name": "skill_inventory",
  "description": "List all installed skills with health summaries.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "status_filter": {
        "type": "string",
        "enum": ["active", "stale_warning", "deprecated", "all"],
        "description": "Filter by lifecycle status. Default: active."
      }
    }
  }
}
```

**Output:**
```json
{
  "skills": [
    {
      "skill_id": "okapi-watcher",
      "version": "0.1.0",
      "kind": "Actionable",
      "status": "active",
      "symptoms": ["llm_slow", "resource_exhaustion", "gpu_fallback_to_cpu"],
      "health": {
        "quality_score": 0.85,
        "reliability": 0.92,
        "probe_runs": 340,
        "last_probe_run_at": "2026-05-15T08:55:00"
      }
    }
  ]
}
```

#### `skill_health` Tool Schema

```json
{
  "name": "skill_health",
  "description": "Get aggregated health assessment for skills.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "skill_id": {
        "type": "string",
        "description": "Optional. If omitted, returns health for all active skills."
      }
    }
  }
}
```

**Output:**
```json
{
  "skills": {
    "okapi-watcher": {
      "quality_score": 0.85,
      "reliability": 0.92,
      "latency_p95_ms": 350.0,
      "freshness_days": 5,
      "safety_posture": "pass",
      "staleness_days": 145,
      "probe_runs": 340,
      "intervention_runs": 12
    }
  }
}
```

### Duncan (Curator) Integration

Duncan — hKask's infrastructure Curator — uses `skill_inventory` and `skill_health`
to surface Russell skill state in the hKask dashboard:

```
Duncan's dashboard
  ├── Russell host group → skill health cards
  ├── Per-skill: quality_score gauge, reliability sparkline, staleness countdown
  └── Alert: skill staleness_days < 0 → notify operator
```

### `stack-control-plane` Integration

hKask's control plane queries `skill_lifecycle_history` to detect version drift:

```
stack-control-plane
  ├── Periodic: skill_inventory (all hosts)
  ├── Diff: skill version mismatch across hosts → flag for reconciliation
  └── Action: propose skill export from canonical host → import to drifters
```

---

## 2. Okapi LLM Skill Routing

### `skill-hint` Context Extension

Okapi's request context gains a `skill-hint` JSON field that maps active symptoms
→ loaded skill KNOWLEDGE.md injection. This enables downstream Okapi consumers
(not just Russell) to use the skill registry for prompt augmentation.

#### Okapi API Extension

```
POST /api/v1/chat/completions
{
  "model": "default",
  "messages": [...],
  "skill_hint": {
    "active_symptoms": ["llm_slow", "resource_exhaustion"],
    "skill_registry_path": "~/.local/share/harness/registry/local-cache.yaml",
    "skills_dir": "~/.local/share/harness/skills/",
    "budget_tokens": 3000
  }
}
```

The `skill_hint` block is **optional**. When present, Okapi:

1. Loads `RegistryCache` from `skill_registry_path`
2. Queries `lookup_symptom()` for each active symptom
3. Reads `KNOWLEDGE.md` from `skills_dir/<skill_id>/KNOWLEDGE.md`
4. Scores relevance using `score_knowledge_relevance()` 
5. Injects knowledge into the system prompt under the `knowledge` key
6. Emits `okh.llm.knowledge_injection` span with skill IDs and token estimates

#### Response header

```
X-Skills-Injected: okapi-watcher,ubuntu-doctor
X-Knowledge-Tokens: 1847
```

### OpenRouter / Remote Backend Support

When using OpenRouter instead of local Okapi, the `skill_hint` block is forwarded
as a custom header to Russell's prompt assembler. The `PromptAssembler` port
handles knowledge injection before the prompt leaves the machine — OpenRouter
never sees raw skill data.

---

## 3. Cross-Project Skill Portability

### Manifest Schema as Ecosystem Contract

The manifest.yaml schema is the hKask ecosystem's skill contract:

```yaml
id: <kebab-case-id>       # required — unique identifier
version: <semver>          # required — semantic version
authored: <ISO 8601>       # required — creation date
symptoms: [<id>, ...]      # required — poka-yoke enforced
kind: actionable | lens    # optional — default: actionable
applies_when:              # optional — profile preconditions
  os_family: linux
probes:                    # optional — diagnostic steps
  - id: <probe-id>
    cmd: [<argv>, ...]
    timeout: 30s
interventions:             # optional — mutating steps
  - id: <intervention-id>
    cmd: [<argv>, ...]
    risk: none | low | medium | high | critical
    rollback: <rollback-id> | none_needed | reboot
safety:                    # optional — risk constraints
  max_auto_risk: low
evaluation:                # optional — post-intervention checks
  - id: <check-id>
    cmd: [<argv>, ...]
```

**Compatibility guarantee:** Any tool that can parse this YAML schema can load a
skill written for Russell. This includes:

- **Russell** (`FilesystemSkillLoader`): native loader
- **hKask** (future `SkillLoader` adapter): same schema, different dispatch
- **Okapi** (prompt injection only): reads `symptoms` and `kind`, ignores probes/interventions
- **Third-party** (web-based skill browser): renders manifest metadata

### Loader Compatibility Matrix

| Tool | Loads manifest | Executes probes? | Executes interventions? | Injects knowledge? |
|---|---|---|---|---|
| Russell | Yes | Yes (IDRS-gated) | Yes (IDRS + consent) | Yes (scored + budgeted) |
| hKask (proposed) | Yes | Yes (hKask governance) | Yes (hKask governance) | Yes |
| Okapi | Yes | No | No | Yes (via skill_hint) |
| Web browser | Yes | No | No | No |

---

## 4. OKH Span Bridging

### Span Namespace

Russell's OKH spans are emitted via the same `tracing` layer hKask uses:

```
okh.<layer>.<module>.<signal>
```

Russell-specific spans:

```
okh.skill.load.all           ← SkillLoader::load_all()
okh.skill.load.one           ← SkillLoader::load_one()
okh.skill.validate           ← SkillValidator::validate()
okh.skill.eval.quality       ← quality score
okh.skill.eval.reliability   ← EWMA reliability
okh.skill.eval.latency       ← p95 latency
okh.skill.eval.freshness     ← days since install
okh.skill.eval.safety        ← safety posture
okh.skill.eval.staleness     ← days to threshold
okh.skill.eval.complete      ← composite assessment
okh.skill.dispatch.probe     ← probe execution
okh.skill.dispatch.intervention ← intervention execution
okh.skill.dispatch.rollback  ← rollback execution
okh.skill.lifecycle.transition ← state transition
```

### hKask Observability Integration

hKask's observability surface (Loki + Grafana) picks up Russell skill health natively
because both systems use the same `tracing` subscriber:

```
Russell (tracing spans) → OpenTelemetry collector → Loki → Grafana
hKask    (tracing spans) → OpenTelemetry collector → Loki → Grafana
                                         ↑
                                   shared OTEL collector
                                   (same Span ID format)
```

### Grafana Dashboard Queries

```
# Skill health across hosts
okh_skill_eval_complete_total{quality_score > 0.8}
  → "Healthy skills"

# Skill staleness alerts
okh_skill_eval_staleness{staleness_days < 0}
  → "Stale skills"

# Dispatch latency p95
histogram_quantile(0.95, okh_skill_dispatch_probe_duration_seconds)
  → "Probe latency p95 across fleet"

# Reliability degradation
okh_skill_eval_reliability{reliability < 0.7}
  → "Unreliable skills"
```

---

## 5. Integration Architecture Summary

```
┌─────────────────────────────────────────────────────────────────────┐
│                        KASK ECOSYSTEM                                │
│                                                                      │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────────────┐   │
│  │   Russell     │    │   hKask       │    │   Okapi              │  │
│  │   (skill host)│    │   (control   │    │   (LLM router)       │  │
│  │               │    │    plane)    │    │                      │  │
│  │  - Skill reg. │◄──►│  - MCP tools │    │  - skill_hint ctx    │  │
│  │  - Dispatch   │    │  - Curator   │◄──►│  - knowledge inj.    │  │
│  │  - Telemetry  │    │  - Dashboard │    │  - prompt assembly   │  │
│  └──────┬───────┘    └──────┬───────┘    └──────────┬───────────┘  │
│         │                   │                        │              │
│         │  .rsk.tar.gz      │  MCP tools/call        │  skill_hint  │
│         │  (skill bundles)  │  (loopback)            │  (API ext.)  │
│         │                   │                        │              │
│         ▼                   ▼                        ▼              │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                Shared Infrastructure                          │  │
│  │                                                                │  │
│  │  - Manifest YAML schema (ecosystem contract)                   │  │
│  │  - OKH tracing spans (opentelemetry → Loki → Grafana)          │  │
│  │  - .rsk.tar.gz bundle format (portable skill unit)             │  │
│  │  - REUSE_MANIFEST.md (provenance across operator boundaries)   │  │
│  └──────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
```
