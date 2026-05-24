---
title: "Subsystem Entity Relationship Diagrams"
audience: [architects, developers, agents]
last_updated: 2026-05-24
togaf_phase: "D"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Technology — Data Architecture -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-24 -->

# Subsystem Entity Relationship Diagrams

Nine Mermaid ER diagrams mapping Russell's domain model to its Rust
type system and SQLite schema. Each diagram satisfies the
`DIAGRAM_ALIGNMENT` contract defined in
[`../standards/DOCUMENTATION_STANDARDS.md`](../standards/DOCUMENTATION_STANDARDS.md) §4.

---

## 1. Core Domain Model

```mermaid
erDiagram
    Event {
        string id PK "ULID"
        string ts "RFC 3339"
        i64 ts_unix
        string schema
        string severity "info|warn|alert|crit"
        string scope "host|self"
        string action
        bool dry_run
        string evidence_ref
        u64 duration_ms
        string summary
    }
    Severity {
        enum values "Info|Warn|Alert|Crit"
    }
    Profile {
        string schema
        string profile_id PK
        string authored_at
        u64 memory_mib
        u64 swap_mib
    }
    HostInfo {
        string os_family
        string os_distro
        string os_version
        string cpu_vendor
        string cpu_model
        u32 cpu_cores
        u32 cpu_threads
    }
    GpuInfo {
        string pci
        string vendor_id
        string name
        string role
    }
    RuleSet {
        int rule_count
    }
    Rule {
        string probe PK
        string description
        f64 warn_below
        f64 alert_below
        f64 crit_below
        f64 warn_above
        f64 alert_above
        f64 crit_above
    }
    RuntimeConfig {
        string hkask_endpoint
        string okapi_endpoint
    }
    SoapBundle {
        string subjective
        string assessment
    }
    SoapObservation {
        string name
        json value
        string unit
        string severity
    }
    InferenceResponse {
        string text
        string backend
        string model
        u64 latency_ms
    }
    TokenUsage {
        u64 input_tokens
        u64 output_tokens
        u64 total_tokens
    }

    Event ||--|| Severity : "has"
    Profile ||--|| HostInfo : "describes"
    Profile ||--o{ GpuInfo : "has"
    RuleSet ||--o{ Rule : "contains"
    SoapBundle ||--o{ SoapObservation : "contains"
    InferenceResponse ||--o| TokenUsage : "tracks"
```

<!-- DIAGRAM_ALIGNMENT
id: DIAG-CORE-ER-001
type: erDiagram
verified_date: 2026-05-24
verified_against: crates/russell-core/src/event.rs, profile.rs, rule/mod.rs, inference.rs
reference_sources: Cockburn (2005) Ports & Adapters; russell-core crate
status: VERIFIED
-->

---

## 2. Observation Layer

```mermaid
erDiagram
    ProbeRegistry {
        int probe_count
    }
    ProbeDescriptor {
        string name PK
        string unit
        string category
    }
    Sample {
        string name
        f64 value_num
        string value_text
        string unit
    }
    ProprioResult {
        i64 age_s
        string severity
        i64 journal_stall_s
        f64 llm_p95_latency_ms
        i64 timer_drift_s
        f64 help_error_rate_pct
        u64 hkask_mcp_reachable_ms
        i64 remote_discovery_latency_s
        bool journal_chain_intact
        bool evidence_integrity_ok
    }
    ProprioReflex {
        int action_count
    }
    ReflexAction {
        string action_id PK
        string description
        string risk "none|low|medium|high|critical"
        string trigger
    }
    SentinelRun {
        string run_id PK
        int sample_count
        int breach_count
        u64 duration_ms
    }
    AutoimmuneGuard {
        string state "locked|unlocked"
    }

    ProbeRegistry ||--o{ ProbeDescriptor : "registers"
    ProbeDescriptor ||--o{ Sample : "produces"
    SentinelRun ||--o{ Sample : "collects"
    ProprioResult ||--|| SentinelRun : "evaluates"
    ProprioReflex ||--o{ ReflexAction : "queues"
    ProprioResult ||--|| ProprioReflex : "triggers"
    ProprioReflex ||--|| AutoimmuneGuard : "protected by"
```

<!-- DIAGRAM_ALIGNMENT
id: DIAG-OBS-ER-002
type: erDiagram
verified_date: 2026-05-24
verified_against: crates/russell-sentinel/src/probes/mod.rs, crates/russell-proprio/src/lib.rs, crates/russell-proprio/src/reflex.rs
reference_sources: Ashby (1956) Law of Requisite Variety; Beer (1972) VSM
status: VERIFIED
-->

---

## 3. Skill System

```mermaid
erDiagram
    Skill {
        string id PK
        string kind "actionable|lens"
        string version
        string authored
        string min_harness_version
        string visibility "public|private"
    }
    Probe {
        string id PK
        string cmd
        string capture
        string timeout
    }
    Intervention {
        string id PK
        string cmd
        string risk "none|low|medium|high|critical"
        bool idempotent
        string rollback
        string timeout
        bool needs_sudo
    }
    Safety {
        string max_auto_risk
        bool needs_network
    }
    Evaluation {
        int check_count
    }
    EvalCheck {
        string id PK
        string cmd
        string timeout
        i32 expect_exit
    }
    Dispatcher {
        string skill_dir
        string dry_run "enabled|disabled"
        string max_auto_risk
    }
    RunOutcome {
        string cmd
        bool dry_run
        i32 exit_code
        string stdout
        string stderr
        bool timed_out
        u64 duration_ms
    }
    RegistryCache {
        int skill_count
    }
    RegistryEntry {
        string skill_id PK
        string status
        string version
        string source
        string trust_tier
        u64 probe_runs
        u64 intervention_runs
    }

    Skill ||--o{ Probe : "defines"
    Skill ||--o{ Intervention : "defines"
    Skill ||--|| Safety : "constrains"
    Skill ||--o| Evaluation : "has"
    Evaluation ||--o{ EvalCheck : "contains"
    Dispatcher ||--o{ RunOutcome : "produces"
    RunOutcome ||--o| RunOutcome : "rollback"
    RegistryCache ||--o{ RegistryEntry : "tracks"
    Skill ||--|| RegistryEntry : "registered as"
```

<!-- DIAGRAM_ALIGNMENT
id: DIAG-SKILL-ER-003
type: erDiagram
verified_date: 2026-05-24
verified_against: crates/russell-skills/src/lib.rs, dispatch.rs, registry/mod.rs
reference_sources: Nix derivations; Ansible playbook structure; ADR-0024
status: VERIFIED
-->

---

## 4. Metacognitive Layer (The Nurse)

```mermaid
erDiagram
    HelpOutcome {
        string session_id PK
        string backend
        string evidence_dir
        string response
        string skip_reason
    }
    ResolvedAction {
        string skill_id
        string action_id
        string risk_band
        bool consent_required
    }
    HKaskToolInfo {
        string name PK
        string risk_band
        json input_schema
    }
    FallbackAdapter {
        string primary_backend
        string fallback_backend
        bool circuit_open
    }
    OkapiAdapter {
        string endpoint
        string model
    }
    HkaskAdapter {
        string endpoint
        string token_path
    }

    HelpOutcome ||--o| ResolvedAction : "proposes"
    ResolvedAction ||--o| HKaskToolInfo : "references"
    FallbackAdapter ||--|| OkapiAdapter : "primary"
    FallbackAdapter ||--|| HkaskAdapter : "fallback"
```

<!-- DIAGRAM_ALIGNMENT
id: DIAG-META-ER-004
type: erDiagram
verified_date: 2026-05-24
verified_against: crates/russell-meta/src/lib.rs, help.rs, action.rs, fallback_adapter.rs
reference_sources: Brooks (1991) subsumption architecture; ADR-0026
status: VERIFIED
-->

---

## 5. ACP Server

```mermaid
erDiagram
    SessionManager {
        int session_count
    }
    Session {
        string id PK "UUID v4"
        string persona
        string state "active|input_required|completed|closed"
        datetime created
        datetime last_activity
        string token_id FK
    }
    Turn {
        string id PK
        string role "user|assistant|tool"
        string content
        datetime timestamp
    }
    ToolCallRecord {
        string skill_id
        string intervention_id
        string probe_id
        json args
        string result
    }
    PendingAction {
        string action_type
        string skill_id
        string intervention_id
        string risk
        bool requires_consent
    }
    CapabilityToken {
        string token_id PK
        string token
        string issuer
        string nonce
        datetime expires_at
    }
    Attenuation {
        string kind "skill_restriction|rate_limit|time_bound|discharge"
        string value
    }
    MacaroonAuth {
        bool dev_mode_allowed
    }
    RateLimiter {
        int max_requests
        int window_seconds
    }

    SessionManager ||--o{ Session : "manages"
    Session ||--o{ Turn : "records"
    Session ||--o| PendingAction : "awaits"
    Turn ||--o{ ToolCallRecord : "contains"
    Session }o--o| CapabilityToken : "authenticated by"
    CapabilityToken ||--o{ Attenuation : "constrained by"
    MacaroonAuth ||--o{ CapabilityToken : "validates"
    AcpHandler ||--|| SessionManager : "owns"
    AcpHandler ||--|| MacaroonAuth : "uses"
    AcpHandler ||--|| RateLimiter : "enforces"
```

<!-- DIAGRAM_ALIGNMENT
id: DIAG-ACP-ER-005
type: erDiagram
verified_date: 2026-05-24
verified_against: crates/russell-acp-server/src/session.rs, handler.rs, auth.rs, types.rs
reference_sources: Birgisson et al. (2014) Macaroons; ADR-0027, ADR-0041
status: VERIFIED
-->

---

## 6. Agent Pod

```mermaid
erDiagram
    RussellPod {
        string id PK "UUID"
        string state "populated|registered|activated|deactivated"
    }
    AgentPersona {
        string name
        string agent_type "bot|replicant"
        string version
    }
    AgentCharter {
        string description
        string editor
    }
    AgentCapabilities {
        int count
    }
    AgentRights {
        int read_count
        int write_count
    }
    CnsEmitter {
        string endpoint
        bool enabled
    }
    ArtifactStore {
        string base_path
    }

    RussellPod ||--|| AgentPersona : "embodies"
    AgentPersona ||--|| AgentCharter : "has"
    AgentPersona ||--|| AgentCapabilities : "declares"
    AgentPersona ||--|| AgentRights : "grants"
    RussellPod ||--|| CnsEmitter : "emits via"
    RussellPod ||--|| ArtifactStore : "stores in"
```

<!-- DIAGRAM_ALIGNMENT
id: DIAG-AGENT-ER-006
type: erDiagram
verified_date: 2026-05-24
verified_against: crates/russell-agent/src/persona.rs, lifecycle.rs, pod.rs
reference_sources: hKask Agent Pod specification; ADR-0045
status: VERIFIED
-->

---

## 7. MCP Client

```mermaid
erDiagram
    HKaskMcpClient {
        string endpoint
        bool initialized
        string server_name
    }
    McpToolDefinition {
        string name PK
        string description
        json input_schema
        json annotations
        string server
    }
    ToolCallResult {
        bool is_error
    }
    ToolContent {
        string content_type
        string text
        json extra
    }
    ToolRegistry {
        int tool_count
        datetime last_refresh
        int ttl_seconds
    }

    HKaskMcpClient ||--o{ McpToolDefinition : "discovers"
    HKaskMcpClient ||--o{ ToolCallResult : "produces"
    ToolCallResult ||--o{ ToolContent : "contains"
    ToolRegistry ||--o{ McpToolDefinition : "caches"
    HKaskMcpClient ||--|| ToolRegistry : "refreshes"
```

<!-- DIAGRAM_ALIGNMENT
id: DIAG-MCP-ER-007
type: erDiagram
verified_date: 2026-05-24
verified_against: crates/russell-mcp/src/client.rs, registry.rs, types.rs
reference_sources: Model Context Protocol specification; ADR-0003, ADR-0025
status: VERIFIED
-->

---

## 8. Journal Persistence (SQLite Schema)

```mermaid
erDiagram
    samples {
        INTEGER ts PK "unix seconds"
        TEXT scope PK "host|self"
        TEXT probe PK
        REAL value_num
        TEXT value_text
        TEXT unit
    }
    events {
        TEXT id PK "ULID"
        INTEGER ts_unix
        TEXT ts "RFC 3339"
        TEXT schema
        TEXT scope "host|self"
        TEXT severity "info|warn|alert|crit"
        TEXT action
        INTEGER dry_run
        TEXT summary
        TEXT evidence_ref
        INTEGER duration_ms
        TEXT outputs "structured JSON (0005)"
        TEXT payload "full JSON"
        TEXT prev_hash FK "hash chain (0006)"
        TEXT hash "(0006)"
    }
    baselines {
        TEXT probe PK
        TEXT scope PK "host|self"
        REAL ewma_mean
        REAL ewma_var
        REAL p50
        REAL p95
        REAL p99
        INTEGER updated_ts
    }
    help_sessions {
        TEXT id PK "ULID"
        INTEGER ts_unix
        TEXT backend
        TEXT model
        TEXT note
        INTEGER prompt_chars
        INTEGER response_chars
        INTEGER latency_ms
        TEXT status "ok|error|fallback|threshold_skip"
        TEXT evidence_ref
    }
    confirmations {
        TEXT evidence_id PK
        INTEGER confirmed_ts
        TEXT actor
        TEXT note
    }
    used_nonces {
        TEXT token_id PK
        TEXT nonce PK
        INTEGER expires_at
    }
    schema_migrations {
        INTEGER version PK
        TEXT slug
        INTEGER applied_ts
    }

    events ||--o| events : "hash chain (prev_hash → hash)"
    baselines }o--|| samples : "derived from"
    confirmations }o--|| events : "references evidence"
    used_nonces }o--o| events : "token auth audit"
```

<!-- DIAGRAM_ALIGNMENT
id: DIAG-JOURNAL-ER-008
type: erDiagram
verified_date: 2026-05-24
verified_against: crates/russell-core/src/journal/migrations.rs
reference_sources: SQLite documentation; ADR-0004; PERSISTENCE_CATALOG.md
status: VERIFIED
-->

---

## 9. CLI Dispatch Layer

```mermaid
erDiagram
    Cli {
        string name "russell"
        string version
    }
    Command {
        int variant_count "20"
    }
    Paths {
        string base "from_env()"
    }
    HandlerStatus {
        string function "status::run"
        string crate_dep "russell-core"
        string async "no"
    }
    HandlerPod {
        string functions "status, activate, deactivate, persona_show, artifacts_list, artifacts_export"
        string crate_dep "russell-agent, russell-core"
        string async "activate, deactivate"
    }
    HandlerList {
        string function "list::run"
        string crate_dep "russell-core"
        string async "no"
    }
    HandlerDigest {
        string function "digest::run"
        string crate_dep "russell-core"
        string async "no"
    }
    HandlerSentinel {
        string function "sentinel_once::run"
        string crate_dep "russell-sentinel, russell-core"
        string async "no"
    }
    HandlerHelp {
        string function "help::run"
        string crate_dep "russell-meta, russell-core"
        string async "yes"
    }
    HandlerSkill {
        string functions "skill::list, skill::run"
        string crate_dep "russell-skills, russell-core"
        string async "run only"
    }
    HandlerSkillLifecycle {
        string functions "install_skill, prune_skill"
        string crate_dep "russell-skills, russell-core"
        string async "no"
    }
    HandlerProprio {
        string function "proprio::run"
        string crate_dep "russell-proprio, russell-core"
        string async "yes"
    }
    HandlerSelfTriage {
        string function "self_triage::run"
        string crate_dep "russell-meta, russell-proprio, russell-core"
        string async "yes"
    }
    HandlerDocs {
        string function "docs::run"
        string crate_dep "russell-core"
        string async "no"
    }
    HandlerVerify {
        string function "verify::run"
        string crate_dep "russell-core"
        string async "no"
    }

    Cli ||--|| Command : "parses via clap"
    Cli ||--|| Paths : "resolves"
    Command ||--o| HandlerStatus : "Status"
    Command ||--o| HandlerPod : "6 pod subcommands"
    Command ||--o| HandlerList : "List"
    Command ||--o| HandlerDigest : "Digest"
    Command ||--o| HandlerSentinel : "SentinelOnce"
    Command ||--o| HandlerHelp : "Jack"
    Command ||--o| HandlerSkill : "SkillList, SkillRun"
    Command ||--o| HandlerSkillLifecycle : "SkillInstall, SkillPrune"
    Command ||--o| HandlerProprio : "Proprio"
    Command ||--o| HandlerSelfTriage : "SelfTriage"
    Command ||--o| HandlerDocs : "Docs"
    Command ||--o| HandlerVerify : "VerifyJournal"
```

<!-- DIAGRAM_ALIGNMENT
id: DIAG-CLI-ER-009
type: erDiagram
verified_date: 2026-05-24
verified_against: crates/russell-cli/src/main.rs, crates/russell-cli/src/commands/mod.rs
reference_sources: Clap Parser/Subcommand derive macros; ADR-0003 (lift paths)
status: VERIFIED
-->

---

## References

- Cockburn, A. (2005). *Hexagonal Architecture*.
- Beer, S. (1972). *Brain of the Firm*.
- Ashby, W.R. (1956). *An Introduction to Cybernetics*.
- Brooks, R. (1991). "Intelligence without representation." *AI Journal*.
- Birgisson, A. et al. (2014). "Macaroons: Cookies with Contextual Caveats."
- Model Context Protocol specification. <https://modelcontextprotocol.io>
- Russell ADRs: 0003, 0004, 0024, 0025, 0026, 0027, 0041, 0045.
