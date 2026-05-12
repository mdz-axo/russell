# Changelog

## [Unreleased]

### Added
- **GitHub Actions CI workflow** (`ci.yml`): enforces `cargo fmt --check`,
  `cargo clippy -D warnings`, `cargo test --workspace`, and `cargo deny check`
  on every push/PR to `main`.

### Fixed
- `cargo clippy -D warnings` now passes: added `is_empty()` methods to
  `ScheduleSet` and `RuleSet`, added rustdoc to `Schedule` and `ScheduleSet`
  public items, suppressed dead-code warning on deserialization-only struct
  `OkapiMetricsResponse`, replaced `len() > 0` with `!is_empty()`,
  removed needless borrow in `proprio.rs`.
- `cargo fmt --check` now passes: fixed formatting in `rule/mod.rs`,
  `fallback.rs`, `disks.rs`, `network.rs`.
- Workspace `Cargo.toml` repository URL corrected from placeholder to
  `https://github.com/Replicant-Partners/russell`.

## [0.2.0] — 2026-05-11

### Added
- **Process probes** (7): `proc_total_count`, `proc_zombie_count`, `proc_stuck_count`,
  `proc_running_count`, `proc_top_cpu_name`, `proc_top_mem_name`, `proc_top_mem_pct`.
  Scan `/proc` and `/proc/[pid]/stat` to surface process-level health.
- **GPU probes** (5): `gpu_vram_used_pct`, `gpu_vram_used_mib`, `gpu_vram_total_mib`,
  `gpu_temp_c`, `gpu_util_pct`. Target discrete GPU via `/sys/class/drm/card1/device/`.
- **Disk probes** (2): `disk_io_pressure_some_pct`, `disk_io_pressure_full_pct`.
  From `/proc/pressure/io`.
- **Systemd probes** (3): `systemd_degraded`, `systemd_user_failed_count`,
  `systemd_system_failed_count`. Via `systemctl` subprocess.
- **Baseline deviation**: 30-day p95 baseline column in Jack's SOAP Objective table.
  `JournalReader::read_baselines()` queries persisted percentiles.
- **Skill consent flow**: `russell chat` supports `/approve` and `/deny` commands.
  Jack proposes interventions via `ACTION: <skill>/<intervention>` syntax.
  Parsed by both `russell jack` and `russell chat`.
- **Production skill**: `okapi-watcher` with `restart-okapi` intervention (risk: low,
  `systemctl --user restart okapi`). `needs_sudo: bool` field on `Intervention` struct.
- **Risk band enforcement**: `Dispatcher::check_risk()` gates interventions above
  `max_auto_risk` (default: Low). `RiskBand::as_str()` for journal-friendly formatting.
- **CLI verb**: `russell proprio` — standalone self-observation.
- **Packaging**: `install.sh` (one-command setup with `--dev`, `--check`, `--uninstall`),
  `Makefile` (dev targets: build, test, lint, jack, chat).
- **Default rules**: 21 rules covering all numeric probes with production thresholds.

### Changed
- **IPC dispatch upgrade**: `raven-win` 0.3.7 replaces `message-broker` 1.0.2
  for all inter-process control channels.
- **Dispatch wired for production**: Removed `#[cfg(test)]` gates from all core
  dispatch types and functions (`run_and_journal`, `run_intervention_with_rollback`,
  `check_risk`, `write_evidence`).
- **Sudo support in dispatcher**: `Dispatcher` accepts optional `sudo_password`,
  wraps commands in `sudo -S` with piped stdin. Manual `Debug` impl redacts the field.
- **Jack persona updated**: Prompts teach `ACTION:` proposal syntax, baseline
  deviation interpretation (1.5×/3×/10× p95 thresholds), consent-gated interventions.
- **SOAP Objective enhanced**: Sample summary table conditionally includes
  "p95 (30d)" column when baselines exist.
- **Systemd unit files**: Removed hardcoded documentation paths. Okapi service
  pulls default model from `$RUSSELL_OKAPI_DEFAULT_MODEL`.
- **Single-scan process collection**: `process_samples()` collects all 7 process
  probes in one `/proc` sweep (was 7 independent scans).
- **Single-read disk pressure**: `disk_io_pressure_samples()` reads
  `/proc/pressure/io` once for both metrics.
- **Risk band formatting**: Replaced `Debug::{:?}` + `to_lowercase()` with
  `RiskBand::as_str()` across all journaling paths.

### Fixed
- **`systemd_degraded` dead code** (P0): Probe never returned 1.0 because
  `run_command_stdout` rejected non-zero exit codes. Fixed with
  `run_command_stdout_always` + `match` on all degraded states.
- **Risk enforcement bypass** (P0): `max_auto_risk` was set to action's own risk,
  `check_risk()` was never called. Fixed with system default cap + explicit check.
- **Sudo password leak** (P0): `Debug`/`Clone` derives on `Dispatcher` exposed
  the password field. Fixed with manual redacting `Debug` impl.
- **Division by zero**: `proc_top_mem_pct` returned `inf`/`NaN` when `MemTotal=0`.
  Added zero-guard.
- **Failed dispatch not journaled**: Spawn failures silently skipped journaling.
  Now writes failure events before propagating the error.
- **reqwest 0.13 compatibility**: `okapi_probe.rs` converted to async reqwest calls
  after lockfile update removed `reqwest::blocking`.
- **Contradictory prompt formats**: `prompt.rs` told LLM to use `RECOMMEND:`
  while persona said `ACTION:`. Aligned on `ACTION:` throughout.

## [0.1.0-phase0] — 2026-04-18

### Added
- Initial skeleton: `russell-core`, `russell-sentinel`, `russell-proprio`,
  `russell-doctor`, `russell-skills`, `russell-mcp` crates.
- Six CLI verbs: `status`, `list`, `profile`, `digest`, `sentinel-once`, `jack`.
- Three observation probes: `mem_available_mib`, `swap_used_mib`, `loadavg_1m`.
- One self-vital: `sentinel_last_run_age_s`.
- SQLite journal with WAL + migrations.
- LLM help flow (`russell jack`) with offline fallback.
- Pinned MVP boundary at `MVP_SPEC.md`.
- Seven ADRs accepted, seven deferred.