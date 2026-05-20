// SPDX-License-Identifier: MIT OR Apache-2.0
//! Minimal env-file loader for `~/.config/harness/russell.env`.
//!
//! Russell does not depend on `dotenvy` or similar — JR-1 says
//! the smallest thing that could possibly work. We read a
//! file of `KEY=value` lines, skip comments, and set any var
//! that is **not already set** in the process environment.
//! Existing env vars always win.
//!
//! Called once from the CLI `main` before parsing arguments.
//! All subsequent reads go through `std::env::var`.

use std::fs;
use std::path::Path;

use tracing::{debug, warn};

/// Load an env file at `path` if present. Silent if absent.
/// Never fails visibly; malformed lines are logged at `warn`.
pub fn load_env_file(path: &Path) {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            debug!(path = %path.display(), "no env file; skipping");
            return;
        }
        Err(e) => {
            warn!(path = %path.display(), error = %e, "failed to read env file");
            return;
        }
    };
    let mut loaded = 0usize;
    for (lineno, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            warn!(path = %path.display(), lineno = lineno + 1, "ignoring malformed env line");
            continue;
        };
        let key = k.trim();
        let value = strip_optional_quotes(v.trim());
        if key.is_empty() || value.is_empty() {
            // Skip blank keys or empty values — a blank value in a
            // template file should not mask a real value set elsewhere.
            continue;
        }
        // Existing env var wins — CI/CD and shell overrides matter.
        if std::env::var_os(key).is_some() {
            continue;
        }
        // SAFETY: `set_var` is sound iff no other thread is concurrently
        // reading the environment. `load_env_file` is documented and
        // called from `main` before any `tokio::spawn`.
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var(key, value);
        }
        loaded += 1;
    }
    debug!(path = %path.display(), loaded, "env file loaded");
}

fn strip_optional_quotes(s: &str) -> &str {
    let bytes = s.as_bytes();
    if bytes.len() >= 2
        && ((bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\''))
    {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

/// Load the first env file found in Russell's discovery order.
pub fn load_discovered(
    config_harness_dir: &std::path::Path,
    override_path: Option<&std::path::Path>,
) -> Option<std::path::PathBuf> {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if let Some(p) = override_path {
        candidates.push(p.to_path_buf());
    }
    candidates.push(config_harness_dir.join("russell.env"));
    if let Ok(cwd) = std::env::current_dir()
        && let Some(repo) = find_repo_root(&cwd)
    {
        candidates.push(repo.join(".env"));
    }
    candidates.push(std::path::PathBuf::from(".env"));

    // Load in precedence order (highest first). `load_env_file` already
    // refuses to overwrite already-set vars, so subsequent files only
    // fill in what the higher-precedence file left blank. Empty values
    // in a file are skipped (see \`load_env_file\`), so a template file
    // with a blank key never masks a real value from a later file.
    let mut first_found: Option<std::path::PathBuf> = None;
    for c in candidates {
        if c.exists() {
            load_env_file(&c);
            if first_found.is_none() {
                first_found = Some(c);
            }
        }
    }
    first_found
}

/// Find the first existing env file in Russell's discovery order
/// (read-only — does not load any values).
///
/// Search order matches [`load_discovered`]: config dir first,
/// then repo root, then cwd. Returns `None` if no env file exists.
pub fn find_env_file(config_harness_dir: &Path) -> Option<std::path::PathBuf> {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    candidates.push(config_harness_dir.join("russell.env"));
    if let Ok(cwd) = std::env::current_dir()
        && let Some(repo) = find_repo_root(&cwd)
    {
        candidates.push(repo.join(".env"));
    }
    candidates.push(std::path::PathBuf::from(".env"));
    candidates.into_iter().find(|c| c.exists())
}

/// Walk up from `start` looking for a `Cargo.toml` that declares a
/// `[workspace]`. Returns the directory containing it, or `None`.
fn find_repo_root(start: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut cur: Option<&std::path::Path> = Some(start);
    while let Some(dir) = cur {
        let ct = dir.join("Cargo.toml");
        if ct.exists()
            && let Ok(text) = std::fs::read_to_string(&ct)
            && text.contains("[workspace]")
        {
            return Some(dir.to_path_buf());
        }
        cur = dir.parent();
    }
    None
}

