---
title: "ADR-0019: Probe Cadence Separation and OKH Instrumentation"
audience: [developers, architects]
last_updated: 2026-05-06
ddmvss_context: "sentinel"
ddmvss_artifact: "adr"
version: "1.0.0"
status: "Active"
---



# ADR-0019: Separate probe cadences and adopt OKH instrumentation at tool/connector boundaries

- **Status:** Accepted
- **Date:** 2026-05-06
- **Deciders:** Project operator
- **Tags:** `sentinel`, `cadence`, `okh`, `observability`, `probes`

## Context

Phase 2 introduces two new probe families beyond the MVP's three
memory/load probes:

1. **Disk hygiene probes** — `statvfs("/")`, cache directory walks.
   Cheap (< 50ms). Fit the existing 5-minute Sentinel cadence.

2. **Package ecosystem probes** — subprocess invocations to `apt`,
   `pip`, `npm`, `brew`, `snap`, `flatpak`. Expensive (up to 5s
   timeout per provider × 8 providers = 40s worst case). Cannot
   run every 5 minutes without dominating the Sentinel cycle.

Simultaneously, the Kask platform's three-layer audit methodology
identifies that **every tool/connector boundary is a sensor placement point**. Russell already has
proprioception (ADR-0015), but it operates at the macro level
("did Sentinel run on time?"). The OKH discipline adds micro-level
instrumentation: how long did each probe take? Did the apt
subprocess timeout? How many bytes did the journal write?

These two concerns are coupled: cadence separation creates new
pipeline stages, and each stage boundary is a OKH measurement
point.

## Decision

### Part 1: Cadence Separation via Flag

The existing `russell sentinel-once` verb gains an `--extended`
flag that triggers the expensive package probes. The systemd
deployment uses **two timer units**:

```ini
# russell-sentinel.timer (existing, 5-minute cadence)
[Timer]
OnBootSec=60
OnUnitActiveSec=300

# russell-sentinel-extended.timer (new, hourly cadence)
[Timer]
OnBootSec=300
OnCalendar=hourly
Persistent=true
```

Both timers invoke the same binary:
- `russell sentinel-once` — disk + memory probes (cheap, 5-min)
- `russell sentinel-once --extended` — additionally runs package
  ecosystem probes (expensive, hourly)

This is **Option A from the spec** (second timer) with a twist:
same binary, different flag, separate timer unit. Benefits:
- Single binary (JR-1: austere)
- Independent failure domains (extended can fail without affecting core)
- Operator can `systemctl --user disable russell-sentinel-extended.timer`
  without affecting core health monitoring

### Part 2: OKH Instrumentation at Tool/Connector Boundaries

Russell adopts the Kask OKH naming convention for `tracing` spans
and fields. Every tool/connector boundary emits structured telemetry:

**Naming convention:**
```
okh.<layer>.<module>.<signal>
```

**Layers:** `tool`, `connector`, `pipeline`

**Placement rules:**

1. **Tool boundaries** — `tracing::instrument` on pure transform
   functions. Fields: `okh.tool.<module>.duration_ms`,
   `okh.tool.<module>.items_out`.

2. **Connector boundaries** — `tracing::instrument` on I/O
   functions. Fields: `okh.connector.<module>.latency_ms`,
   `okh.connector.<module>.success`,
   `okh.connector.<module>.error_class`.

3. **Pipeline stages** — `tracing::info_span!` around each
   collection phase. Fields: `okh.pipeline.<stage>.duration_ms`,
   `okh.pipeline.<stage>.items_out`.

**Error classification** — every error carries `okh.error.class`:
`timeout`, `parse_failure`, `io_error`, `permission_denied`,
`not_found`, `rate_limited`.

**Relationship to proprioception (ADR-0015):**
OKH instrumentation is the *mechanism* by which proprioception
observes Russell's internals. The existing `sentinel_last_run_age_s`
self-vital is a pipeline-level OKH signal. Phase 2 adds
connector-level and tool-level signals that the meta-Sentinel can
aggregate.

### Part 3: Tool/Connector Refactoring of Existing Probes

The existing `probes.rs` is refactored into the tool/connector
pattern:

- `probes/connectors.rs` — I/O functions (`read_proc_meminfo`,
  `read_proc_loadavg`, `statvfs_raw`, `run_cmd`)
- `probes/tools.rs` — pure transforms (`parse_meminfo_kib`,
  `compute_used_pct`, `kib_to_mib`)
- `probes/memory.rs` — composes connector + tool for memory probes
- `probes/disk.rs` — composes connector + tool for disk probes
- `probes/packages.rs` — composes connector + tool for package probes
- `probes/mod.rs` — orchestrator (`collect()`, `collect_extended()`)

Each composition function is a thin glue layer:
```rust
fn mem_available_mib() -> Option<f64> {
    let raw = connectors::read_proc_meminfo()?;  // connector
    tools::parse_meminfo_kib(&raw, "MemAvailable") // tool
        .map(tools::kib_to_mib)                    // tool
}
```

## Consequences

### Positive

- Expensive probes cannot starve the core 5-minute cadence.
- Every I/O boundary is instrumented — Russell can detect its own
  degradation at the probe level (not just "did the cycle complete?").
- Tool functions are independently unit-testable without filesystem
  or subprocess access.
- OKH fields flow through the existing `tracing` subscriber to
  stderr (Phase 0) and will flow to journald (Phase 2+).
- The pattern is consistent with Kask platform conventions, enabling
  future integration.

### Negative / accepted costs

- Two systemd timer units instead of one. Marginal packaging
  complexity.
- `tracing` spans add ~microsecond overhead per probe. Acceptable
  given 5-minute cadence.
- Refactoring existing probes.rs is churn on working code. Justified
  by the testability and instrumentation gains.

### Neutral

- The `--extended` flag is additive; existing `russell sentinel-once`
  behaviour is unchanged without the flag.
- OKH field names are structured metadata in tracing spans; they
  don't affect journal `samples` table schema.

## Alternatives considered

### Conditional cadence in single timer

Run expensive probes only if N minutes have elapsed since last
extended run, checked via journal query. Rejected: conflates
cadences in one code path; Sentinel cycle duration becomes
unpredictable; harder to reason about failure modes.

### Separate binary (`russell-pkg-sentinel`)

Maximum isolation but violates JR-1 (one binary). Rejected.

### Skip OKH, keep ad-hoc tracing

The existing `tracing::debug!` calls are unstructured. Without
OKH naming, proprioception cannot programmatically detect "the
apt connector is timing out 80% of the time." Rejected: the
whole point of proprioception is structured self-observation.

### Defer OKH to Phase 3

Possible, but the refactoring to tool/connector separation is
happening now anyway. Adding OKH spans at the same time is
marginal effort. Rejected: do it once, do it right.

## Implementation notes

1. **Refactoring order:**
   - Extract `probes/connectors.rs` and `probes/tools.rs` from
     existing `probes.rs`
   - Add `probes/mod.rs` orchestrator
   - Add OKH spans to connector and tool functions
   - Add `probes/disk.rs` (new, already decomposed)
   - Add `probes/packages.rs` (new, already decomposed)
   - Add `--extended` flag to CLI
   - Add `russell-sentinel-extended.timer` to packaging

2. **OKH span example:**
   ```rust
   #[tracing::instrument(fields(
       okh.connector.proc.target = "/proc/meminfo",
       okh.connector.proc.success,
   ))]
   fn read_proc_meminfo() -> Option<String> {
       let result = std::fs::read_to_string("/proc/meminfo").ok();
       tracing::Span::current()
           .record("okh.connector.proc.success", result.is_some());
       result
   }
   ```

3. **No new crate dependencies** for OKH. The `tracing` crate
   (already in workspace) provides all needed primitives.

4. **Proprioception integration:** The meta-Sentinel can query
   OKH signals by subscribing to the tracing layer. In Phase 2,
   a `tracing::Layer` implementation aggregates OKH fields into
   the `samples` table with `scope='self'`. This is the bridge
   between ADR-0015 (proprioception) and OKH.

## References

- [`DISK_PKG_HYGIENE_SPEC.md`](../specifications/DISK_PKG_HYGIENE_SPEC.md) — parent spec
- [`06-open-questions.md`](../specifications/disk-pkg-hygiene/06-open-questions.md) — Question 1
- [ADR-0015](0015-proprioception-self-health.md) — proprioception
- [ADR-0013](0013-rust-workspace-layout.md) — crate topology
- Stafford Beer, *Brain of the Firm* — System 3* (self-observation)
