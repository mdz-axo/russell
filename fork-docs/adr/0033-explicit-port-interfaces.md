---
title: "ADR-0033 — Explicit Port Interfaces"
audience: [developers, architects]
last_updated: 2026-05-19
ddmvss_context: "cross-cutting"
ddmvss_artifact: "adr"
version: "1.0.0"
status: "Active"
---


# ADR-0033 — Explicit Port Interfaces


## Context

The adversarial multi-perspective review (2026-05-19) identified weakness A2:

> **A2 — No explicit port interfaces** — traits exist but are not enforced as hexagonal ports. Ad-hoc interface design, not systematic. Speed of MVP delivery.

Russell's architecture uses SQLite for journal persistence. Consumers (sentinel, meta, proprio) call methods directly on `JournalReader` and `JournalWriter`, coupling them to the concrete implementation.

The existing `JournalWritePort` and `JournalReadPort` traits (in `journal/port.rs`) define the hexagonal boundary but are incomplete:
- Only `recent()` method in `JournalReadPort`
- Consumers must use concrete types, not trait objects
- No test doubles for full integration testing

This violates the hexagonal ports/adapters pattern (Cockburn) and the capability separation principle (Miller).

## Decision

Expand the port traits to cover all journal operations:

1. **`JournalReadPort` expansion** — Add 6 methods:
   - `severity_counts()` — Count events by severity in time window
   - `last_host_sample_ts()` — Get last host sample timestamp
   - `previous_sample()` — Get previous sample for rate-of-change
   - `host_samples_summary()` — Get sample summary for time window
   - `read_baselines()` — Read all baselines
   - `count_reflex_events()` — Count reflex events for probe

2. **Full implementation** — `JournalReader` implements all trait methods via delegation.

3. **Test doubles** — `InMemoryJournal` already implements `JournalWritePort`; can be extended for read operations if needed.

4. **Consumer migration** — Consumers can now depend on `&dyn JournalReadPort` instead of `&JournalReader`, enabling:
   - Test injection of in-memory journals
   - Future storage backend swaps without consumer changes
   - Capability-based access control (read-only vs write-only references)

## Consequences

### Positive

- **Hexagonal architecture** — Consumers depend on port traits, not concrete implementation.

- **Testability** — In-memory journal can be injected for integration tests.

- **Capability separation** — Read-only code receives `&dyn JournalReadPort`, write-only code receives `&dyn JournalWritePort`.

- **Miller principle** — Possession of port reference IS the permission; type system enforces least-authority.

- **Future flexibility** — Storage backend can be swapped (e.g., PostgreSQL, embedded KV) without changing consumer code.

### Negative

- **Trait boilerplate** — Each method requires explicit impl delegation to concrete type.

- **No new functionality** — Traits expose existing methods; no new capabilities added.

- **Migration effort** — Existing code uses concrete types; trait adoption is incremental.

### Neutral

- **Backward compatible** — Concrete types continue to work; traits are optional.

- **No breaking changes** — All existing tests pass with trait expansion.

## Implementation

### Trait Definition

```rust
pub trait JournalReadPort: Send + Sync {
    fn recent(&self, limit: usize) -> Result<Vec<EventRow>>;
    fn severity_counts(&self, since: i64, until: i64) -> Result<SeverityCounts>;
    fn last_host_sample_ts(&self) -> Result<Option<i64>>;
    fn previous_sample(&self, probe: &str, now: i64) -> Result<Option<(f64, i64)>>;
    fn host_samples_summary(&self, since: i64, until: i64) -> Result<Vec<SampleSummary>>;
    fn read_baselines(&self) -> Result<Vec<BaselineRow>>;
    fn count_reflex_events(&self, probe: &str, since: i64, until: i64) -> Result<usize>;
}
```

### Code Changes

| File | Change |
|---|---|
| `russell-core/src/journal/port.rs` | Expand `JournalReadPort` trait, implement all methods for `JournalReader` |

### Usage Example

```rust
// Consumer depends on trait, not concrete type
fn evaluate_samples(rules: &RuleSet, samples: &[Sample], reader: &dyn JournalReadPort) -> Vec<Event> {
    // Can read previous samples via trait method
    let prev = reader.previous_sample("mem_available_mib", now);
    // ...
}

// Test injection
let journal = InMemoryJournal::default();
evaluate_samples(&rules, &samples, &journal); // Uses trait impl
```

## Compliance

| Principle | Compliance |
|---|---|
| **JR-1** (Austere) | Minimal expansion: only essential read methods added to trait |
| **JR-6** (Reuse over dependency) | Traits reuse existing `JournalReader` methods via delegation |
| **Cockburn** (Hexagonal) | Consumers depend on port traits, not concrete storage |
| **Miller** (Capability separation) | Read/write capabilities separated via distinct traits |

## Future Work

- **Consumer migration** — Update sentinel, meta, proprio to use trait objects instead of concrete types.

- **In-memory read impl** — Extend `InMemoryJournal` to implement `JournalReadPort` for full test isolation.

- **Additional ports** — Consider `ProbePort` (probe collection), `DispatchPort` (skill dispatch), `LlmPort` (LLM interactions) for full hexagonal coverage.

- **Capability-based routing** — Use trait objects to enforce read-only vs write-only access at runtime.

## References

- Adversarial Review Action Plan §A2 (Task A2)
- `docs/architecture/PRINCIPLES_CATALOG.md` (JR principles)
- `crates/russell-core/src/journal/port.rs` (Port trait definitions)
- Cockburn, A. (2005). "Hexagonal Architecture"
