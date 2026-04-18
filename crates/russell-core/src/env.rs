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

/// Keys Russell recognises and documents
/// ([`../../../docs/specifications/PERSISTENCE_CATALOG.md`](../../../docs/specifications/PERSISTENCE_CATALOG.md) §2.5).
pub const KNOWN_KEYS: &[&str] = &[
    "OPENROUTER_API_KEY",
    "RUSSELL_DOCTOR_MODEL",
    "RUSSELL_DOCTOR_BACKEND",
    "RUSSELL_DOCTOR_BASE_URL",
    "RUSSELL_DRY_RUN",
    "RUSSELL_LOG",
    "RUST_LOG",
];

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
        if key.is_empty() {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn with_sandbox_env<F: FnOnce()>(f: F) {
        // Tests touch process-wide env; keep them serial by using unique
        // keys per test.
        f();
    }

    #[test]
    fn missing_file_is_silent() {
        with_sandbox_env(|| {
            let tmp = tempfile::tempdir().unwrap();
            load_env_file(&tmp.path().join("absent.env"));
        });
    }

    #[test]
    fn loads_keys_but_respects_existing_env() {
        let tmp = tempfile::tempdir().unwrap();
        let f = tmp.path().join("russell.env");
        fs::write(
            &f,
            "# comment\nRUSSELL_TEST_LOAD_A=one\nRUSSELL_TEST_LOAD_B=\"two\"\n",
        )
        .unwrap();
        // Pre-set B so we can verify it is NOT overwritten.
        // SAFETY: test runs in isolation; no other threads touch env here.
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("RUSSELL_TEST_LOAD_B", "pre-set");
        }
        load_env_file(&f);
        assert_eq!(std::env::var("RUSSELL_TEST_LOAD_A").unwrap(), "one");
        assert_eq!(std::env::var("RUSSELL_TEST_LOAD_B").unwrap(), "pre-set");
        // Clean up.
        // SAFETY: test runs in isolation.
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
        fs::write(
            &f,
            "RUSSELL_TEST_LOAD_C=ok\nno_equals_sign_here\nRUSSELL_TEST_LOAD_D='fine'\n",
        )
        .unwrap();
        load_env_file(&f);
        assert_eq!(std::env::var("RUSSELL_TEST_LOAD_C").unwrap(), "ok");
        assert_eq!(std::env::var("RUSSELL_TEST_LOAD_D").unwrap(), "fine");
        // SAFETY: test runs in isolation.
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("RUSSELL_TEST_LOAD_C");
            std::env::remove_var("RUSSELL_TEST_LOAD_D");
        }
    }
}
