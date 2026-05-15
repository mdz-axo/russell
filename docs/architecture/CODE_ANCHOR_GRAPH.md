---
title: "Code Anchor Graph — Public Type Registry"
audience: [architects, developers, agents]
last_updated: 2026-05-14
togaf_phase: "C"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Data Architecture -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-14 -->

# Russell — Code Anchor Graph

<!-- DIAGRAM_ALIGNMENT
id: DIAG-ANCHOR-001
type: ER diagram
verified_date: 2026-05-13
verified_against: All public types extracted from russell/crates/*/src/
reference_sources: russell-core, russell-sentinel, russell-doctor, russell-skills, russell-proprio
status: VERIFIED
-->

## 1. Russell Core — Domain Types

```
russell_core::event::EventId rdf:type struct .
russell_core::event::Event rdf:type struct .
russell_core::event::Severity rdf:type enum .  # Info, Warn, Alert, Crit; impl Display + FromStr
russell_core::event::Scope rdf:type enum .  # Host, Self; impl Display + FromStr

russell_core::profile::Profile rdf:type struct .  # russell.profile.v1
russell_core::profile::HostInfo rdf:type struct .  # os, chassis, cpu, memory, swap
russell_core::profile::GpuInfo rdf:type struct .  # pci, vendor_id, name, gfx, role

russell_core::journal::JournalWriter rdf:type struct .  # SQLite WAL connection
russell_core::journal::JournalReader rdf:type struct .  # read-only handle
russell_core::journal::HelpSessionInput rdf:type struct .  # structured input for append_help_session
russell_core::journal::HelpSessionStatus rdf:type enum .  # Ok, Error, Fallback, ThresholdSkip

russell_core::time::now_rfc3339 rdf:type fn .  # RFC 3339 UTC timestamp
russell_core::time::now_unix rdf:type fn .  # Unix seconds
russell_core::time::now_date_iso8601 rdf:type fn .  # YYYY-MM-DD date string
russell_core::time::approx_days_between rdf:type fn .  # approximate day count between dates

russell_core::rule::Rule rdf:type struct .  # probe + directional thresholds
russell_core::rule::RuleSet rdf:type struct .  # loaded rules keyed by probe name

russell_core::schedule::Schedule rdf:type struct .  # model + time window
russell_core::schedule::ScheduleSet rdf:type struct .  # collection of schedules

russell_core::paths::Paths rdf:type struct .  # config, state, data

russell_core::error::CoreError rdf:type enum .  # BasePath, Io, Json, Sqlite, etc.
```

## 2. Russell Sentinel — Probe System

```
russell_sentinel::probes::ProbeDescriptor rdf:type trait .  # name(), unit(), collect()
russell_sentinel::probes::Sample rdf:type struct .  # name, value_num, value_text, unit
russell_sentinel::probes::ProbeRegistry rdf:type struct .  # Vec<Box<dyn ProbeDescriptor>>

# GPU probes (5 zero-sized types)
russell_sentinel::probes::gpu::GpuVramUsedPct rdf:type struct .  # implements ProbeDescriptor
russell_sentinel::probes::gpu::GpuVramUsedMib rdf:type struct .
russell_sentinel::probes::gpu::GpuVramTotalMib rdf:type struct .
russell_sentinel::probes::gpu::GpuTempC rdf:type struct .
russell_sentinel::probes::gpu::GpuUtilPct rdf:type struct .

# Memory probes (5)
russell_sentinel::probes::memory::MemAvailableMib rdf:type struct .
russell_sentinel::probes::memory::SwapUsedMib rdf:type struct .
russell_sentinel::probes::memory::LoadAvg1m rdf:type struct .
russell_sentinel::probes::memory::MemPressureSome rdf:type struct .
russell_sentinel::probes::memory::MemPressureFull rdf:type struct .

# Disk probes (3)
russell_sentinel::probes::disks::DiskIoPressureSome rdf:type struct .
russell_sentinel::probes::disks::DiskIoPressureFull rdf:type struct .
russell_sentinel::probes::disks::DiskRootUsedPct rdf:type struct .

# Network probes (2)
russell_sentinel::probes::network::NetTcpConnections rdf:type struct .
russell_sentinel::probes::network::NetTcp6Connections rdf:type struct .

# Process probes (5)
russell_sentinel::probes::process::ProcTotalCount rdf:type struct .
russell_sentinel::probes::process::ProcZombieCount rdf:type struct .
russell_sentinel::probes::process::ProcStuckCount rdf:type struct .
russell_sentinel::probes::process::ProcRunningCount rdf:type struct .
russell_sentinel::probes::process::ProcTopMemPct rdf:type struct .

# Systemd probes (3)
russell_sentinel::probes::systemd::SystemdDegraded rdf:type struct .
russell_sentinel::probes::systemd::SystemdUserFailedCount rdf:type struct .
russell_sentinel::probes::systemd::SystemdSystemFailedCount rdf:type struct .
```

## 3. Russell Doctor — LLM Consultation

```
russell_doctor::client::LlmClient rdf:type trait .  # chat(&SoapPrompt) -> Result<LlmResponse>
russell_doctor::client::SoapPrompt rdf:type struct .  # system, subjective, objective, rendered
russell_doctor::client::LlmResponse rdf:type struct .  # content, model, tokens, latency
russell_doctor::client::Backend rdf:type enum .  # Okapi, Mock, Offline
russell_doctor::client::EscalateMin rdf:type enum .  # Crit, Alert, Warn, Always

russell_doctor::oai_client::OkapiClient rdf:type struct .  # OpenAI-compatible client
russell_doctor::mock::MockClient rdf:type struct .  # deterministic test client

russell_doctor::help::HelpOutcome rdf:type struct .  # session_id, backend, evidence_dir, response
russell_doctor::help::SkipReason rdf:type enum .  # OfflineFallback, ThresholdSkip

russell_doctor::action::ResolvedAction rdf:type enum .  # Probe, Intervention, KaskTool
russell_doctor::action::KaskToolInfo rdf:type struct .  # name, risk_band, input_schema
russell_doctor::action::ActionError rdf:type enum .  # MalformedPrefix, MissingSeparator, etc.

russell_doctor::error::DoctorError rdf:type enum .  # Io, Json, Core, Http, Auth, etc.
```

## 4. Russell Skills — Playbook Execution

```
russell_skills::Skill rdf:type struct .  # id, version, probes, interventions, safety
russell_skills::Probe rdf:type struct .  # id, cmd, capture, timeout
russell_skills::Intervention rdf:type struct .  # id, cmd, risk, idempotent, rollback
russell_skills::Safety rdf:type struct .  # max_auto_risk, require_human_for
russell_skills::RiskBand rdf:type enum .  # None, Low, Medium, High, Critical
russell_skills::Rollback rdf:type enum .  # RollbackId, NoneNeeded, Reboot

russell_skills::dispatch::Dispatcher rdf:type struct .  # subprocess dispatcher
russell_skills::dispatch::RunOutcome rdf:type struct .  # cmd, exit_code, stdout, stderr
russell_skills::dispatch::RollbackOutcome rdf:type struct .  # forward, rollback, rollback_applied
russell_skills::dispatch::DryRun rdf:type enum .  # Enabled, Disabled
russell_skills::dispatch::StepType rdf:type enum .  # Probe, Intervention

russell_skills::registry::RegistryCache rdf:type struct .  # BTreeMap<skill_id, RegistryEntry>
russell_skills::registry::RegistryEntry rdf:type struct .  # status, version, symptoms, telemetry
russell_skills::registry::LifecycleStatus rdf:type enum .  # Discovered→Evaluated→Installed→Active→…
russell_skills::registry::ScanSeverity rdf:type enum .  # Info, Warn, Block; impl as_str()
russell_skills::registry::SafetyScan rdf:type struct .  # 7-rule content safety checker
```

## 5. Russell Proprio — Self-Observation

```
russell_proprio::TimerSource rdf:type trait .  # read_last_trigger_us()
russell_proprio::SystemdTimerSource rdf:type struct .  # production TimerSource
russell_proprio::AutoimmuneGuard rdf:type struct .  # recursion guard
russell_proprio::ProprioResult rdf:type struct .  # 5 self-vitals with severity
```

## 6. Russell MCP — Kask Tool Interface

```
russell_mcp::client::KaskMcpClient rdf:type struct .  # JSON-RPC stdio client
russell_mcp::registry::ToolRegistry rdf:type struct .  # tool cache with TTL
russell_mcp::config::KaskMcpConfig rdf:type struct .  # endpoint, tool_ttl, auth
```

## 7. Cross-Crate Dependencies

```
russell-cli depends_on russell-core, russell-sentinel, russell-doctor, russell-skills, russell-proprio, russell-mcp
russell-doctor depends_on russell-core
russell-skills depends_on russell-core
russell-proprio depends_on russell-core
russell-sentinel depends_on russell-core
russell-mcp depends_on nothing (standalone)
```
