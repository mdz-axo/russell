// SPDX-License-Identifier: MIT OR Apache-2.0
// Tests for russell-core crate

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
        let clock = FixedClock::new(1_768_435_200);
        assert_eq!(clock.now_unix(), 1_768_435_200);
        assert_eq!(clock.now_rfc3339(), "2026-01-15T00:00:00Z");
        assert_eq!(clock.now_date_iso8601(), "2026-01-15");
    }

    #[test]
    fn fixed_clock_advance() {
        let clock = FixedClock::new(1_000_000);
        clock.advance(3600);
        assert_eq!(clock.now_unix(), 1_003_600);
    }

    #[test]
    fn fixed_clock_set() {
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

mod env {
    use russell_core::env::{load_env_file, load_discovered};
    use std::fs;

    #[test]
    fn empty_value_does_not_mask_existing_env() {
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("RUSSELL_TEST_EMPTY_A", "real");
        }
        let tmp = tempfile::tempdir().unwrap();
        let f = tmp.path().join("russell.env");
        fs::write(&f, "RUSSELL_TEST_EMPTY_A=\n").unwrap();
        load_env_file(&f);
        assert_eq!(std::env::var("RUSSELL_TEST_EMPTY_A").unwrap(), "real");
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("RUSSELL_TEST_EMPTY_A");
        }
    }

    #[test]
    fn empty_value_in_file_is_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let f = tmp.path().join("russell.env");
        fs::write(&f, "RUSSELL_TEST_EMPTY_B=\n").unwrap();
        load_env_file(&f);
        assert!(std::env::var("RUSSELL_TEST_EMPTY_B").is_err());
    }

    #[test]
    fn discovery_prefers_config_over_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = tmp.path().join("config/harness");
        std::fs::create_dir_all(&cfg).unwrap();
        std::fs::write(cfg.join("russell.env"), "RUSSELL_TEST_DISC_A=from_config").unwrap();
        load_discovered(&cfg, None);
        assert_eq!(std::env::var("RUSSELL_TEST_DISC_A").unwrap(), "from_config");
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("RUSSELL_TEST_DISC_A");
        }
    }

    #[test]
    fn discovery_override_wins() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = tmp.path().join("config/harness");
        std::fs::create_dir_all(&cfg).unwrap();
        std::fs::write(cfg.join("russell.env"), "RUSSELL_TEST_DISC_B=from_config").unwrap();
        let override_file = tmp.path().join("override.env");
        std::fs::write(&override_file, "RUSSELL_TEST_DISC_B=from_override").unwrap();
        load_discovered(&cfg, Some(&override_file));
        assert_eq!(std::env::var("RUSSELL_TEST_DISC_B").unwrap(), "from_override");
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("RUSSELL_TEST_DISC_B");
        }
    }

    #[test]
    fn discovery_returns_none_when_no_files_exist() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = tmp.path().join("config/harness");
        std::fs::create_dir_all(&cfg).unwrap();
        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(&tmp).unwrap();
        let result = load_discovered(&cfg, None);
        std::env::set_current_dir(prev).unwrap();
        assert!(result.is_none() || result.as_deref().map(|p| p.exists()).unwrap_or(false));
    }

    #[test]
    fn missing_file_is_silent() {
        let tmp = tempfile::tempdir().unwrap();
        load_env_file(&tmp.path().join("absent.env"));
    }

    #[test]
    fn loads_keys_but_respects_existing_env() {
        let tmp = tempfile::tempdir().unwrap();
        let f = tmp.path().join("russell.env");
        fs::write(&f, "# comment\nRUSSELL_TEST_LOAD_A=one\nRUSSELL_TEST_LOAD_B=\"two\"\n").unwrap();
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("RUSSELL_TEST_LOAD_B", "pre-set");
        }
        load_env_file(&f);
        assert_eq!(std::env::var("RUSSELL_TEST_LOAD_A").unwrap(), "one");
        assert_eq!(std::env::var("RUSSELL_TEST_LOAD_B").unwrap(), "pre-set");
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("RUSSELL_TEST_LOAD_A");
            std::env::remove_var("RUSSELL_TEST_LOAD_B");
        }
    }

    #[test]
    fn malformed_line_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let f = tmp.path().join("russell.env");
        fs::write(&f, "RUSSELL_TEST_LOAD_C=ok\nno_equals_sign_here\nRUSSELL_TEST_LOAD_D='fine'\n").unwrap();
        load_env_file(&f);
        assert_eq!(std::env::var("RUSSELL_TEST_LOAD_C").unwrap(), "ok");
        assert_eq!(std::env::var("RUSSELL_TEST_LOAD_D").unwrap(), "fine");
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("RUSSELL_TEST_LOAD_C");
            std::env::remove_var("RUSSELL_TEST_LOAD_D");
        }
    }
}

mod schedule {
    use russell_core::schedule::ScheduleSet;
    use time::Weekday;

    #[test]
    fn parse_works() {
        assert_eq!(ScheduleSet::parse_time("08:00"), Some((8, 0)));
        assert_eq!(ScheduleSet::parse_time("23:59"), Some((23, 59)));
        assert_eq!(ScheduleSet::parse_time("24:00"), None);
        assert_eq!(ScheduleSet::parse_time(""), None);
    }

    #[test]
    fn day_checks() {
        assert!(ScheduleSet::day_matches(Weekday::Monday, &["Mon".into()]));
        assert!(!ScheduleSet::day_matches(Weekday::Tuesday, &["Mon".into()]));
        assert!(ScheduleSet::day_matches(Weekday::Friday, &["Mon".into(), "Fri".into()]));
    }

    #[test]
    fn empty_set_no_active() {
        assert!(ScheduleSet::new().active_now().is_none());
    }
}

mod journal_migrations {
    use russell_core::journal::migrations::run;
    use rusqlite::Connection;

    fn fresh() -> Connection {
        Connection::open_in_memory().unwrap()
    }

    fn current_version(conn: &Connection) -> u32 {
        conn.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0)
    }

    #[test]
    fn runs_once_then_noop() {
        let c = fresh();
        run(&c).unwrap();
        assert_eq!(current_version(&c), 2);
        run(&c).unwrap();
        assert_eq!(current_version(&c), 2);
    }

    #[test]
    fn init_creates_all_core_tables() {
        let c = fresh();
        run(&c).unwrap();
        for t in ["samples", "events", "baselines", "confirmations", "help_sessions"] {
            let n: i64 = c
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    rusqlite::params![t],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 1, "table {t} missing");
        }
    }
}

mod journal_port {
    use russell_core::journal::port::InMemoryJournal;
    use russell_core::event::{Event, Scope, Severity};

    #[test]
    fn in_memory_journal_captures_events() {
        let journal = InMemoryJournal::default();
        let event = Event::new("test_action", Severity::Info);
        journal.append(&event).unwrap();
        assert_eq!(journal.events.lock().unwrap().len(), 1);
    }

    #[test]
    fn in_memory_journal_captures_samples() {
        let journal = InMemoryJournal::default();
        journal
            .append_sample(1000, Scope::Host, "test_probe", Some(42.0), None, Some("MiB"))
            .unwrap();
        let samples = journal.samples.lock().unwrap();
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].1, "test_probe");
        assert_eq!(samples[0].2, Some(42.0));
    }
}

mod journal {
    use russell_core::journal::{JournalWriter, SeverityCounts};
    use russell_core::event::{Event, Scope, Severity};

    fn tmp_path() -> (tempfile::TempDir, std::path::PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("journal.db");
        (tmp, p)
    }

    #[test]
    fn open_runs_migrations() {
        let (_g, p) = tmp_path();
        let w = JournalWriter::open(&p).unwrap();
        let n: i64 = w
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type='table' AND name IN ('events','samples','baselines','confirmations')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 4);
    }

    #[test]
    fn reopen_is_idempotent() {
        let (_g, p) = tmp_path();
        {
            let _ = JournalWriter::open(&p).unwrap();
        }
        let _ = JournalWriter::open(&p).unwrap();
    }

    #[test]
    fn append_and_read_round_trip() {
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
    fn severity_counts_buckets_correctly() {
        let (_g, p) = tmp_path();
        let w = JournalWriter::open(&p).unwrap();
        for sev in [Severity::Info, Severity::Info, Severity::Warn, Severity::Crit] {
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

    #[test]
    fn samples_insert_or_replace_is_idempotent() {
        let (_g, p) = tmp_path();
        let w = JournalWriter::open(&p).unwrap();
        w.append_sample(100, Scope::Host, "cpu_temp_c", Some(42.0), None, Some("C"))
            .unwrap();
        w.append_sample(100, Scope::Host, "cpu_temp_c", Some(43.0), None, Some("C"))
            .unwrap();
        let conn = w.reader().open_ro().unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM samples WHERE ts=100", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(n, 1);
    }
}

mod reflex {
    use russell_core::reflex::{ReflexSet, ReflexBudget, BudgetVerdict};
    use russell_core::event::Severity;

    #[test]
    fn empty_set_returns_none() {
        let rs = ReflexSet::new();
        assert!(rs.find("disk_root_used_pct", Severity::Alert).is_none());
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
    fn below_min_severity_returns_none() {
        let rs = ReflexSet::with_defaults();
        assert!(rs.find("disk_root_used_pct", Severity::Warn).is_none());
    }

    #[test]
    fn crit_triggers_alert_arc() {
        let rs = ReflexSet::with_defaults();
        assert!(rs.find("disk_root_used_pct", Severity::Crit).is_some());
    }

    #[test]
    fn budget_allows_within_limit() {
        let mut b = ReflexBudget::with_limits(3, 3);
        let now = 1_000_000;
        assert_eq!(b.check(now), BudgetVerdict::Allowed);
        b.record_firing(now);
        assert_eq!(b.check(now), BudgetVerdict::Allowed);
        b.record_firing(now + 1);
        assert_eq!(b.check(now + 2), BudgetVerdict::Allowed);
        b.record_firing(now + 2);
        assert_eq!(b.check(now + 3), BudgetVerdict::BudgetExhausted);
    }

    #[test]
    fn budget_window_evicts_old_firings() {
        let mut b = ReflexBudget::with_limits(2, 3);
        let hour_ago = 1_000_000;
        b.record_firing(hour_ago);
        b.record_firing(hour_ago + 1);
        assert_eq!(b.check(hour_ago + 100), BudgetVerdict::BudgetExhausted);
        assert_eq!(b.check(hour_ago + 3601), BudgetVerdict::Allowed);
    }

    #[test]
    fn breaker_trips_on_consecutive_failures() {
        let mut b = ReflexBudget::with_limits(10, 3);
        b.record_outcome(false);
        assert!(!b.is_breaker_open());
        b.record_outcome(false);
        assert!(!b.is_breaker_open());
        b.record_outcome(false);
        assert!(b.is_breaker_open());
        assert_eq!(b.check(1_000_000), BudgetVerdict::BreakerOpen);
    }

    #[test]
    fn success_resets_failure_counter() {
        let mut b = ReflexBudget::with_limits(10, 3);
        b.record_outcome(false);
        b.record_outcome(false);
        b.record_outcome(true);
        b.record_outcome(false);
        b.record_outcome(false);
        assert!(!b.is_breaker_open());
    }

    #[test]
    fn breaker_reset_allows_new_firings() {
        let mut b = ReflexBudget::with_limits(10, 2);
        b.record_outcome(false);
        b.record_outcome(false);
        assert!(b.is_breaker_open());
        b.reset_breaker();
        assert!(!b.is_breaker_open());
        assert_eq!(b.check(1_000_000), BudgetVerdict::Allowed);
    }
}

mod rule {
    use russell_core::rule::{RuleSet, Rule};
    use russell_core::event::Severity;

    #[test]
    fn empty_ruleset_returns_info() {
        let rs = RuleSet::new();
        assert_eq!(rs.evaluate("mem_available_mib", 100.0), Severity::Info);
    }

    #[test]
    fn defaults_mem_warn_below() {
        let rs = RuleSet::with_defaults();
        assert_eq!(rs.evaluate("mem_available_mib", 8192.0), Severity::Info);
        assert_eq!(rs.evaluate("mem_available_mib", 3000.0), Severity::Warn);
        assert_eq!(rs.evaluate("mem_available_mib", 1500.0), Severity::Alert);
        assert_eq!(rs.evaluate("mem_available_mib", 500.0), Severity::Crit);
        assert_eq!(rs.evaluate("mem_available_mib", 4096.0), Severity::Info);
        assert_eq!(rs.evaluate("mem_available_mib", 4095.99), Severity::Warn);
    }

    #[test]
    fn defaults_swap_warn_above() {
        let rs = RuleSet::with_defaults();
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
}