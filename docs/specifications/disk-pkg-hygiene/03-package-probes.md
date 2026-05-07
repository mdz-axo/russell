---
title: "Disk & Package Hygiene — Task 3: Package Ecosystem Probes"
audience: [developers, agents]
last_updated: 2026-05-06
togaf_phase: "D — Technology Architecture"
version: "0.1.0"
status: "Draft"
---

# Task 3 — Probe Design: Package Ecosystem Observation

## Design Constraints

1. **Infallible.** Every probe returns `Option<f64>`. Provider not installed = `None`. Command timeout = `None`. Parse failure = `None`.
2. **No mutation.** Observe only (JR-2).
3. **Subprocess timeout.** Every `Command` invocation has a 5-second timeout. On timeout, return `None`.
4. **Longer cadence.** These probes do NOT run on the 5-minute Sentinel cycle. They run hourly or daily (cadence TBD — see Task 6).
5. **Provider auto-detection.** Each adapter checks `is_available()` before observation. Missing providers are silently skipped.

## Tool/Connector Decomposition

| Layer | Responsibility | Example |
|---|---|---|
| **Connector** | Invoke subprocess, capture stdout | `Command::new("apt").args(["list", "--upgradable"]).output()` |
| **Tool** | Parse stdout into domain value | Count non-header lines in apt output |

The connector is a generic `run_cmd(cmd, args, timeout) -> Option<String>` utility.
The tool is a per-provider parser function `parse_apt_upgradable(stdout: &str) -> Option<f64>`.

These are **never conflated** — the parser is unit-testable with canned stdout strings.

---

## Probe Table

| Probe name | Source command | Unit | Notes |
|---|---|---|---|
| `pkg_apt_upgradable_count` | `apt list --upgradable` | count | Pending apt updates |
| `pkg_apt_autoremovable_count` | `apt --dry-run autoremove` | count | Orphaned apt packages |
| `pkg_pip_outdated_count` | `pip list --outdated --format=json` | count | Stale pip packages |
| `pkg_pipx_outdated_count` | `pipx list --json` + version comparison | count | Stale pipx apps |
| `pkg_npm_outdated_global_count` | `npm outdated -g --json` | count | Stale global npm packages |
| `pkg_brew_outdated_count` | `brew outdated --json` | count | Stale brew formulae |
| `pkg_snap_held_revisions` | `snap list --all` (grep disabled) | count | Old snap revisions consuming disk |
| `pkg_flatpak_unused_runtimes` | `flatpak list --runtime --columns=ref` | count | Unused Flatpak runtimes |
| `pkg_cargo_installed_count` | `ls ~/.cargo/bin` | count | Cargo-installed binaries (inventory) |
| `pkg_local_bin_untracked_count` | `~/.local/bin` cross-reference | count | Provenance-unknown binaries |

---

## Module Layout

```
crates/russell-sentinel/src/probes/
├── mod.rs              # collect() + collect_extended()
├── memory.rs           # existing probes (extracted from probes.rs)
├── disk.rs             # Task 2 disk probes
└── packages.rs         # NEW: package ecosystem probes

crates/russell-core/src/
├── provider.rs         # NEW: ProviderHealth trait (port definition)
└── ...
```

---

## Port Definition: `ProviderHealth` Trait

Lives in `russell-core` as a domain port (no external dependencies):

```rust
// crates/russell-core/src/provider.rs

/// Port: a package provider that Russell can observe.
pub trait ProviderHealth {
    /// Provider identifier (e.g., "apt", "pip", "npm").
    fn provider_id(&self) -> &'static str;

    /// Whether this provider is present on the system.
    fn is_available(&self) -> bool;

    /// Collect observable samples from this provider.
    /// Returns empty vec if provider is not installed or all probes fail.
    ///
    /// Implementations MUST:
    /// - Return within 10 seconds (sum of all internal timeouts)
    /// - Never panic
    /// - Never mutate host state
    fn observe(&self) -> Vec<crate::Sample>;
}
```

---

## Generic Connector: `run_cmd`

```rust
use std::process::Command;
use std::time::Duration;

/// Connector: run a command with timeout, return stdout as String.
/// Returns None on: command not found, timeout, non-zero exit, UTF-8 error.
fn run_cmd(program: &str, args: &[&str], timeout: Duration) -> Option<String> {
    let child = Command::new(program)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;

    let output = child.wait_with_output().ok()?;
    String::from_utf8(output.stdout).ok()
}

/// Connector: check if a program exists on PATH.
fn is_on_path(program: &str) -> bool {
    Command::new("which")
        .arg(program)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
```

---

## Provider Adapters (Tool Layer — Parsers)

### Apt Adapter

```rust
/// Tool: parse `apt list --upgradable` output.
/// First line is "Listing..." header; count remaining non-empty lines.
fn parse_apt_upgradable(stdout: &str) -> f64 {
    stdout.lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .count() as f64
}

/// Tool: parse `apt --dry-run autoremove` output.
/// Count lines matching "Remv " prefix.
fn parse_apt_autoremove(stdout: &str) -> f64 {
    stdout.lines()
        .filter(|l| l.starts_with("Remv "))
        .count() as f64
}
```

### Pip Adapter

```rust
/// Tool: parse pip JSON output — count array elements.
fn parse_pip_outdated_json(stdout: &str) -> f64 {
    match serde_json::from_str::<Vec<serde_json::Value>>(stdout) {
        Ok(arr) => arr.len() as f64,
        Err(_) => 0.0,
    }
}
```

### Npm Adapter

```rust
/// Tool: parse npm outdated JSON — count top-level keys.
/// Output shape: `{ "package-name": { "current": "...", ... }, ... }`
fn parse_npm_outdated_json(stdout: &str) -> f64 {
    match serde_json::from_str::<serde_json::Value>(stdout) {
        Ok(serde_json::Value::Object(map)) => map.len() as f64,
        _ => 0.0,
    }
}
```

### Brew Adapter

```rust
/// Tool: parse brew outdated JSON — count array elements.
fn parse_brew_outdated_json(stdout: &str) -> f64 {
    match serde_json::from_str::<Vec<serde_json::Value>>(stdout) {
        Ok(arr) => arr.len() as f64,
        Err(_) => 0.0,
    }
}
```

### Snap Adapter

```rust
/// Tool: count lines containing "disabled" in snap list --all output.
fn parse_snap_disabled(stdout: &str) -> f64 {
    stdout.lines()
        .filter(|l| l.contains("disabled"))
        .count() as f64
}
```

### Cargo Adapter (filesystem-only)

```rust
/// Tool: count executable files in ~/.cargo/bin.
fn count_cargo_bins(home: &Path) -> Option<f64> {
    let bin_dir = home.join(".cargo/bin");
    let count = std::fs::read_dir(bin_dir).ok()?
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .count();
    Some(count as f64)
}
```

### Local Bin Untracked

```rust
/// Tool: count binaries in ~/.local/bin not tracked by known providers.
fn count_untracked_local_bins(home: &Path) -> Option<f64> {
    let bin_dir = home.join(".local/bin");
    let entries: Vec<_> = std::fs::read_dir(&bin_dir).ok()?
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_file() || t.is_symlink()).unwrap_or(false))
        .collect();

    let total = entries.len();
    let tracked = entries.iter().filter(|e| {
        let path = e.path();
        if let Ok(target) = std::fs::read_link(&path) {
            let target_str = target.to_string_lossy();
            return target_str.contains(".local/pipx/") || target_str.contains(".cargo/bin");
        }
        false
    }).count();

    Some((total - tracked) as f64)
}
```

---

## Provider Registry

```rust
fn provider_registry() -> Vec<Box<dyn ProviderHealth>> {
    vec![
        Box::new(AptProvider),
        Box::new(PipProvider),
        Box::new(NpmProvider),
        Box::new(BrewProvider),
        Box::new(SnapProvider),
        Box::new(FlatpakProvider),
        Box::new(CargoProvider),
        Box::new(LocalBinProvider),
    ]
}

/// Collect samples from all available providers.
pub fn collect_packages() -> Vec<Sample> {
    provider_registry()
        .into_iter()
        .filter(|p| p.is_available())
        .flat_map(|p| p.observe())
        .collect()
}
```

---

## Testing Strategy

Unit tests use canned stdout strings (tool layer only):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_apt_upgradable_empty() {
        assert_eq!(parse_apt_upgradable("Listing...\n"), 0.0);
    }

    #[test]
    fn parse_apt_upgradable_three() {
        let stdout = "Listing...\nfoo/jammy 2.0 amd64\nbar/jammy 3.0 amd64\nbaz/jammy 1.1 all\n";
        assert_eq!(parse_apt_upgradable(stdout), 3.0);
    }

    #[test]
    fn parse_apt_autoremove_two() {
        let stdout = "Remv libfoo (1.0)\nRemv libbar (2.0)\n";
        assert_eq!(parse_apt_autoremove(stdout), 2.0);
    }

    #[test]
    fn parse_pip_outdated_json_two() {
        let json = r#"[{"name":"foo"},{"name":"bar"}]"#;
        assert_eq!(parse_pip_outdated_json(json), 2.0);
    }

    #[test]
    fn parse_snap_disabled_three() {
        let stdout = "Name Ver Rev Track Pub Notes\nfoo 1.0 42 stable can disabled\nbar 2.0 43 stable can -\nbaz 1.0 10 stable can disabled\nqux 3.0 99 stable can disabled\n";
        assert_eq!(parse_snap_disabled(stdout), 3.0);
    }
}
```

---

## Connector Boundary: Sharing with Jack / Kask

Package probe results flow to the LLM (Jack) or Kask platform via:

1. **Samples → Journal** (connector: `JournalWriter::append_sample`)
2. **Journal → SOAP bundle** (tool: compose Objective text from recent samples)
3. **SOAP bundle → LLM** (connector: OpenRouter HTTP POST)

The package probes are **never sent directly** to the LLM. They
flow through the journal first (JR-7: persistence is auditable),
then are composed into the SOAP bundle by the Doctor's tool layer.
