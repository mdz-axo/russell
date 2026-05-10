---
title: "Disk & Package Hygiene — Task 4: Provenance Registry"
audience: [developers, operators, agents]
last_updated: 2026-05-06
togaf_phase: "C — Data Architecture"
version: "0.1.0"
status: "Draft"
---

# Task 4 — Provenance Registry: Tracking curl-installed and GitHub-sourced Binaries

## Problem Statement

Binaries installed via `curl | sh`, `gh release download`, or
manual `cp` to `~/.local/bin` have no package-manager metadata.
Russell cannot observe their version, origin, or update status
through any standard provider query.

The provenance registry solves this by providing a
**operator-maintained** TOML manifest that Russell **reads but
never writes** (JR-2).

---

## Persistence Registration (JR-7)

This introduces a new persistent artifact:

| Path | Owner | Format | Retention |
|---|---|---|---|
| `~/.local/state/harness/provenance.toml` | Operator (manual edits) | TOML | Unbounded |

**PERSISTENCE_CATALOG.md entry required** in the same commit that
implements this feature.

---

## Schema

```toml
# ~/.local/state/harness/provenance.toml
#
# Each entry tracks a binary Russell cannot observe via a package manager.
# Operator-maintained; Russell reads but never writes (JR-2).

[[artifact]]
name = "zed"
path = "~/.local/bin/zed"
source = "https://github.com/zed-industries/zed/releases"
installed_via = "curl"                    # curl | gh-release | manual | script
version_cmd = "zed --version"
version_pattern = 'Zed (\d+\.\d+\.\d+)'  # regex, first capture group = version
installed_date = "2026-04-10"
check_github_releases = "zed-industries/zed"  # optional: owner/repo for update checks

[[artifact]]
name = "ollama"
path = "/usr/local/bin/ollama"
source = "https://ollama.com/install.sh"
installed_via = "curl"
version_cmd = "ollama --version"
version_pattern = 'ollama version (\S+)'
installed_date = "2026-03-15"

[[artifact]]
name = "claude-code"
path = "~/.local/bin/claude"
source = "https://github.com/anthropics/claude-code/releases"
installed_via = "gh-release"
version_cmd = "claude --version"
version_pattern = '(\d+\.\d+\.\d+)'
installed_date = "2026-04-20"
check_github_releases = "anthropics/claude-code"

[[artifact]]
name = "rust-mcp-server"
path = "~/.cargo/bin/rust-mcp-server"
source = "https://github.com/nicholasgasior/rust-mcp-server"
installed_via = "cargo-install"
version_cmd = "rust-mcp-server --version"
version_pattern = '(\d+\.\d+\.\d+)'
installed_date = "2026-04-01"
check_github_releases = "nicholasgasior/rust-mcp-server"
```

### Field Definitions

| Field | Required | Type | Description |
|---|---|---|---|
| `name` | yes | string | Human-readable identifier |
| `path` | yes | string | Filesystem path (~ expanded at runtime) |
| `source` | yes | string | Origin URL (for audit trail) |
| `installed_via` | yes | enum | `curl` \| `gh-release` \| `manual` \| `script` \| `cargo-install` |
| `version_cmd` | yes | string | Command to extract current version |
| `version_pattern` | no | string | Regex; first capture group = semver. If absent, entire stdout trimmed = version |
| `installed_date` | no | date | ISO 8601 date of installation |
| `check_github_releases` | no | string | `owner/repo` for GitHub Releases API checks |

---

## Tool/Connector Decomposition

### Connectors (I/O boundary)

| Connector | Responsibility |
|---|---|
| `read_provenance_toml(path) -> Option<String>` | Read the TOML file from disk |
| `run_version_cmd(cmd) -> Option<String>` | Execute version command, capture stdout |
| `check_github_latest(owner_repo) -> Option<String>` | HTTP GET to GitHub Releases API (cached, rate-limited) |

### Tools (pure transforms)

| Tool | Responsibility |
|---|---|
| `parse_provenance_toml(content: &str) -> Vec<ArtifactEntry>` | Deserialize TOML into domain structs |
| `extract_version(stdout: &str, pattern: Option<&str>) -> Option<String>` | Apply regex to extract version string |
| `compare_versions(current: &str, latest: &str) -> DriftStatus` | Semver comparison: `UpToDate`, `Behind`, `Unparseable` |
| `check_binary_exists(path: &str) -> bool` | `stat()` on expanded path |

---

## Adapter Implementation: `ProvenanceAdapter`

Implements `ProviderHealth` from `russell-core`:

```rust
pub struct ProvenanceAdapter {
    artifacts: Vec<ArtifactEntry>,
}

#[derive(Debug, Clone)]
pub struct ArtifactEntry {
    pub name: String,
    pub path: String,
    pub source: String,
    pub installed_via: String,
    pub version_cmd: String,
    pub version_pattern: Option<String>,
    pub installed_date: Option<String>,
    pub check_github_releases: Option<String>,
}

#[derive(Debug)]
enum DriftStatus {
    UpToDate,
    Behind { current: String, latest: String },
    Unparseable,
    CheckSkipped,
}

impl ProviderHealth for ProvenanceAdapter {
    fn provider_id(&self) -> &'static str { "provenance" }

    fn is_available(&self) -> bool {
        // Available if provenance.toml exists and has at least one entry
        !self.artifacts.is_empty()
    }

    fn observe(&self) -> Vec<Sample> {
        let mut missing_count: f64 = 0.0;
        let mut stale_count: f64 = 0.0;
        let mut ok_count: f64 = 0.0;

        for artifact in &self.artifacts {
            let expanded_path = expand_tilde(&artifact.path);

            // Check existence (connector: stat)
            if !std::path::Path::new(&expanded_path).exists() {
                missing_count += 1.0;
                continue;
            }

            // Check version (connector: subprocess)
            let current_version = run_version_cmd(&artifact.version_cmd)
                .and_then(|stdout| extract_version(&stdout, artifact.version_pattern.as_deref()));

            // Check for drift (connector: GitHub API, if configured)
            if let Some(ref repo) = artifact.check_github_releases {
                if let Some(ref current) = current_version {
                    match check_drift(current, repo) {
                        DriftStatus::Behind { .. } => stale_count += 1.0,
                        DriftStatus::UpToDate => ok_count += 1.0,
                        _ => ok_count += 1.0, // unparseable = don't alarm
                    }
                }
            } else {
                ok_count += 1.0;
            }
        }

        let mut samples = Vec::new();
        samples.push(Sample::num("pkg_provenance_missing_count", missing_count, "count"));
        samples.push(Sample::num("pkg_provenance_stale_count", stale_count, "count"));
        samples.push(Sample::num("pkg_provenance_tracked_count", ok_count + stale_count + missing_count, "count"));
        samples
    }
}
```

---

## GitHub API Rate Limiting

Russell checks GitHub releases for version drift. Constraints:

- **Unauthenticated rate limit:** 60 requests/hour
- **Strategy:** Conditional GET with `If-None-Match` (ETag caching)
- **Cache location:** In-memory for the process lifetime; persisted as journal samples (the "last known latest version" is the sample itself)
- **Cadence:** At most once per day per artifact (not on every probe cycle)
- **Fallback:** If rate-limited (HTTP 403), skip gracefully — return `DriftStatus::CheckSkipped`

```rust
/// Connector: check GitHub releases API for latest version.
/// Returns None on network error, rate limit, or timeout.
fn check_github_latest(owner_repo: &str) -> Option<String> {
    // GET https://api.github.com/repos/{owner}/{repo}/releases/latest
    // Parse response JSON for .tag_name
    // Strip leading 'v' if present
    // Timeout: 5 seconds
    // On 403 (rate limited): return None
    // On 404 (no releases): return None
    todo!("Implementation uses ureq or reqwest — dependency decision pending")
}
```

**Design decision pending:** Should Russell use `ureq` (minimal,
blocking) or `reqwest` (async, heavier)? Per JR-1, `ureq` is
preferred for its small footprint. However, if `reqwest` is
already in the workspace for the Doctor's LLM calls, reuse it (JR-6).

---

## Testing Strategy

### Unit Tests (tool layer)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_provenance_toml_valid() {
        let toml = r#"
[[artifact]]
name = "zed"
path = "~/.local/bin/zed"
source = "https://github.com/zed-industries/zed/releases"
installed_via = "curl"
version_cmd = "zed --version"
"#;
        let entries = parse_provenance_toml(toml);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "zed");
    }

    #[test]
    fn extract_version_with_pattern() {
        let stdout = "Zed 0.233.1 – /home/user/.local/bin/zed\n";
        let version = extract_version(stdout, Some(r"Zed (\d+\.\d+\.\d+)"));
        assert_eq!(version.as_deref(), Some("0.233.1"));
    }

    #[test]
    fn extract_version_without_pattern() {
        let stdout = "1.2.3\n";
        let version = extract_version(stdout, None);
        assert_eq!(version.as_deref(), Some("1.2.3"));
    }

    #[test]
    fn compare_versions_behind() {
        match compare_versions("0.233.1", "0.234.0") {
            DriftStatus::Behind { current, latest } => {
                assert_eq!(current, "0.233.1");
                assert_eq!(latest, "0.234.0");
            }
            _ => panic!("expected Behind"),
        }
    }

    #[test]
    fn compare_versions_up_to_date() {
        assert!(matches!(compare_versions("1.0.0", "1.0.0"), DriftStatus::UpToDate));
    }

    #[test]
    fn expand_tilde_works() {
        std::env::set_var("HOME", "/home/test");
        assert_eq!(expand_tilde("~/.local/bin/zed"), "/home/test/.local/bin/zed");
    }
}
```

### Integration Tests

Gated behind `#[cfg(feature = "integration")]`:
- Verify `provenance.toml` parsing with the real file on the dev machine
- Verify version extraction for known-installed binaries
- Verify GitHub API returns parseable response (requires network)

---

## Operator Workflow

1. Operator installs a binary via curl/gh-release
2. Operator adds an entry to `~/.local/state/harness/provenance.toml`
3. Russell reads the registry on next extended probe cycle
4. `russell digest` reports provenance status
5. `russell jack` includes provenance drift in SOAP Objective
6. Jack may recommend: "zed is 3 versions behind; consider updating"

Russell **never** updates the binary or edits `provenance.toml`.
The operator acts on Jack's recommendation manually.

---

## Future: Auto-population (Phase 3+)

A future enhancement could auto-populate `provenance.toml` by
scanning `~/.local/bin` and attempting `--version` extraction.
This is explicitly deferred because:
- High false-positive risk (not all binaries support `--version`)
- Operator ownership model is cleaner (JR-7: auditable)
- Auto-population would require Russell to WRITE the file (violates JR-2 in current phase)

When this lands, it would be a new CLI verb: `russell provenance scan`
that proposes additions for operator review (observe > recommend > act).
