// SPDX-License-Identifier: MIT OR Apache-2.0
//! Integration test: verify the full telemetry pipeline — registry cache
//! mutation, persistence, score computation, and freshness tracking.

use russell_skills::registry::{
    LifecycleStatus, RegistryCache, RegistryEntry, SkillSource, TrustTier,
};
use std::path::Path;

fn test_entry() -> RegistryEntry {
    RegistryEntry {
        status: LifecycleStatus::Active,
        version: "1.0.0".into(),
        symptoms: vec!["vram_oom".into()],
        source: SkillSource::Bundled,
        trust_tier: TrustTier::T4,
        installed: "2026-05-01".into(),
        last_evaluated: None,
        valid_until: None,
        coverage_score: None,
        superseded_by: None,
        deprecation_reason: None,
        probe_runs: 0,
        recent_probe_failures: 0,
        intervention_runs: 0,
        recent_intervention_failures: 0,
        last_probe_run_at: None,
        last_error: None,
        avg_probe_duration_ms: None,
        bundled: true,
    }
}

#[test]
fn full_telemetry_pipeline() {
    let tmp = tempfile::tempdir().unwrap();
    let cache_path: &Path = &tmp.path().join("local-cache.yaml");

    // 1. Create cache with one skill.
    RegistryCache::with_update(cache_path, |cache| {
        cache.upsert("test-skill", test_entry());
    })
    .unwrap();

    // 2. Record several probe executions with mixed success.
    for (success, duration, err) in [
        (true, 45, None::<&str>),
        (true, 52, None),
        (false, 230, Some("connection refused")),
        (true, 48, None),
        (false, 500, Some("timeout after 5s")),
    ] {
        RegistryCache::with_update(cache_path, |cache| {
            cache.record_execution("test-skill", success, duration, err);
        })
        .unwrap();
    }

    // 3. Verify counters.
    let cache = RegistryCache::load(cache_path).unwrap();
    let entry = cache.skills.get("test-skill").unwrap();
    assert_eq!(entry.probe_runs, 5, "5 probes should have been recorded");
    assert_eq!(entry.recent_probe_failures, 2, "2 failures expected");
    assert!(entry.last_probe_run_at.is_some(), "timestamp should be set");
    assert_eq!(
        entry.last_error.as_deref(),
        Some("timeout after 5s"),
        "last error should be the most recent"
    );
    assert!(
        entry.avg_probe_duration_ms.is_some(),
        "EWMA should be computed"
    );

    // 4. Record interventions.
    RegistryCache::with_update(cache_path, |cache| {
        cache.record_intervention("test-skill", true, None);
        cache.record_intervention("test-skill", false, Some("rollback failed"));
    })
    .unwrap();

    let cache = RegistryCache::load(cache_path).unwrap();
    let entry = cache.skills.get("test-skill").unwrap();
    assert_eq!(entry.intervention_runs, 2);
    assert_eq!(entry.recent_intervention_failures, 1);
    assert_eq!(entry.last_error.as_deref(), Some("rollback failed"));

    // 5. Verify EWMA is sane (should be between min and max durations).
    let ewma = entry.avg_probe_duration_ms.unwrap();
    assert!(
        (45.0..=500.0).contains(&ewma),
        "EWMA {ewma} should be in [45, 500]"
    );
}

#[test]
fn freshness_and_scoring_integration() {
    let mut entry = test_entry();
    entry.probe_runs = 100;
    entry.recent_probe_failures = 0;
    assert!((RegistryCache::freshness_score(&entry) - 1.0).abs() < f64::EPSILON);

    entry.recent_probe_failures = 20;
    assert!(
        (RegistryCache::freshness_score(&entry) - 0.8).abs() < f64::EPSILON,
        "20% failures should give freshness 0.8"
    );
}

#[test]
fn score_full_skill_with_section_scoped_parsing() {
    let entry = test_entry();
    // Manifest with probes and interventions having section-scoped entries.
    let manifest = "\
id: my-skill
version: 0.2.0
authored: 2026-05-14
# This is a comment that should not affect scoring
symptoms:
  - vram_oom
  - gpu_hang
probes:
  - id: check-vram
    cmd:
      - nvidia-smi
      - --query-gpu=memory.used
    timeout: 5s
  - id: check-temp
    cmd:
      - nvidia-smi
      - --query-gpu=temperature
    timeout: 5s
interventions:
  - id: restart-gpu
    cmd:
      - systemctl
      - restart
      - nvidia-persistenced
    rollback: none_needed
  - id: kill-oom
    cmd:
      - pkill
      - -9
      - oom_killer
    rollback: reboot
recovery:
  documentation: extra section that should not affect score
";
    let score = RegistryCache::compute_score(&entry, manifest, true);
    // Full skill: 2 probes, 2 interventions, rollback present, docs present.
    assert!(
        score > 0.8,
        "full skill with section-scoped entries should score > 0.8, got {score}"
    );
}

#[test]
fn score_not_fooled_by_comments() {
    let entry = test_entry();
    // Malicious manifest: probes section is empty but comments mention "id:".
    let manifest = "\
id: skeleton
version: 0.1.0
authored: 2026-05-14
# The probes section is empty but we have - id: in a comment
# Don't be fooled by documentation mentioning version: 2.0
symptoms: []
probes: []
interventions: []
";
    let score = RegistryCache::compute_score(&entry, manifest, false);
    assert!(
        score < 0.5,
        "comments should not inflate score, got {score}"
    );
}
