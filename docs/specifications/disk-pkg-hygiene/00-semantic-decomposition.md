---
title: "Disk & Package Hygiene — Task 0: Semantic Decomposition"
audience: [developers, architects, agents]
last_updated: 2026-05-06
togaf_phase: "B"
version: "0.1.0"
status: "Draft"
parent: "../DISK_PKG_HYGIENE_SPEC.md"
---

<!-- TOGAF_DOMAIN: Business -->
<!-- VERSION: 0.1.0 -->
<!-- STATUS: Draft -->
<!-- LAST_UPDATED: 2026-05-06 -->


# Task 0 — Semantic Decomposition: Root Causes of Host Entropy

## Overview

Six causal drivers produce disk and package entropy on a
single-operator Ubuntu 25.10 AI/ML workstation. Each driver is
decomposed into: (a) observable signal, (b) unit and severity
threshold, (c) Russell infrastructure that surfaces it.

---

## 1. Provenance Loss

**Definition.** Binaries installed via `curl | sh`,
`gh release download`, or manual `cp` to `~/.local/bin` have no
package-manager metadata. Their origin, version, and update
channel become unknowable over time.

### (a) Observable Signal

| What Russell observes | How (no mutation) |
|---|---|
| Count of files in `~/.local/bin` not owned by any known provider | `readdir` + cross-reference against apt `dpkg -S`, pip `pip show`, cargo `~/.cargo/bin` symlinks |
| Binary exists but `--version` fails or returns unparseable output | `Command::new(path).arg("--version")` with 2s timeout |
| Binary listed in `provenance.toml` but missing from disk | `stat()` on declared path |

**Tool/Connector separation:**
- **Connector:** `std::fs::read_dir("~/.local/bin")`, `Command::new(binary).arg("--version")`
- **Tool:** Classify each binary as tracked/untracked; parse version output against known patterns

### (b) Unit and Severity

| Metric | Unit | Warn | Alert |
|---|---|---|---|
| `pkg_local_bin_untracked_count` | count | ≥ 5 | ≥ 15 |
| `pkg_provenance_missing_count` | count | ≥ 1 | ≥ 3 |
| `pkg_provenance_stale_count` | count | ≥ 3 | ≥ 10 |

### (c) Russell Surface

- **Journal samples:** Written by `pkg_ecosystem` probe family (hourly cadence).
- **Digest:** `russell digest` includes provenance summary when any count > 0.
- **Jack consultation:** SOAP Objective section includes provenance counts; Jack can recommend the operator update `provenance.toml` or check for newer releases.

---

## 2. Orphaned Dependencies

**Definition.** Packages installed as transitive dependencies by
apt, pip, npm, or brew that remain after the parent is removed.
`apt autoremove` misses some; pip has no dependency tracking;
brew leaves accumulate; npm global orphans persist.

### (a) Observable Signal

| What Russell observes | How (no mutation) |
|---|---|
| apt autoremovable count | `apt --dry-run autoremove 2>/dev/null` — parse "The following packages will be removed" |
| pip packages with no reverse dependency | `pip list --outdated --format=json` (partial signal — pip lacks true orphan detection) |
| brew leaves (installed but not depended upon) | `brew leaves --installed-on-request` vs `brew leaves` delta |
| npm global packages with no dependents | `npm ls -g --json` — top-level only |

**Tool/Connector separation:**
- **Connector:** `Command::new("apt").args(["--dry-run", "autoremove"])` with 5s timeout
- **Tool:** Parse stdout line count, extract package names, compute count

### (b) Unit and Severity

| Metric | Unit | Warn | Alert |
|---|---|---|---|
| `pkg_apt_autoremovable_count` | count | ≥ 10 | ≥ 50 |
| `pkg_brew_leaves_count` | count | ≥ 20 | ≥ 50 |

### (c) Russell Surface

- **Journal samples:** `pkg_ecosystem` probe family.
- **Digest:** Orphan counts appear in the "Package Health" section.
- **Jack consultation:** Jack references orphan counts in Assessment; may recommend `apt autoremove` (recommend only — JR-2).

---

## 3. Cache Accumulation

**Definition.** Each provider maintains caches that grow
unboundedly: `~/.cache/pip`, `~/.npm/_cacache`,
`~/.cache/huggingface/`, `$(brew --cache)`,
`/var/cache/apt/archives/`, `~/.cargo/registry/cache/`,
Docker/Podman layer caches, Ollama model blobs.

### (a) Observable Signal

| What Russell observes | How (no mutation) |
|---|---|
| Size of each known cache directory | Recursive `read_dir` + `metadata().len()` summation |
| Total pruneable cache size | Sum of all known cache regions |
| Growth rate (Δ since last observation) | Journal diff between current and previous sample |

**Tool/Connector separation:**
- **Connector:** `std::fs::read_dir` recursive walk, `std::fs::metadata` for file sizes
- **Tool:** Sum bytes, convert to MiB, compute delta from previous journal entry

### (b) Unit and Severity

| Metric | Unit | Warn | Alert |
|---|---|---|---|
| `cache_total_mib` | MiB | ≥ 50,000 (50 GiB) | ≥ 150,000 (150 GiB) |
| `cache_huggingface_mib` | MiB | ≥ 30,000 | ≥ 100,000 |
| `cache_pip_mib` | MiB | ≥ 5,000 | ≥ 20,000 |
| `cache_npm_mib` | MiB | ≥ 2,000 | ≥ 10,000 |
| `cache_cargo_mib` | MiB | ≥ 5,000 | ≥ 20,000 |

*Note: Thresholds are calibrated for a 3.6 TB root partition. On smaller disks, scale proportionally.*

### (c) Russell Surface

- **Journal samples:** `disk_hygiene` probe family (5-minute cadence — cheap I/O only).
- **Digest:** Cache sizes appear with delta-since-last-digest.
- **Jack consultation:** SOAP Objective includes cache sizes; Jack can recommend `pip cache purge`, `npm cache clean --force`, etc. (recommend only).

---

## 4. Version Drift

**Definition.** Installed packages fall behind their upstream
releases at different rates across providers; the providers
themselves (pip, npm, brew, rustup, nvm) also drift.

### (a) Observable Signal

| What Russell observes | How (no mutation) |
|---|---|
| apt upgradable count | `apt list --upgradable 2>/dev/null \| wc -l` |
| pip outdated count | `pip list --outdated --format=json` |
| brew outdated count | `brew outdated --json` |
| npm global outdated count | `npm outdated -g --json` |
| Provenance-tracked binaries behind latest GitHub release | GitHub Releases API (conditional GET, cached) |
| Rust toolchain age | `rustup check` output parsing |

**Tool/Connector separation:**
- **Connector:** Subprocess invocations; GitHub API HTTP GET (rate-limited)
- **Tool:** Parse JSON/text output into counts; compare semver strings

### (b) Unit and Severity

| Metric | Unit | Warn | Alert |
|---|---|---|---|
| `pkg_apt_upgradable_count` | count | ≥ 20 | ≥ 100 |
| `pkg_pip_outdated_count` | count | ≥ 10 | ≥ 50 |
| `pkg_brew_outdated_count` | count | ≥ 10 | ≥ 30 |
| `pkg_npm_outdated_global_count` | count | ≥ 5 | ≥ 20 |
| `pkg_provenance_stale_count` | count | ≥ 3 | ≥ 10 |

### (c) Russell Surface

- **Journal samples:** `pkg_ecosystem` probe family (hourly/daily cadence).
- **Digest:** "Version Drift" section with per-provider counts.
- **Jack consultation:** Jack references drift in Assessment; may note security implications of stale packages.

---

## 5. Configuration Residue

**Definition.** Uninstalled tools leave behind
`~/.config/<tool>/`, `~/.local/share/<tool>/`, systemd user
units, shell completions, PATH entries, and environment variables.

### (a) Observable Signal

| What Russell observes | How (no mutation) |
|---|---|
| Config dirs in `~/.config/` with no matching installed binary | `readdir ~/.config` cross-referenced against PATH binaries |
| Data dirs in `~/.local/share/` with no matching binary | Same cross-reference |
| Stale systemd user units (loaded=not-found) | `systemctl --user list-unit-files` parse |

**Tool/Connector separation:**
- **Connector:** `read_dir("~/.config")`, `Command::new("systemctl")`
- **Tool:** Cross-reference directory names against known-installed set; classify as orphaned or active

### (b) Unit and Severity

| Metric | Unit | Warn | Alert |
|---|---|---|---|
| `config_orphaned_dirs_count` | count | ≥ 10 | ≥ 30 |
| `systemd_user_not_found_count` | count | ≥ 3 | ≥ 10 |

*Note: This probe has high false-positive risk. Many config dirs are legitimate even without a binary in PATH (e.g., `~/.config/gtk-3.0`). Phase 2 requires a curated allowlist.*

### (c) Russell Surface

- **Journal samples:** `pkg_ecosystem` probe family (daily cadence — expensive cross-reference).
- **Digest:** "Configuration Residue" section (only when count > threshold).
- **Jack consultation:** Jack can identify specific orphaned configs if operator asks.

---

## 6. Provider Conflict

**Definition.** The same logical package (e.g., `python3-numpy`)
may exist via apt AND pip, creating shadowing, ABI mismatch, or
silent version pinning.

### (a) Observable Signal

| What Russell observes | How (no mutation) |
|---|---|
| Python packages installed via both apt and pip | `dpkg -l 'python3-*'` vs `pip list --format=json` — name correlation |
| Node packages shadowing system binaries | `which <name>` resolving to different paths than `dpkg -S` |
| Multiple Python interpreters with conflicting site-packages | `python3 --version` vs `/usr/bin/python3 --version` vs `~/.local/bin/python3` |

**Tool/Connector separation:**
- **Connector:** `Command::new("dpkg")`, `Command::new("pip")`, `which` lookups
- **Tool:** Normalize package names across providers (e.g., `python3-numpy` ↔ `numpy`); detect duplicates; compare versions

### (b) Unit and Severity

| Metric | Unit | Warn | Alert |
|---|---|---|---|
| `pkg_cross_provider_conflict_count` | count | ≥ 3 | ≥ 10 |

*Note: This is computationally expensive and semantically complex. Deferred to Phase 3 as a Doctor-level assessment rather than a simple probe.*

### (c) Russell Surface

- **Journal samples:** Not a standard probe — too complex for a scalar. Surfaces as a structured event (`harness.event.v1` with `outputs` map).
- **Digest:** "Provider Conflicts" section (only when detected).
- **Jack consultation:** This is Jack's strongest value-add — the LLM can reason about shadowing implications and recommend resolution order. The connector to Jack/Kask carries the conflict list in the SOAP Objective for assessment.

---

## Summary Table

| # | Driver | Primary Metric | Cadence | Phase |
|---|---|---|---|---|
| 1 | Provenance loss | `pkg_local_bin_untracked_count` | hourly | 2 |
| 2 | Orphaned deps | `pkg_apt_autoremovable_count` | hourly | 2 |
| 3 | Cache accumulation | `cache_total_mib` | 5-min | 2 |
| 4 | Version drift | `pkg_apt_upgradable_count` | daily | 2 |
| 5 | Config residue | `config_orphaned_dirs_count` | daily | 3 |
| 6 | Provider conflict | `pkg_cross_provider_conflict_count` | daily | 3 |

All six drivers are **observe-only** in Phase 2. Recommendations
surface through Jack (the LLM connector). Mutations (cleanup
actions) are Phase 4+ skill territory requiring full IDRS
compliance.
