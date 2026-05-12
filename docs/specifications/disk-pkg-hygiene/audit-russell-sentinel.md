---
title: "Crate Audit: russell-sentinel"
audience: [developers, agents]
last_updated: 2026-05-06
status: "Completed — refactoring landed per ADR-0019"
---

# Crate Audit: russell-sentinel

## Summary

- Files audited: 2
- Clean (tool): 0
- Clean (connector): 0
- Conflated: 2
- Parameterization issues: 3
- OKH sensors needed: 7

---

## Layer 1: Separation Issues

| File | Transformation (extract as tool) | Transfer (extract as connector) | Boundary |
|------|----------------------------------|--------------------------------|----------|
| `src/probes.rs` — `read_meminfo_kib()` | Parse key-value text to extract a u64 | `fs::read_to_string("/proc/meminfo")` | The `fs::read_to_string` call returns raw text; parsing it is a separate concern |
| `src/probes.rs` — `mem_available_mib()` | Convert KiB to MiB (`kib as f64 / 1024.0`) | Calls `read_meminfo_kib` which does I/O | The I/O is buried inside `read_meminfo_kib`; the division is the tool |
| `src/probes.rs` — `swap_used_mib()` | Compute `total - free`, convert KiB to MiB | Calls `read_meminfo_kib` twice (I/O) | Same pattern: I/O hidden inside helper |
| `src/probes.rs` — `load_avg_1m()` | Parse first whitespace-delimited token as f64 | `fs::read_to_string("/proc/loadavg")` | File read is connector; parse is tool |
| `src/lib.rs` — `run_once()` | Timestamp generation, sample iteration | `writer.append_sample()` (SQLite write) | The loop body mixes "prepare args" (tool) with "write to DB" (connector) |

### Diagnosis

Every probe function **conflates** connector (filesystem read) with tool (parsing/arithmetic). The pattern is:

```rust
fn probe_name() -> Option<f64> {
    let text = fs::read_to_string(PATH).ok()?;  // CONNECTOR
    // ... parsing and arithmetic ...            // TOOL
    Some(result)
}
```

This makes the tool layer untestable without a real `/proc` filesystem.

---

## Layer 2: Parameterization Issues

| File | Hardcoded value | Should be | Severity |
|------|----------------|-----------|----------|
| `src/probes.rs` | `"/proc/meminfo"` path | Parameter to connector function | medium |
| `src/probes.rs` | `"/proc/loadavg"` path | Parameter to connector function | medium |
| `src/lib.rs` | `Scope::Host` hardcoded in `run_once()` | Could be parameter for self-scope probes | low |

**Not defects (correctly hardcoded):**
- `1024.0` divisor (KiB→MiB is a universal constant)
- Key names like `"MemAvailable"`, `"SwapTotal"`, `"SwapFree"` (these ARE the probe's purpose)
- Sample names like `"mem_available_mib"` (domain vocabulary)

---

## Layer 3: OKH Instrumentation Plan

| Location | Type | Sensor | Fields | Priority |
|----------|------|--------|--------|----------|
| `read_proc_meminfo()` (to be extracted) | connector | span | `okh.connector.proc.target="/proc/meminfo"`, `okh.connector.proc.success` | high |
| `read_proc_loadavg()` (to be extracted) | connector | span | `okh.connector.proc.target="/proc/loadavg"`, `okh.connector.proc.success` | high |
| `parse_meminfo_kib()` (to be extracted) | tool | span | `okh.tool.memory.items_out` (count of parsed keys) | low |
| `kib_to_mib()` (to be extracted) | tool | — | No span needed (trivial arithmetic) | — |
| `collect()` | pipeline | span | `okh.pipeline.sentinel_collect.duration_ms`, `okh.pipeline.sentinel_collect.items_out` | high |
| `run_once()` | pipeline | span | `okh.pipeline.sentinel_run.duration_ms`, `okh.pipeline.sentinel_run.samples_written` | high |
| `writer.append_sample()` (in loop) | connector | span | `okh.connector.journal.success`, `okh.connector.journal.error_class` | medium |

### Sensor Rationale

- **Pipeline spans** (`collect`, `run_once`) — these are the signals proprioception already needs. `sentinel_last_run_age_s` is derived from the pipeline completion timestamp. Adding duration gives richer self-observation.
- **Connector spans** (`read_proc_*`, `append_sample`) — if `/proc` becomes unavailable (container, namespace issue) or the journal write fails (disk full, WAL stuck), OKH detects it at the boundary where it happens.
- **Tool spans** — mostly unnecessary for trivial arithmetic. Only add spans to tools that process variable-size input (e.g., parsing a full meminfo file, parsing apt stdout with hundreds of lines).

---

## Recommended Refactoring Order

1. **Extract connectors** — Create `probes/connectors.rs`:
   - `read_file_to_string(path: &str) -> Option<String>` — generic proc/sys file reader
   - Instrument with `okh.connector.proc.*` span

2. **Extract tools** — Create `probes/tools.rs`:
   - `parse_meminfo_kib(content: &str, key: &str) -> Option<u64>` — pure parser
   - `kib_to_mib(kib: u64) -> f64` — pure arithmetic
   - `parse_loadavg_1m(content: &str) -> Option<f64>` — pure parser

3. **Rewrite probes as composition** — `probes/memory.rs`:
   ```rust
   fn mem_available_mib() -> Option<f64> {
       let raw = connectors::read_file_to_string("/proc/meminfo")?;
       tools::parse_meminfo_kib(&raw, "MemAvailable")
           .map(tools::kib_to_mib)
   }
   ```

4. **Add pipeline span** to `collect()` and `run_once()`

5. **Add new probe modules** (`disk.rs`, `packages.rs`) already in decomposed form

6. **Add `--extended` flag** per ADR-0019

---

## Post-Refactoring File Layout

```
crates/russell-sentinel/src/
├── lib.rs                  # run_once(), run_extended()
└── probes/
    ├── mod.rs              # collect(), collect_extended() [pipeline orchestrator]
    ├── connectors.rs       # I/O boundary functions [CONNECTOR]
    ├── tools.rs            # Pure transforms [TOOL]
    ├── memory.rs           # Memory probe compositions
    ├── disk.rs             # Disk hygiene probe compositions (Phase 2)
    ├── packages.rs         # Package ecosystem probe compositions (Phase 2)
    └── provenance.rs       # Provenance registry adapter (Phase 2)
```

Each file is cleanly one thing:
- `connectors.rs` — all side effects, all OKH connector spans
- `tools.rs` — all pure logic, unit-testable with canned strings
- `memory.rs` / `disk.rs` / `packages.rs` — thin composition glue
- `mod.rs` — pipeline orchestration with OKH pipeline spans
