---
title: "ADR-0010: Observability Stack (Deferred)"
audience: [developers, architects]
last_updated: 2026-04-18
togaf_phase: "H"
version: "1.0.0"
status: "Deferred"
---

<!-- TOGAF_DOMAIN: Change Management -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Accepted — Deferred -->
<!-- LAST_UPDATED: 2026-04-18 -->

> **Deferred.** This ADR's subject is outside the MVP boundary per
> [`../../specifications/MVP_SPEC.md`](../../specifications/MVP_SPEC.md) §5.
> It remains **Accepted** — when its phase opens, it ships this way.


<!--
audience: logging / tracing / metrics contributors
last-reviewed: 2026-04-17
-->

# ADR-0010: Observability stack — tracing + journald, no Prometheus

- **Status:** Accepted
- **Date:** 2026-04-17
- **Deciders:** Project founders
- **Tags:** `observability`, `tracing`, `journald`

## Context

Russell is a single-host tool under systemd. It already writes
structured events to its own SQLite journal (`events`) and
samples to `samples`. That's the domain-level observability.
It also needs standard software-level observability: log
lines, spans, error traces, for when Russell itself misbehaves.

Options:

- `log` + `env_logger` — minimal, widely understood, no
  structure.
- `tracing` ecosystem — structured, spans, fields,
  subscribers for journald/OTel/fmt.
- Prometheus scrape endpoint — centralized metrics.

See [ADR-0003](0003-mcp-transport.md): no network listener in
v1.

## Decision

1. **`tracing` is the observability surface.** All log output,
   span instrumentation, and in-process metrics go through
   `tracing`.
2. **`tracing-subscriber`** composes:
   - `tracing-journald` as the primary subscriber when a
     `JOURNAL_STREAM` is detected (i.e. running under
     systemd), and
   - `tracing-subscriber::fmt` as the fallback for ad-hoc
     runs (terminal use).
3. **`RUST_LOG`** is the standard filter control, defaulting
   to `russell=info`.
4. **Spans are mandatory** around:
   - each MCP tool invocation (`otel.name =
     "mcp.<tool_name>"`),
   - each skill probe (`otel.name = "skill.probe"`),
   - each skill intervention (`otel.name =
     "skill.intervention"`),
   - each LLM request (`otel.name = "doctor.llm"`),
   - each journal write (`otel.name = "journal.write"`).
5. **In-process metrics** use the
   `metrics` crate façade with an in-memory recorder. The
   proprioception meta-Sentinel reads these counters /
   histograms on its cadence and writes them into
   `proprio_samples`. This replaces any scrape endpoint.
6. **No Prometheus scrape, no OpenTelemetry export, in
   v1.** Exporters are opt-in and require an ADR.

## Consequences

### Positive

- `journalctl --user -t russell -f` is the one-liner
  debug path.
- Structured fields match the journal event schema
  (same snake_case vocabulary), so joins are natural.
- Proprioception gets metrics "for free" via the
  in-memory recorder.
- No network port to secure.

### Negative / accepted costs

- External dashboards require the operator to write a
  script that reads from `journalctl` and/or
  `journal.db`. Acceptable given single-host scope.
- Changing later to OpenTelemetry means adding an
  exporter; `tracing` is chosen partly because its
  `tracing-opentelemetry` bridge already exists if we
  need it.

### Neutral

- `tracing` is the Rust ecosystem default; engineers
  will find it familiar.

## Alternatives considered

### `log` + `env_logger`

Rejected. No structured fields, no spans, and a
journald bridge would be additional work.

### Prometheus `/metrics` endpoint

Rejected for v1. Violates "no listener" and adds a
second operational service; the proprioception
pathway already covers the use case.

### OpenTelemetry OTLP exporter

Rejected for v1; available via a future ADR as an
opt-in export path if an operator wants Grafana or
similar.

## Implementation notes

- `russell-core::telemetry::init()` wires the
  subscriber and the metrics recorder; called once
  from `russell-cli::main` and from the MCP server
  entry.
- Span fields follow the `harness.event.v1` field
  naming: `module`, `tier`, `severity`, `dry_run`,
  `evidence_ref`, `skill_id`, `probe_id`,
  `intervention_id`, `risk`.
- The `russell-proprio` crate subscribes to the metrics
  recorder; documented in
  [`../../archive/proprioception.md`](../../archive/proprioception.md).

## References

- tracing: https://docs.rs/tracing
- tracing-journald: https://docs.rs/tracing-journald
- metrics: https://docs.rs/metrics
- systemd journald: https://www.freedesktop.org/software/systemd/man/systemd-journald.service.html
