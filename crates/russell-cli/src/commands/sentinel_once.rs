// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell sentinel-once` — fire the Sentinel once.
//!
//! Runs the proprioception self-vital (JR-5) FIRST — measuring
//! how stale the previous cycle's data is — then the host probe
//! set, then emits a cycle event summarising both.
//!
//! The ordering matters: proprio must read `MAX(ts)` from host
//! samples *before* the current cycle writes new ones, otherwise
//! the age is always ~0 s and the self-vital can never trigger.

use anyhow::{Context, Result};
use russell_core::{BudgetVerdict, ReflexBudget, ReflexSet};
use russell_core::RuleSet;
use russell_core::event::{Event, Scope, Severity};
use russell_core::journal::{JournalReader, JournalWriter};
use russell_core::paths::Paths;

pub fn run(paths: &Paths) -> Result<()> {
    let started = std::time::Instant::now();
    let journal = JournalWriter::open(&paths.journal())
        .with_context(|| format!("opening journal {}", paths.journal().display()))?;

    // Load rule set: defaults + operator overrides from rules.d/.
    let mut rules = RuleSet::with_defaults();
    rules.load_from_dir(&paths.rules());

    // Load reflex arcs: defaults + operator overrides from reflex.d/.
    let mut reflexes = ReflexSet::with_defaults();
    reflexes.load_from_dir(&paths.reflex());

    // 1. Proprioception: self-vital (JR-5).
    //    Must run BEFORE host probes so it measures the age of the
    //    *previous* cycle's samples, not the ones we're about to write.
    let reader = journal.reader();
    let proprio = russell_proprio::run_once(&journal, &reader).context("running proprioception")?;

    // 2. Host probes with rule evaluation (absolute + rate-of-change).
    let (n, threshold_events) =
        russell_sentinel::run_once_with_rules(&journal, &rules, Some(&reader))
            .context("running Sentinel")?;

    // 2b. Evaluate externally-written scenario metrics (from scenario-tester skill).
    //     Scenario metrics like okapi_latency_p95_ms are journaled as samples but
    //     not collected by the sentinel's ProbeRegistry. Re-read and evaluate them.
    let scenario_events = russell_sentinel::evaluate_scenario_samples(
        &reader,
        &rules,
        3600, // last hour — scenario metrics may be written outside the sentinel cycle
        &threshold_events,
    );
    let total_breaches = threshold_events.len() + scenario_events.len();

    // Write all threshold breach events.
    for ev in &threshold_events {
        journal.append(ev)?;
    }
    for ev in &scenario_events {
        journal.append(ev)?;
    }

    // 3. Cycle event.
    let mut ev = Event::new("observe", Severity::Info);
    ev.tier = Some("sentinel".into());
    ev.module = Some("sentinel/cycle".into());
    ev.summary = Some(format!(
        "captured {n} host samples, {} threshold breaches; proprio: age={}s stall={}s llm_p95={}ms drift={}s err_rate={}%",
        total_breaches,
        proprio
            .age_s
            .map(|a| a.to_string())
            .unwrap_or_else(|| "none".into()),
        proprio
            .journal_stall_s
            .map(|s| s.to_string())
            .unwrap_or_else(|| "?".into()),
        proprio
            .llm_p95_latency_ms
            .map(|v| format!("{v:.0}"))
            .unwrap_or_else(|| "?".into()),
        proprio
            .timer_drift_s
            .map(|d| d.to_string())
            .unwrap_or_else(|| "?".into()),
        proprio
            .help_error_rate_pct
            .map(|v| format!("{v:.1}"))
            .unwrap_or_else(|| "?".into()),
    ));
    ev.duration_ms = Some(started.elapsed().as_millis() as u64);
    ev.outputs
        .insert("sample_count".into(), serde_json::Value::from(n as u64));
    ev.outputs.insert(
        "proprio_age_s".into(),
        match proprio.age_s {
            Some(a) => serde_json::Value::from(a),
            None => serde_json::Value::Null,
        },
    );
    ev.outputs.insert(
        "proprio_severity".into(),
        serde_json::Value::from(format!("{:?}", proprio.severity)),
    );
    if let Some(s) = proprio.journal_stall_s {
        ev.outputs
            .insert("journal_stall_s".into(), serde_json::Value::from(s));
    }
    if let Some(v) = proprio.llm_p95_latency_ms {
        ev.outputs
            .insert("llm_p95_latency_ms".into(), serde_json::Value::from(v));
    }
    if let Some(d) = proprio.timer_drift_s {
        ev.outputs
            .insert("timer_drift_s".into(), serde_json::Value::from(d));
    }
    if let Some(p) = proprio.help_error_rate_pct {
        ev.outputs.insert(
            "help_error_rate_pct".into(),
            serde_json::Value::from((p * 10.0).round() / 10.0),
        );
    }
    journal.append(&ev)?;

    // 4. Reflex arc check — for each threshold breach, find matching
    //    arcs and emit reflex_proposed events. These events are consumed
    //    by the Nurse (russell jack / chat) which may auto-execute
    //    risk:low interventions or propose risk:medium/+ for consent.
    //
    //    Cooldown: skip arcs that fired for the same probe within the
    //    arc's cooldown window (prevents oscillation).
    //
    //    Budget (T10): Global cap prevents cascading intervention storms.
    //    Circuit breaker halts all reflexes after 3 consecutive failures.
    let now = russell_core::time::now_unix();
    let mut budget = ReflexBudget::new();

    for ev in threshold_events.iter().chain(scenario_events.iter()) {
        let Some(probe) = ev.outputs.get("probe").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(arc) = reflexes.find(probe, ev.severity) else {
            continue;
        };

        // Budget gate: check global reflex budget before per-arc cooldown.
        match budget.check(now) {
            BudgetVerdict::Allowed => {}
            BudgetVerdict::BudgetExhausted => {
                tracing::warn!(
                    probe,
                    "reflex budget exhausted — skipping all remaining arcs"
                );
                let mut budget_ev = Event::new("reflex_budget_exhausted", Severity::Warn);
                budget_ev.tier = Some("sentinel".into());
                budget_ev.module = Some("reflex/budget".into());
                budget_ev.summary = Some(format!(
                    "reflex budget exhausted ({} firings this hour) — escalating to operator",
                    budget.firings_this_hour()
                ));
                journal.append(&budget_ev)?;
                break;
            }
            BudgetVerdict::BreakerOpen => {
                tracing::warn!("reflex circuit breaker OPEN — all reflexes halted");
                let mut breaker_ev = Event::new("reflex_breaker_open", Severity::Alert);
                breaker_ev.tier = Some("sentinel".into());
                breaker_ev.module = Some("reflex/breaker".into());
                breaker_ev.summary = Some(
                    "circuit breaker tripped — consecutive failures exceeded threshold".into(),
                );
                journal.append(&breaker_ev)?;
                break;
            }
        }

        // Check cooldown: has this arc fired recently for this probe?
        let cooldown_since = now - arc.cooldown_secs;
        let recent_for_probe = reader
            .count_reflex_events(probe, cooldown_since, now)
            .unwrap_or(0);

        if recent_for_probe > 0 {
            tracing::debug!(
                probe,
                cooldown_secs = arc.cooldown_secs,
                "reflex arc in cooldown — skipping"
            );
            continue;
        }

        // Record firing in budget.
        budget.record_firing(now);

        let mut ref_ev = Event::new("reflex_proposed", ev.severity);
        ref_ev.tier = Some("sentinel".into());
        ref_ev.module = Some(format!("reflex/{}", probe));
        ref_ev.summary = Some(format!(
            "probe {} at {:?} → proposed intervention {}",
            probe, ev.severity, arc.intervention
        ));
        ref_ev.outputs.insert("probe".into(), probe.into());
        ref_ev
            .outputs
            .insert("severity".into(), format!("{:?}", ev.severity).into());
        ref_ev
            .outputs
            .insert("intervention".into(), arc.intervention.clone().into());
        ref_ev
            .outputs
            .insert("cooldown_secs".into(), arc.cooldown_secs.into());
        ref_ev
            .outputs
            .insert("max_retries".into(), arc.max_retries.into());

        journal.append(&ref_ev)?;
    }

    // 5. Refresh EWMA baselines once per 24h.
    maybe_refresh_baselines(&journal, &reader);

    // 6. Reconcile skill registry against disk.
    reconcile_registry(paths);

    println!(
        "sentinel: captured {n} samples, {} threshold breaches in {} ms; proprio: age={}s stall={}s llm_p95={}ms drift={}s err_rate={}%",
        total_breaches,
        started.elapsed().as_millis(),
        proprio
            .age_s
            .map(|a| a.to_string())
            .unwrap_or_else(|| "n/a".into()),
        proprio
            .journal_stall_s
            .map(|s| s.to_string())
            .unwrap_or_else(|| "?".into()),
        proprio
            .llm_p95_latency_ms
            .map(|v| format!("{v:.0}"))
            .unwrap_or_else(|| "?".into()),
        proprio
            .timer_drift_s
            .map(|d| d.to_string())
            .unwrap_or_else(|| "?".into()),
        proprio
            .help_error_rate_pct
            .map(|v| format!("{v:.1}"))
            .unwrap_or_else(|| "?".into()),
    );
    Ok(())
}

/// Reconcile the skill registry cache against disk-loaded skills.
/// Removes orphan entries, adds missing ones, updates stale metadata.
/// Cheap enough to run every sentinel cycle (5 min).
fn reconcile_registry(paths: &Paths) {
    let skills = russell_skills::load_all(&paths.skills()).unwrap_or_default();
    let registry_path = paths.state.join("registry").join("local-cache.yaml");
    let mut registry = russell_skills::registry::RegistryCache::load(&registry_path)
        .unwrap_or_default();
    if registry.reconcile(&skills) {
        if let Err(e) = registry.save(&registry_path) {
            tracing::warn!(error = %e, "registry reconcile save failed");
        } else {
            tracing::debug!("registry reconciled");
        }
    }
}

/// Compute and persist EWMA baselines if 24h have elapsed since
/// the last computation. Runs inline (not async) — the cost is
/// acceptable once per day.
#[tracing::instrument(level = "info", skip_all)]
fn maybe_refresh_baselines(journal: &JournalWriter, reader: &JournalReader) {
    let now = russell_core::time::now_unix();

    // Check if any baseline is older than 24h.
    let needs_refresh = reader
        .open_ro_conn()
        .ok()
        .and_then(|conn| {
            conn.query_row("SELECT MAX(updated_ts) FROM baselines", [], |r| {
                r.get::<_, Option<i64>>(0)
            })
            .ok()
            .flatten()
        })
        .is_none_or(|last| now - last > 86_400);

    if !needs_refresh {
        return;
    }

    tracing::info!("refreshing EWMA baselines (30-day window)");
    match reader.compute_baselines(30) {
        Ok(rows) => {
            let count = rows.len();
            for row in &rows {
                if let Err(e) = journal.upsert_baseline(
                    &row.probe,
                    Scope::Host,
                    row.ewma_mean,
                    row.ewma_var,
                    row.p50,
                    row.p95,
                    row.p99,
                ) {
                    tracing::warn!(probe = %row.probe, error = %e, "failed to upsert baseline");
                }
            }
            tracing::info!(probes = count, "baselines refreshed");
        }
        Err(e) => {
            tracing::warn!(error = %e, "failed to compute baselines");
        }
    }
}
