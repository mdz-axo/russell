# Skill Health Model — OKH Span Taxonomy

> Defines the `SkillHealth` aggregate and its OKH tracing span hierarchy.
> References: `crates/russell-skills/src/registry/health_ext.rs`
> Version: 1.0.0 | 2026-05-15

---

## SkillHealth Aggregate

```
SkillHealth {
    quality_score:    f64,         // 0.0–1.0, 6 weighted dimensions
    reliability:      f64,         // EWMA probe success rate (α=0.2)
    latency_p95_ms:   Option<f64>, // 95th percentile probe latency
    freshness:        u32,         // Days since installation
    safety_posture:   SafetyPosture,  // Pass | Warn | Block
    staleness_days:   i32,         // Days to 180d threshold (negative = stale)
    probe_runs:       u64,         // Total probe executions
    intervention_runs: u64,        // Total intervention executions
    last_error:       Option<String>,
}
```

## Six-Dimension Quality Score (from `health.rs:80-94`)

| Dimension | Weight | Input | Score Logic |
|---|---|---|---|
| Manifest completeness | 0.20 | `manifest.yaml` presence of `id:`, `version:`, `authored:`, `symptoms:` | Fraction present / 4 |
| Probe coverage | 0.25 | Count of `- id:` entries in `probes:` section | 1.0 if any, 0.0 if none |
| Intervention coverage | 0.20 | Count of entries in `interventions:` section | 1.0 if any, 0.5 if none |
| Rollback quality | 0.15 | Existence of `rollback:`, `none_needed`, or `reboot` | 1.0 (rollback field), 0.8 (none/reboot), 0.3 (none) |
| Script quality | 0.10 | Shebang, `set -e`, command presence | Fraction of 3 checks passed |
| Documentation | 0.10 | KNOWLEDGE.md existence + symptom list | 0.6 (knowledge) + 0.4 (symptoms) |

---

## OKH Span Taxonomy

All spans follow the `okh.<layer>.<module>.<signal>` convention from ADR-0019.
Layer: `skill`, Module: `eval`.

```
okh.skill.eval.quality       ← quality score computation
  ├── Fields: skill_id, quality_weighted
  ├── Duration: ~10μs (string parsing)
  └── Level: debug

okh.skill.eval.reliability   ← EWMA reliability
  ├── Fields: skill_id, probe_runs, recent_failures
  ├── Duration: ~5μs (arithmetic)
  └── Level: debug

okh.skill.eval.latency       ← p95 latency
  ├── Fields: skill_id, ewma_ms
  ├── Duration: ~2μs
  └── Level: debug

okh.skill.eval.freshness     ← days since evaluation
  ├── Fields: skill_id, installed
  ├── Duration: ~5μs (date parsing)
  └── Level: debug

okh.skill.eval.safety        ← safety posture
  ├── Fields: skill_id
  ├── Duration: ~50μs (regex scanning, 7 rule categories)
  └── Level: debug

okh.skill.eval.staleness     ← days to staleness threshold
  ├── Fields: skill_id
  ├── Duration: ~5μs
  └── Level: debug

okh.skill.eval.complete      ← composite assessment
  ├── Fields: skill_id, quality_score, reliability, latency_p95_ms,
  │           freshness_days, safety_posture, staleness_days
  ├── Duration: ~80μs (sum of sub-spans)
  └── Level: info
```

---

## Auto-Pruning Decision Flow

```
                    ┌─────────────────────────┐
                    │  SkillHealth::compute()  │
                    └───────────┬─────────────┘
                                │
                    ┌───────────▼─────────────┐
                    │  quality_score < 0.3?    │
                    │  AND staleness_days ≤ 0? │
                    │  AND probe_runs > 20?    │
                    └───────────┬─────────────┘
                                │
                    ┌──── Yes ──┼── No ────┐
                    ▼           │           ▼
      ┌───────────────────┐    │    ┌──────────────┐
      │ ACTIVE →          │    │    │ Stay Active  │
      │ StaleWarning      │    │    │ (re-evaluate │
      │ (typestate        │    │    │  next cycle) │
      │  transition)      │    │    └──────────────┘
      └───────┬───────────┘    │
              │                │
    ┌─────────▼─────────┐      │
    │ StaleWarning      │      │
    │ persist > 30 days?│      │
    └───────┬───────────┘      │
            │                  │
    ┌── Yes ──┼── No ────┐     │
    ▼         │          ▼     │
┌───────┐     │    ┌────────┐  │
│ Depre │     │    │ Stay   │  │
│ cated │     │    │ Stale  │  │
└───┬───┘     │    │ Warning│  │
    │         │    └────────┘  │
    ▼         │                │
┌───────┐     │                │
│ Retire│     │                │
│  (op  │     │                │
│  cmd) │     │                │
└───────┘     │                │
```

---

## Scenario Tester Integration

Scenario tester probe results feed into `reliability` and `latency_p95` dimensions.

```
  scenario-tester probe runs
          │
          ├──→ success/failure status → reliability (EWMA)
          │      - Each scenario test run is a "probe" from registry PoV
          │      - record_probe_execution(entry, success, duration_ms, error)
          │
          └──→ duration_ms → avg_probe_duration_ms (EWMA)
                 - latency_p95 = avg_probe_duration_ms * 1.5 (heuristic)
```

A dedicated `Evaluation` struct on `Skill` (`lib.rs:356`) holds per-skill
scenario definitions, but it is currently **unwired**. Wiring plan:

1. When `Dispatcher::dispatch()` executes a probe, check if the skill has `Evaluation` entries
2. After probe dispatch, run each `EvalCheck` as a sub-process
3. Record eval results via `RegistryCache::record_execution()` (modulated by pass/fail)
4. On next `SkillHealth::compute()`, the EWMA reflects scenario test outcomes

---

## Calibration Notes

The 6-dimension quality score weights are **heuristic** (Task 10 §6). Current weights:

```
manifest:    0.20
probes:      0.25  ← highest weight (actionable skills must have probes)
interventions: 0.20
rollback:    0.15
scripts:     0.10
docs:        0.10
```

These were chosen to prioritize operational readiness (probes + interventions) over
cosmetic quality (docs + scripts). No empirical calibration data exists yet. The
`okh.skill.eval.complete` span enables Prometheus/Grafana dashboards to surface
distributions of each dimension across the skill fleet — providing the data needed
for calibration.

### Weight Adjustment via Scenario Tester

Proposed feedback loop:

1. For each skill, correlate its `quality_score` with its `reliability` over time
2. If `quality_score` predicts `reliability` poorly (R² < 0.5), re-weight dimensions
3. Run a linear regression: `reliability ~ w₁(manifest) + w₂(probes) + ...`
4. Adjust weights to maximize predictive power of `reliability`
5. This is deferred until enough telemetry exists (est. 30 days of data across 10+ skills)
