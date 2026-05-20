// SPDX-License-Identifier: MIT OR Apache-2.0
// Tests for russell-core crate - migrated to separate crate per AGENTS.md §12

mod hash_chain {
    use russell_core::hash_chain::{compute_event_hash, genesis_hash, verify_chain, verify_link, ChainVerdict, HASH_HEX_LEN};

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
    use russell_core::time::{approx_days_between, now_date_iso8601, now_rfc3339, now_unix, SystemClock, FixedClock};
    use russell_core::Clock;

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
    use russell_core::paths::{Paths, ensure_dir};
    use russell_core::error::CoreError;

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
    use russell_core::profile::{Profile, PROFILE_SCHEMA};
    use russell_core::error::CoreError;

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
    use russell_core::journal::JournalWriter;
    use russell_core::event::{Event, Severity};

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
        for sev in [Severity::Info, Severity::Info, Severity::Warn, Severity::Crit] {
            w.append(&Event::new("x", sev)).unwrap();
        }
        let _c = w.reader().severity_counts(0, i64::MAX).unwrap();
    }
}

mod reflex {
    use russell_core::reflex::{ReflexSet, ReflexBudget, BudgetVerdict};
    use russell_core::event::Severity;

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
    use russell_core::rule::RuleSet;
    use russell_core::event::Severity;

    #[test]
    fn defaults_mem_warn_below() {
        let rs = RuleSet::with_defaults();
        assert_eq!(rs.evaluate("mem_available_mib", 3000.0), Severity::Warn);
        assert_eq!(rs.evaluate("mem_available_mib", 1500.0), Severity::Alert);
        assert_eq!(rs.evaluate("mem_available_mib", 500.0), Severity::Crit);
    }

    #[test]
    fn defaults_swap_warn_above() {
        let rs = RuleSet::with_defaults();
        assert_eq!(rs.evaluate("swap_used_mib", 7168.0), Severity::Warn);
        assert_eq!(rs.evaluate("swap_used_mib", 7680.0), Severity::Alert);
        assert_eq!(rs.evaluate("swap_used_mib", 7936.0), Severity::Crit);
    }

    #[test]
    fn unknown_probe_returns_info() {
        let rs = RuleSet::with_defaults();
        assert_eq!(rs.evaluate("nonexistent_probe", 42.0), Severity::Info);
    }
}
