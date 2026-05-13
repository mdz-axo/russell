---
title: "Disk & Package Hygiene — Task 2: Disk Hygiene Probes"
audience: [developers, agents]
last_updated: 2026-05-06
togaf_phase: "D"
version: "0.1.0"
status: "Draft"
parent: "../DISK_PKG_HYGIENE_SPEC.md"
---

<!-- TOGAF_DOMAIN: Technology -->
<!-- VERSION: 0.1.0 -->
<!-- STATUS: Draft -->
<!-- LAST_UPDATED: 2026-05-06 -->


# Task 2 — Probe Design: Disk Hygiene Observation

## Design Constraints

1. **Infallible.** Every probe returns `Option<f64>`. No panics. No propagated I/O errors.
2. **No mutation.** Read-only filesystem operations only (JR-2).
3. **No external crates** beyond `nix` (already in the ecosystem for `statvfs`). Directory walking uses `std::fs::read_dir` (JR-1/JR-6).
4. **5-minute cadence.** These probes are cheap enough for the standard Sentinel cycle.
5. **Permission-denied = None.** If a directory is unreadable, the probe returns `None` rather than failing the entire collection.

## Tool/Connector Decomposition

Per `audit-crate.md`, each probe decomposes into:

| Layer | Responsibility | Example |
|---|---|---|
| **Connector** (I/O boundary) | Acquire raw data from OS | `statvfs("/")` returns `Statvfs` struct |
| **Tool** (pure transform) | Convert raw data to domain value | `(total_blocks - free_blocks) / total_blocks * 100.0` |

The implementation keeps these in separate functions so the tool
layer is unit-testable without filesystem access.

---

## Probe Table

| Probe name | Source | Unit | Notes |
|---|---|---|---|
| `disk_root_used_pct` | `statvfs("/")` | `%` | Percentage of root filesystem used |
| `disk_root_avail_gib` | `statvfs("/")` | `GiB` | Absolute available space |
| `inode_root_used_pct` | `statvfs("/")` | `%` | Inode exhaustion signal |
| `cache_total_mib` | Sum of known cache dirs | `MiB` | Aggregate pruneable cache size |
| `cache_pip_mib` | `~/.cache/pip` | `MiB` | pip-specific |
| `cache_npm_mib` | `~/.npm/_cacache` | `MiB` | npm-specific |
| `cache_cargo_mib` | `~/.cargo/registry/cache` | `MiB` | cargo-specific |
| `cache_huggingface_mib` | `~/.cache/huggingface` | `MiB` | Model cache (often largest) |
| `tmp_age_max_days` | oldest file in `/tmp` | `days` | Stale temp file signal |

---

## Module Layout

New file: `crates/russell-sentinel/src/probes/disk.rs`

The module restructuring is complete (per ADR-0019): `probes.rs`
has been decomposed into `probes/mod.rs` + sub-modules. The disk
probe module slots in alongside the existing `memory.rs`.

```
crates/russell-sentinel/src/
├── lib.rs
└── probes/
    ├── mod.rs          # collect() orchestrator
    ├── memory.rs       # existing mem/swap/loadavg probes (extracted)
    └── disk.rs         # NEW: disk hygiene probes
```

---

## Implementation Sketch

### Connector Layer (I/O boundary)

```rust
// crates/russell-sentinel/src/probes/disk.rs — connector functions

use std::fs;
use std::path::Path;
use std::time::SystemTime;

/// Connector: call statvfs on a path. Returns None on any error.
fn statvfs_raw(path: &str) -> Option<nix::sys::statvfs::Statvfs> {
    nix::sys::statvfs::statvfs(path).ok()
}

/// Connector: recursively measure directory size in bytes.
/// Returns None if the directory doesn't exist or is unreadable.
/// Skips unreadable entries silently (permission-denied = skip).
fn dir_size_bytes(path: &Path) -> Option<u64> {
    if !path.is_dir() {
        return None;
    }
    let mut total: u64 = 0;
    dir_size_recursive(path, &mut total);
    Some(total)
}

fn dir_size_recursive(path: &Path, total: &mut u64) {
    let entries = match fs::read_dir(path) {
        Ok(e) => e,
        Err(_) => return, // permission denied or gone — skip
    };
    for entry in entries.flatten() {
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        if meta.is_file() {
            *total += meta.len();
        } else if meta.is_dir() {
            dir_size_recursive(&entry.path(), total);
        }
    }
}

/// Connector: find the oldest file modification time in a directory (non-recursive).
fn oldest_file_mtime(path: &Path) -> Option<SystemTime> {
    let entries = fs::read_dir(path).ok()?;
    let mut oldest: Option<SystemTime> = None;
    for entry in entries.flatten() {
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        if meta.is_file() {
            if let Ok(mtime) = meta.modified() {
                oldest = Some(match oldest {
                    Some(prev) if mtime < prev => mtime,
                    Some(prev) => prev,
                    None => mtime,
                });
            }
        }
    }
    oldest
}
```

### Tool Layer (pure transforms)

```rust
// crates/russell-sentinel/src/probes/disk.rs — tool functions (pure, testable)

/// Tool: compute used percentage from statvfs fields.
/// Formula: (total - available) / total * 100
/// Uses f_bavail (available to unprivileged users), not f_bfree.
fn compute_used_pct(total_blocks: u64, avail_blocks: u64) -> f64 {
    if total_blocks == 0 {
        return 0.0;
    }
    let used = total_blocks.saturating_sub(avail_blocks);
    (used as f64 / total_blocks as f64) * 100.0
}

/// Tool: compute available GiB from statvfs fields.
fn compute_avail_gib(avail_blocks: u64, block_size: u64) -> f64 {
    let bytes = avail_blocks as f64 * block_size as f64;
    bytes / (1024.0 * 1024.0 * 1024.0)
}

/// Tool: compute inode used percentage.
fn compute_inode_used_pct(total_inodes: u64, free_inodes: u64) -> f64 {
    if total_inodes == 0 {
        return 0.0;
    }
    let used = total_inodes.saturating_sub(free_inodes);
    (used as f64 / total_inodes as f64) * 100.0
}

/// Tool: convert bytes to MiB.
fn bytes_to_mib(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

/// Tool: compute age in fractional days from a SystemTime.
fn mtime_to_age_days(mtime: SystemTime) -> Option<f64> {
    let elapsed = mtime.elapsed().ok()?;
    Some(elapsed.as_secs_f64() / 86_400.0)
}
```

### Probe Functions (compose connector + tool)

```rust
/// Probe: root filesystem used percentage.
fn disk_root_used_pct() -> Option<f64> {
    let st = statvfs_raw("/")?;
    Some(compute_used_pct(st.blocks(), st.blocks_available()))
}

/// Probe: root filesystem available GiB.
fn disk_root_avail_gib() -> Option<f64> {
    let st = statvfs_raw("/")?;
    Some(compute_avail_gib(st.blocks_available(), st.fragment_size()))
}

/// Probe: root filesystem inode used percentage.
fn inode_root_used_pct() -> Option<f64> {
    let st = statvfs_raw("/")?;
    Some(compute_inode_used_pct(st.files(), st.files_free()))
}

/// Probe: size of a specific cache directory in MiB.
fn cache_dir_mib(path: &Path) -> Option<f64> {
    dir_size_bytes(path).map(bytes_to_mib)
}

/// Probe: oldest file in /tmp, age in days.
fn tmp_age_max_days() -> Option<f64> {
    let oldest = oldest_file_mtime(Path::new("/tmp"))?;
    mtime_to_age_days(oldest)
}
```

### Collection Function

```rust
use super::Sample;
use std::path::PathBuf;

/// Known cache directories. Each entry: (probe_name, path).
const CACHE_DIRS: &[(&str, &str)] = &[
    ("cache_pip_mib", ".cache/pip"),
    ("cache_npm_mib", ".npm/_cacache"),
    ("cache_cargo_mib", ".cargo/registry/cache"),
    ("cache_huggingface_mib", ".cache/huggingface"),
];

/// Collect all disk hygiene samples.
pub fn collect() -> Vec<Sample> {
    let mut out = Vec::new();

    // Root filesystem probes (via statvfs)
    if let Some(v) = disk_root_used_pct() {
        out.push(Sample {
            name: "disk_root_used_pct".into(),
            value_num: Some(v),
            value_text: None,
            unit: Some("%"),
        });
    }
    if let Some(v) = disk_root_avail_gib() {
        out.push(Sample {
            name: "disk_root_avail_gib".into(),
            value_num: Some(v),
            value_text: None,
            unit: Some("GiB"),
        });
    }
    if let Some(v) = inode_root_used_pct() {
        out.push(Sample {
            name: "inode_root_used_pct".into(),
            value_num: Some(v),
            value_text: None,
            unit: Some("%"),
        });
    }

    // Cache directory probes
    let home = match std::env::var("HOME") {
        Ok(h) => PathBuf::from(h),
        Err(_) => return out, // no HOME = skip cache probes
    };

    let mut cache_total: f64 = 0.0;
    for (name, rel_path) in CACHE_DIRS {
        let full_path = home.join(rel_path);
        if let Some(v) = cache_dir_mib(&full_path) {
            cache_total += v;
            out.push(Sample {
                name: (*name).into(),
                value_num: Some(v),
                value_text: None,
                unit: Some("MiB"),
            });
        }
    }
    if cache_total > 0.0 {
        out.push(Sample {
            name: "cache_total_mib".into(),
            value_num: Some(cache_total),
            value_text: None,
            unit: Some("MiB"),
        });
    }

    // Tmp age probe
    if let Some(v) = tmp_age_max_days() {
        out.push(Sample {
            name: "tmp_age_max_days".into(),
            value_num: Some(v),
            value_text: None,
            unit: Some("days"),
        });
    }

    out
}
```

---

## Testing Strategy

### Unit Tests (tool layer — no filesystem needed)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn used_pct_zero_when_all_available() {
        assert_eq!(compute_used_pct(1000, 1000), 0.0);
    }

    #[test]
    fn used_pct_hundred_when_none_available() {
        assert_eq!(compute_used_pct(1000, 0), 100.0);
    }

    #[test]
    fn used_pct_handles_zero_total() {
        assert_eq!(compute_used_pct(0, 0), 0.0);
    }

    #[test]
    fn avail_gib_correct() {
        // 1 GiB = 1024*1024*1024 bytes
        // With block_size=4096 and avail_blocks=262144:
        // 262144 * 4096 = 1,073,741,824 = 1 GiB
        let gib = compute_avail_gib(262_144, 4096);
        assert!((gib - 1.0).abs() < 0.001);
    }

    #[test]
    fn bytes_to_mib_correct() {
        assert!((bytes_to_mib(1_048_576) - 1.0).abs() < 0.001);
    }

    #[test]
    fn inode_pct_fifty() {
        assert!((compute_inode_used_pct(1000, 500) - 50.0).abs() < 0.001);
    }
}
```

### Integration Tests (connector layer — requires Linux /proc, /tmp)

```rust
#[cfg(test)]
mod integration {
    use super::*;

    #[test]
    fn disk_probes_return_some_on_linux() {
        if !std::path::Path::new("/proc").exists() {
            return; // skip on non-Linux
        }
        assert!(disk_root_used_pct().is_some());
        assert!(disk_root_avail_gib().is_some());
        assert!(inode_root_used_pct().is_some());
    }

    #[test]
    fn collect_produces_at_least_statvfs_probes() {
        if !std::path::Path::new("/proc").exists() {
            return;
        }
        let samples = collect();
        let names: Vec<&str> = samples.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"disk_root_used_pct"));
        assert!(names.contains(&"disk_root_avail_gib"));
    }

    #[test]
    fn cache_dir_mib_returns_none_for_missing_dir() {
        let result = cache_dir_mib(Path::new("/nonexistent/path/xyz"));
        assert!(result.is_none());
    }
}
```

---

## Dependency Impact

| Crate | Change |
|---|---|
| `russell-sentinel` | Add `nix` dependency (for `statvfs`) |
| `russell-core` | None — `Sample` struct already sufficient |

The `nix` crate is already in the Rust ecosystem and provides
safe wrappers around POSIX syscalls. Per JR-6 (reuse, don't
depend), if `nix` is too heavy, the alternative is a minimal
`libc::statvfs` wrapper with `unsafe` — but `nix` is preferred
for safety.

---

## Severity Thresholds (Phase 2 Rules)

When the rule engine lands, these thresholds will be defined in
`~/.config/harness/rules.d/disk-hygiene.toml`:

```toml
[disk_root_used_pct]
warn = 80.0
alert = 90.0
crit = 95.0

[inode_root_used_pct]
warn = 80.0
alert = 90.0
crit = 95.0

[cache_total_mib]
warn = 50000.0   # 50 GiB
alert = 150000.0 # 150 GiB

[tmp_age_max_days]
warn = 30.0
alert = 90.0
```

Until the rule engine exists, these are informational only —
surfaced in `russell digest` and available to Jack in the SOAP
Objective.
