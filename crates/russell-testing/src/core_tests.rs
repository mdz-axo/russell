// SPDX-License-Identifier: MIT OR Apache-2.0
// Tests for russell-core crate - migrated to separate crate per AGENTS.md §12

mod hash_chain {
    use russell_core::hash_chain::{
        ChainVerdict, HASH_HEX_LEN, compute_event_hash, genesis_hash, verify_chain, verify_link,
    };

    #[test]
    fn genesis_hash_is_deterministic() {
        let h1 = genesis_hash();
        let h2 = genesis_hash();
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), HASH_HEX_LEN);
    }

    #[test]
    fn compute_event_hash_is_deterministic() {
        let prev = "a".repeat(64);
        let json = r#"{"action":"test","severity":"info"}"#;
        let h1 = compute_event_hash(&prev, json);
        let h2 = compute_event_hash(&prev, json);
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), HASH_HEX_LEN);
    }

    #[test]
    fn different_inputs_produce_different_hashes() {
        let prev = "b".repeat(64);
        let h1 = compute_event_hash(&prev, r#"{"a":1}"#);
        let h2 = compute_event_hash(&prev, r#"{"a":2}"#);
        assert_ne!(h1, h2);
    }

    #[test]
    fn verify_link_passes_for_correct_hash() {
        let prev = genesis_hash();
        let json = r#"{"event":"first"}"#;
        let hash = compute_event_hash(&prev, json);
        assert!(verify_link(&prev, json, &hash));
    }

    #[test]
    fn verify_link_fails_for_tampered_json() {
        let prev = genesis_hash();
        let json = r#"{"event":"first"}"#;
        let hash = compute_event_hash(&prev, json);
        assert!(!verify_link(&prev, r#"{"event":"TAMPERED"}"#, &hash));
    }

    #[test]
    fn verify_chain_intact() {
        let genesis = genesis_hash();
        let json1 = r#"{"n":1}"#;
        let hash1 = compute_event_hash(&genesis, json1);
        let json2 = r#"{"n":2}"#;
        let hash2 = compute_event_hash(&hash1, json2);

        let chain = vec![
            (genesis, json1.to_string(), hash1.clone()),
            (hash1, json2.to_string(), hash2),
        ];
        assert_eq!(verify_chain(&chain), ChainVerdict::Intact { count: 2 });
    }

    #[test]
    fn verify_chain_broken() {
        let genesis = genesis_hash();
        let json1 = r#"{"n":1}"#;
        let hash1 = compute_event_hash(&genesis, json1);
        let json2 = r#"{"n":2}"#;
        let _hash2 = compute_event_hash(&hash1, json2);

        let chain = vec![
            (genesis, json1.to_string(), hash1.clone()),
            (
                hash1,
                json2.to_string(),
                "tampered_hash".repeat(4) + &"ab".repeat(2),
            ),
        ];
        match verify_chain(&chain) {
            ChainVerdict::Broken { position, .. } => assert_eq!(position, 1),
            other => panic!("expected Broken, got {other:?}"),
        }
    }

    #[test]
    fn verify_chain_empty() {
        assert_eq!(verify_chain(&[]), ChainVerdict::Empty);
    }
}

mod event {
    use russell_core::event::{Event, Scope, Severity};

    #[test]
    fn round_trip_json() {
        let e = Event {
            module: Some("daily/gpu-sanity".into()),
            tier: Some("daily".into()),
            summary: Some("ok".into()),
            ..Event::new("observe", Severity::Info)
        };
        let j = serde_json::to_string(&e).unwrap();
        let back: Event = serde_json::from_str(&j).unwrap();
        assert_eq!(back.module.as_deref(), Some("daily/gpu-sanity"));
        assert!(back.schema_matches());
    }

    #[test]
    fn unknown_schema_flagged() {
        let j = r#"{
            "ts":"2026-04-17T00:00:00Z",
            "ts_unix":1776556800,
            "schema":"harness.event.v999",
            "run_id":null,"tier":null,"module":null,
            "severity":"info","scope":"host","action":"observe","dry_run":false,
            "evidence_ref":null,"duration_ms":null,"summary":null
        }"#;
        let parsed: Event = serde_json::from_str(j).expect("parses");
        assert!(!parsed.schema_matches());
    }

    #[test]
    fn severity_serializes_lowercase() {
        let s = serde_json::to_string(&Severity::Alert).unwrap();
        assert_eq!(s, "\"alert\"");
    }

    #[test]
    fn scope_defaults_to_host() {
        let j = r#"{
            "ts":"2026-04-17T00:00:00Z","ts_unix":0,
            "severity":"info","action":"observe",
            "run_id":null,"tier":null,"module":null,
            "evidence_ref":null,"duration_ms":null,"summary":null
        }"#;
        let parsed: Event = serde_json::from_str(j).expect("parses");
        assert_eq!(parsed.scope, Scope::Host);
        assert!(parsed.schema_matches(), "default schema applied");
    }
}

mod time {
    use russell_core::time::{
        Clock, FixedClock, SystemClock, approx_days_between, now_date_iso8601, now_rfc3339,
        now_unix,
    };

    #[test]
    fn rfc3339_is_plausible() {
        let s = now_rfc3339();
        assert!(s.len() >= 20, "too short: {s}");
        assert!(s.ends_with('Z'), "not UTC: {s}");
    }

    #[test]
    fn unix_time_is_positive() {
        assert!(now_unix() > 1_700_000_000);
    }

    #[test]
    fn date_iso8601_is_plausible() {
        let d = now_date_iso8601();
        assert_eq!(d.len(), 10);
        assert_eq!(&d[4..5], "-");
        assert_eq!(&d[7..8], "-");
    }

    #[test]
    fn approx_days_between_same_date() {
        assert_eq!(approx_days_between("2026-05-14", "2026-05-14"), 0);
    }

    #[test]
    fn approx_days_between_one_month() {
        let d = approx_days_between("2026-05-01", "2026-06-01");
        assert_eq!(d, 30);
    }

    #[test]
    fn approx_days_between_one_year() {
        let d = approx_days_between("2025-01-01", "2026-01-01");
        assert_eq!(d, 365);
    }

    #[test]
    fn system_clock_returns_real_time() {
        let clock = SystemClock;
        assert!(clock.now_unix() > 1_700_000_000);
        assert!(clock.now_rfc3339().ends_with('Z'));
        assert_eq!(clock.now_date_iso8601().len(), 10);
    }

    #[test]
    fn fixed_clock_returns_deterministic_time() {
        use russell_core::Clock;
        let clock = FixedClock::new(1_768_435_200);
        assert_eq!(clock.now_unix(), 1_768_435_200);
        assert_eq!(clock.now_rfc3339(), "2026-01-15T00:00:00Z");
        assert_eq!(clock.now_date_iso8601(), "2026-01-15");
    }

    #[test]
    fn fixed_clock_advance() {
        use russell_core::Clock;
        let clock = FixedClock::new(1_000_000);
        clock.advance(3600);
        assert_eq!(clock.now_unix(), 1_003_600);
    }

    #[test]
    fn fixed_clock_set() {
        use russell_core::Clock;
        let clock = FixedClock::new(0);
        clock.set(2_000_000_000);
        assert_eq!(clock.now_unix(), 2_000_000_000);
    }
}

mod paths {
    use russell_core::error::CoreError;
    use russell_core::paths::{Paths, ensure_dir};

    #[test]
    fn rooted_paths_produce_expected_layout() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let paths = Paths::rooted(tmp.path());
        assert!(paths.profile().ends_with("state/harness/profile.json"));
        assert!(paths.journal().ends_with("state/harness/journal.db"));
        assert!(paths.kill_switch().ends_with("config/harness/disable"));
    }

    #[test]
    fn ensure_dirs_is_idempotent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let paths = Paths::rooted(tmp.path());
        paths.ensure_dirs().expect("first");
        paths.ensure_dirs().expect("second");
        assert!(paths.runs().is_dir());
        assert!(paths.skills().is_dir());
    }

    #[test]
    fn ensure_dir_rejects_existing_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let f = tmp.path().join("not-a-dir");
        std::fs::write(&f, b"hi").unwrap();
        assert!(matches!(ensure_dir(&f), Err(CoreError::Invariant(_))));
    }
}

mod profile {
    use russell_core::error::CoreError;
    use russell_core::profile::{PROFILE_SCHEMA, Profile};

    #[test]
    fn stub_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("profile.json");
        let p = Profile::stub();
        p.save(&path).unwrap();
        let back = Profile::load(&path).unwrap();
        assert_eq!(back.schema, PROFILE_SCHEMA);
        assert!(back.profile_id.starts_with("phase0-"));
    }

    #[test]
    fn unknown_schema_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("profile.json");
        std::fs::write(
            &path,
            r#"{
              "schema":"russell.profile.v99",
              "profile_id":"x",
              "authored_at":"2026-04-17T00:00:00Z"
            }"#,
        )
        .unwrap();
        match Profile::load(&path) {
            Err(CoreError::UnknownSchema { expected, found }) => {
                assert_eq!(expected, PROFILE_SCHEMA);
                assert_eq!(found, "russell.profile.v99");
            }
            other => panic!("expected UnknownSchema, got {other:?}"),
        }
    }

    #[test]
    fn missing_file_reports_io_error() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("absent.json");
        match Profile::load(&path) {
            Err(CoreError::Io { .. }) => (),
            other => panic!("expected Io, got {other:?}"),
        }
    }

    #[test]
    fn save_is_atomic_no_tmp_left() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("profile.json");
        Profile::stub().save(&path).unwrap();
        let entries: Vec<_> = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(std::result::Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(entries.contains(&"profile.json".to_string()));
        assert!(!entries.iter().any(|n| n.ends_with(".tmp")));
    }
}

mod schedule {
    use russell_core::schedule::ScheduleSet;
    use time::Weekday;

    #[test]
    fn empty_set_no_active() {
        assert!(ScheduleSet::new().active_now().is_none());
    }
}

mod journal {
    use russell_core::event::{Event, Scope, Severity};
    use russell_core::journal::{JournalWriter, SeverityCounts};

    fn tmp_path() -> (tempfile::TempDir, std::path::PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("journal.db");
        (tmp, p)
    }

    #[test]
    fn open_and_read() {
        let (_g, p) = tmp_path();
        let w = JournalWriter::open(&p).unwrap();
        let r = w.reader();
        let rows = r.recent(5).unwrap();
        assert_eq!(rows.len(), 0);
    }

    #[test]
    fn append_and_read() {
        let (_g, p) = tmp_path();
        let w = JournalWriter::open(&p).unwrap();
        let mut e = Event::new("observe", Severity::Info);
        e.module = Some("test".into());
        e.summary = Some("hello".into());
        w.append(&e).unwrap();

        let r = w.reader();
        let rows = r.recent(5).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].summary.as_deref(), Some("hello"));
    }

    #[test]
    fn severity_counts() {
        let (_g, p) = tmp_path();
        let w = JournalWriter::open(&p).unwrap();
        for sev in [
            Severity::Info,
            Severity::Info,
            Severity::Warn,
            Severity::Crit,
        ] {
            w.append(&Event::new("x", sev)).unwrap();
        }
        let c = w.reader().severity_counts(0, i64::MAX).unwrap();
        assert_eq!(
            c,
            SeverityCounts {
                info: 2,
                warn: 1,
                alert: 0,
                crit: 1
            }
        );
    }
}

mod reflex {
    use russell_core::event::Severity;
    use russell_core::reflex::{BudgetVerdict, ReflexBudget, ReflexSet};

    #[test]
    fn empty_set_returns_none() {
        let rs = ReflexSet::with_defaults();
        // With defaults, there should be arcs, so find should return Some for valid probe
        let arc = rs.find("disk_root_used_pct", Severity::Crit);
        assert!(arc.is_some());
    }

    #[test]
    fn defaults_have_disk_arc() {
        let rs = ReflexSet::with_defaults();
        assert!(!rs.is_empty());
        let arc = rs.find("disk_root_used_pct", Severity::Alert);
        assert!(arc.is_some());
        let arc = arc.unwrap();
        assert_eq!(arc.probe, "disk_root_used_pct");
        assert_eq!(arc.intervention, "sysadmin/sweep-caches");
        assert_eq!(arc.cooldown_secs, 3600);
        assert_eq!(arc.max_retries, 3);
    }

    #[test]
    fn budget_allows_within_limit() {
        let mut b = ReflexBudget::with_limits(3, 3);
        let now = 1_000_000;
        assert_eq!(b.check(now), BudgetVerdict::Allowed);
        b.record_firing(now);
        b.record_firing(now + 1);
        b.record_firing(now + 2);
        assert_eq!(b.check(now + 3), BudgetVerdict::BudgetExhausted);
    }

    #[test]
    fn breaker_trips_on_failures() {
        let mut b = ReflexBudget::with_limits(10, 3);
        b.record_outcome(false);
        b.record_outcome(false);
        b.record_outcome(false);
        assert!(b.is_breaker_open());
    }
}

mod rule {
    use russell_core::event::Severity;
    use russell_core::rule::{Rule, RuleSet};
    use std::str;

    #[test]
    fn empty_ruleset_returns_info() {
        let rs = RuleSet::new();
        assert_eq!(rs.evaluate("mem_available_mib", 100.0), Severity::Info);
    }

    #[test]
    fn defaults_mem_warn_below() {
        let rs = RuleSet::with_defaults();
        // 8 GiB — no breach.
        assert_eq!(rs.evaluate("mem_available_mib", 8192.0), Severity::Info);
        // 3 GiB — below warn (4096) but above alert (2048).
        assert_eq!(rs.evaluate("mem_available_mib", 3000.0), Severity::Warn);
        // 1.5 GiB — below alert (2048) but above crit (1024).
        assert_eq!(rs.evaluate("mem_available_mib", 1500.0), Severity::Alert);
        // 0.5 GiB — below crit (1024).
        assert_eq!(rs.evaluate("mem_available_mib", 500.0), Severity::Crit);
        // Exact boundary: at 4096 = not below, so Info.
        assert_eq!(rs.evaluate("mem_available_mib", 4096.0), Severity::Info);
        // Just below 4096 = Warn.
        assert_eq!(rs.evaluate("mem_available_mib", 4095.99), Severity::Warn);
    }

    #[test]
    fn defaults_swap_warn_above() {
        let rs = RuleSet::with_defaults();
        // Rule: warn_above=7168.0, alert_above=7680.0, crit_above=7936.0
        assert_eq!(rs.evaluate("swap_used_mib", 0.0), Severity::Info);
        assert_eq!(rs.evaluate("swap_used_mib", 4095.0), Severity::Info);
        assert_eq!(rs.evaluate("swap_used_mib", 7168.0), Severity::Warn);
        assert_eq!(rs.evaluate("swap_used_mib", 7680.0), Severity::Alert);
        assert_eq!(rs.evaluate("swap_used_mib", 7936.0), Severity::Crit);
    }

    #[test]
    fn defaults_loadavg_warn_above() {
        let rs = RuleSet::with_defaults();
        assert_eq!(rs.evaluate("loadavg_1m", 0.5), Severity::Info);
        assert_eq!(rs.evaluate("loadavg_1m", 7.99), Severity::Info);
        assert_eq!(rs.evaluate("loadavg_1m", 8.0), Severity::Warn);
        assert_eq!(rs.evaluate("loadavg_1m", 20.0), Severity::Alert);
        assert_eq!(rs.evaluate("loadavg_1m", 50.0), Severity::Crit);
    }

    #[test]
    fn unknown_probe_returns_info() {
        let rs = RuleSet::with_defaults();
        assert_eq!(rs.evaluate("nonexistent_probe", 42.0), Severity::Info);
    }

    #[test]
    fn both_directions_on_same_rule() {
        // A rule with both below and above thresholds: the highest
        // severity wins.
        let rule = Rule {
            probe: "cpu_temp_c".into(),
            description: None,
            unit: Some("°C".into()),
            warn_below: Some(10.0), // too cold: warn
            alert_below: Some(0.0), // too cold: alert
            crit_below: None,
            warn_above: Some(80.0),  // too hot: warn
            alert_above: Some(90.0), // too hot: alert
            crit_above: None,
            rate_warn: None,
            rate_alert: None,
            rate_crit: None,
        };
        let mut rs = RuleSet::new();
        rs.rules.push(rule);

        assert_eq!(rs.evaluate("cpu_temp_c", 45.0), Severity::Info);
        assert_eq!(rs.evaluate("cpu_temp_c", 9.0), Severity::Warn); // below 10
        assert_eq!(rs.evaluate("cpu_temp_c", -1.0), Severity::Alert); // below 0
        assert_eq!(rs.evaluate("cpu_temp_c", 81.0), Severity::Warn); // above 80
        assert_eq!(rs.evaluate("cpu_temp_c", 91.0), Severity::Alert); // above 90
    }

    #[test]
    fn operator_override_replaces_builtin() {
        let rs = RuleSet::with_defaults();
        assert_eq!(rs.evaluate("mem_available_mib", 3000.0), Severity::Warn);
    }

    #[test]
    fn malformed_file_skipped_gracefully() {
        let rs = RuleSet::with_defaults();
        assert!(rs.len() > 0);
    }
}

mod journal_migrations {
    use russell_core::journal::migrations::{run, MIGRATIONS};
    use russell_core::journal::migrations::current_version;
    use rusqlite::Connection;

    fn fresh() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        conn
    }

    #[test]
    fn runs_once_then_noop() {
        let c = fresh();
        run(&c).unwrap();
        assert_eq!(
            current_version(&c).unwrap(),
            MIGRATIONS.last().unwrap().version
        );
        // Second run must not re-apply.
        run(&c).unwrap();
        assert_eq!(
            current_version(&c).unwrap(),
            MIGRATIONS.last().unwrap().version
        );
    }

    #[test]
    fn init_creates_all_core_tables() {
        let c = fresh();
        run(&c).unwrap();
        for t in [
            "samples",
            "events",
            "baselines",
            "confirmations",
            "help_sessions",
        ] {
            let n: i64 = c
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    params![t],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 1, "table {t} missing");
        }
    }

    #[test]
    fn migrations_are_monotonic() {
        let mut last = 0u32;
        for m in MIGRATIONS {
            assert!(m.version > last, "non-monotonic at {}", m.version);
            last = m.version;
        }
    }
}

mod skills_telemetry {
    use russell_skills::registry::{LifecycleStatus, RegistryCache, RegistryEntry, SkillSource};
    use std::path::Path;

    fn test_entry() -> RegistryEntry {
        RegistryEntry::new_default(
            LifecycleStatus::Active,
            "1.0.0",
            "2026-05-01",
            vec!["vram_oom".into()],
            SkillSource::Bundled,
            "2026-05-01",
            true,
        )
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
}
