---
title: "Cross-Repository Capability → Port → Adapter → Contract Graph"
audience: [architects, developers, agents]
last_updated: 2026-05-13
togaf_phase: "Preliminary"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Cross-cutting -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-13 -->

<!-- DIAGRAM_ALIGNMENT
id: DIAG-CAPABILITY-001
type: ER diagram
verified_date: 2026-05-13
verified_against: russell/crates/*, kask/stack/crates/*, kask/arsenal/crates/*, Okapi/server/*
reference_sources: AGENTS.md (all three repos), PRINCIPLES_CATALOG.md, ADR-T18
status: VERIFIED
-->

# Cross-Repository Capability → Port → Adapter → Contract Graph

## 1. Russell Capability Graph

### 1.1 Sentinel (Phase B — Business)

```
Capability: Host Observation
  Port: ProbeDescriptor (russell-sentinel/src/probes/mod.rs)
    Adapter: MemoryProbeAdapter (probes/memory.rs)
      Contract: ProbeResult { name, value, unit, risk_band }
    Adapter: GpuProbeAdapter (probes/gpu.rs)
      Contract: ProbeResult { name, value, unit, risk_band }
      Notes: Reads /sys/class/drm/card1/device/ — discrete GPU only
    Adapter: DiskProbeAdapter (probes/disks.rs)
      Contract: ProbeResult { name, value, unit, risk_band }
      Notes: Reads /proc/pressure/io — I/O pressure some/full avg10
    Adapter: ProcessProbeAdapter (probes/process.rs)
      Contract: ProbeResult { name, value, unit, risk_band }
      Notes: 7 probes from /proc, /proc/[pid]/stat
    Adapter: SystemdProbeAdapter (probes/systemd.rs)
      Contract: ProbeResult { name, value, unit, risk_band }
      Notes: systemctl subprocess — degraded, failed user/system units
    Adapter: NetworkProbeAdapter (probes/network.rs)
      Contract: ProbeResult { name, value, unit, risk_band }
    Adapter: ToolsProbeAdapter (probes/tools.rs)
      Contract: ProbeResult { name, value, unit, risk_band }
  Port: RuleEngine (russell-core/src/rule/mod.rs)
    Contract: RuleSet — per-probe TOML rules with operator-overridable thresholds
  Port: JournalWriter (russell-core/src/journal/mod.rs)
    Contract: harness.event.v1 — structured log record
```

### 1.2 Doctor (Phase C — Application)

```
Capability: LLM Consultation (Nurse)
  Port: LlmClient (russell-meta/src/client.rs)
    Adapter: OkapiClientAdapter (russell-meta/src/oai_client.rs)
      Contract: OpenAI-compatible API → /api/chat/completions
      Target: Okapi server at 127.0.0.1:11435
    Adapter: OpenRouterClientAdapter (russell-meta/src/oai_client.rs)
      Contract: OpenAI-compatible API → /v1/chat/completions
      Notes: Opt-in, ZDR enabled
    Adapter: MockClientAdapter (russell-meta/src/mock.rs)
      Contract: Deterministic responses for testing
  Port: ActionParser (russell-meta/src/action.rs)
    Contract: ACTION: <skill>/<probe-or-intervention> — parse and dispatch
  Port: HelpHandler (russell-meta/src/help.rs)
    Contract: SOAP bundle assembly — Subjective, Objective, Assessment, Plan
  Persona: jack.md (crates/russell-meta/prompts/jack.md)
    Contract: Terrier + Jack McFarland + Rust/Linux/cybernetics fluency
    JR-3: Never emits shell commands
```

### 1.3 Proprioception (Phase G — Governance)

```
Capability: Self-Observation
  Port: SelfVitals (russell-proprio/src/lib.rs)
    Contract: 5 self-vitals:
      - sentinel_last_run_age_s
      - journal_writer_stall_s
      - llm_p95_latency_ms
      - timer_drift_s
      - help_error_rate_pct
  Port: AutoimmuneGuard (russell-proprio/src/lib.rs)
    Contract: Recursion guard on self-triage — foundation built, wiring deferred
```

### 1.4 Skills (Phase C — Application)

```
Capability: Playbook Execution
  Port: SkillManifest (russell-skills/src/lib.rs)
    Contract: YAML manifest — id, risk, description, probe/intervention
  Port: Dispatcher (russell-skills/src/dispatch.rs)
    Contract: Poka-yoke — rejects unknown IDs, enforces max_auto_risk cap
    JR-3: LLM selects from known IDs only
```

### 1.5 CLI (Phase D — Technology)

```
Capability: Operator Interface
  Port: ClapCommands (russell-cli/src/main.rs)
    Verbs: jack, chat, sentinel-once, skill list, skill run, digest
  Port: ChatREPL (russell-cli/src/repl.rs)
    Contract: Interactive readline, token budgeting, consent gate
    Consent: /approve, /deny, natural language ("ok", "yes", "do it")
```

### 1.6 MCP (Phase C — Application)

```
Capability: Agent Tool Exposition
  Port: McpServer (russell-mcp/src/lib.rs)
    Contract: MCP protocol — tools/list, tools/call, describe()
    Tools: russell_jack, russell_sentinel, russell_proprio
```

---

## 2. Kask Capability Graph

### 2.1 Core Platform (stack/)

```
Capability: Container Management
  Port: KaskContainer (stack/crates/kask-container/)
    Contract: Typed, composable container — agents, tools, models, connectors, data
  Port: BitemporalStore (stack/crates/kask-memory/)
    Contract: Valid-time + transaction-time semantics
  Port: NeuralGraph (stack/crates/kask-neural-graph/)
    Contract: Knowledge graph with embeddings
  Port: McpInfrastructure (stack/crates/kask-mcp/)
    Contract: MCP server lifecycle, tool registration, capability exposure

Capability: LLM Routing
  Port: PromptRegistry (stack/crates/stack-prompts/)
    Contract: Jinja2 templates + LmContract validation
    OKH spans: okh.prompt.rendered, okh.prompt.contract_validated, okh.prompt.contract_violation
  Port: OutcomeAggregator (stack/crates/stack-prompts/)
    Contract: Pass rate tracking, regression detection
```

### 2.2 Arsenal (arsenal/)

```
Capability: MCP Tool Servers
  Adapter: ArsenalMcpRussell (arsenal/crates/arsenal-mcp-russell/)
    Contract: Reads Russell journal → exposes as MCP tools
  Adapter: ScholarMcp (arsenal/crates/arsenal-mcp-scholar/)
    Contract: Academic search integration
  Adapter: RssReaderMcp (arsenal/crates/arsenal-mcp-rss/)
    Contract: RSS feed ingestion
  Adapter: SettingsStore (arsenal/crates/arsenal-mcp-settings/)
    Contract: Configuration persistence
```

### 2.3 Observability (Cross-cutting)

```
Capability: OKH — Open Kask Health
  Namespace: okh.*
  Prefixes:
    - okh.connector.* — external I/O
    - okh.pipeline.* — multi-stage flows
    - okh.tool.* — tool governance
    - okh.llm.* — model routing
    - okh.error.class — error classification
  Contract: Structured tracing spans → Prometheus/Grafana
```

---

## 3. Okapi Capability Graph

### 3.1 Inference Engine

```
Capability: Model Loading
  Port: Engine (Okapi/server/engine.go)
    Contract: Single-model Engine (OKAPI_SIMPLE_ENGINE=1) or Scheduler
  Port: Runner (Okapi/runner/ollamarunner/)
    Contract: Pure Go inference — primary target for Okapi features
  Port: LlamaRunner (Okapi/runner/llamarunner/)
    Contract: Legacy C++ fallback — do not extend

Capability: LoRA Hot-Swap
  Port: LoraAdapter (Okapi/model/lora/)
    Contract: Load/unload/scale at runtime
  Port: LoraRoutes (Okapi/server/routes_lora.go)
    Contract: POST /api/adapters/load, /api/adapters/unload, /api/adapters/scale
  Port: LoraRunnerHandlers (Okapi/runner/ollamarunner/runner_lora.go)
    Contract: Apply adapter scale to inference

Capability: MoE Observability
  Port: MoEMetrics (Okapi/server/moe_metrics.go)
    Contract: GGUF metadata extraction + Prometheus metrics
    Params: num_moe_offload, ExpertCount, ExpertUsedCount, IsMoE
```

### 3.2 API Surface

```
Capability: Extended API
  Endpoint: GET /api/adapters — list loaded LoRA adapters
  Endpoint: POST /api/adapters/load — load adapter
  Endpoint: POST /api/adapters/unload — unload adapter
  Endpoint: POST /api/adapters/scale — adjust scale
  Endpoint: GET /api/engine/status — engine + capabilities + context utilization
  Endpoint: POST /api/rerank — document ranking (embedding-based or cross-encoder)
  Endpoint: GET /metrics — Prometheus metrics (KV-cache hit ratio)
  Endpoint: GET /api/metrics — JSON metrics (context utilization)
  Endpoint: POST /api/kask/* — Kask lifecycle endpoints (routes_kask.go)
```

### 3.3 Import Pipeline

```
Capability: HuggingFace GGUF Import
  Port: Importer (Okapi/importer/)
    Contract: HF Hub → local storage
  Port: Converter (Okapi/convert/)
    Contract: SafeTensors → GGUF (adapter converters for Qwen3.5, Mistral 3, Nemotron-H, Gemma 4)
```

### 3.4 Storage

```
Capability: Model Persistence
  Port: FlatStorage (Okapi/storage/)
    Contract: Okapi-native flat file format
  Port: GGML (Okapi/fs/ggml/)
    Contract: GGUF parsing + multi-file shard discovery
```

---

## 4. Cross-Repo Integration Points

```
Russell Doctor ──(OpenAI API)──▶ Okapi Server (127.0.0.1:11435)
  Contract: /api/chat/completions with LoRA adapter context

Kask Arsenal ──(MCP)──▶ Russell Journal (SQLite)
  Contract: Read-only access to samples, events tables

Kask Stack ──(Okapi API)──▶ Okapi Server
  Contract: Model loading, LoRA hot-swap, MoE offload, reranking

Russell MCP ──(MCP protocol)──▶ Kask Stack
  Contract: Tool discovery, capability negotiation

Russell Nurse ──(SOAP bundle)──▶ Okapi LLM
  Contract: Persona-injected prompt → structured assessment
```

---

## 5. Documentation → Code Mapping

| Document | Code Evidence |
|----------|---------------|
| `AGENTS.md` §5 vocabulary | Binding — no code reference needed |
| `PRINCIPLES_CATALOG.md` JR-1 | Cargo.toml (binary size), boot time benchmarks |
| `PRINCIPLES_CATALOG.md` JR-2 | `run_and_journal()`, `run_intervention_with_rollback()` |
| `PRINCIPLES_CATALOG.md` JR-3 | `ActionParser`, dispatcher poka-yoke |
| `PRINCIPLES_CATALOG.md` JR-4 | `russell-meta/prompts/jack.md` |
| `PRINCIPLES_CATALOG.md` JR-5 | `russell-proprio/src/lib.rs` — 5 self-vitals |
| `PRINCIPLES_CATALOG.md` JR-6 | `docs/operations/REUSE_MANIFEST.md` |
| `PRINCIPLES_CATALOG.md` JR-7 | `docs/specifications/PERSISTENCE_CATALOG.md` |
| `MVP_SPEC.md` | `crates/*/src/` — Phase 1 code only |
| `PERSISTENCE_CATALOG.md` | `russell-core/src/journal/` — SQLite schema |
| `TOGAF_LITE_FOR_OPEN_SOURCE.md` | `docs/standards/` — pattern document |
| `WRITING_EXCELLENCE.md` | `docs/standards/` — quality rubric |
| `DOCUMENTATION_STANDARDS.md` | `docs/standards/` — governance rules |
