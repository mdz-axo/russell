// SPDX-License-Identifier: MIT OR Apache-2.0
//! Connector layer — I/O boundary functions.
//!
//! Every function in this module performs a side effect (filesystem
//! read, subprocess invocation, syscall). No transformation logic
//! lives here. Each connector is instrumented with a OKH span.
//!
//! See `docs/specifications/audit-crate.md` Layer 3 for the
//! instrumentation discipline.

use std::fs;

/// Read a file to string. Returns `None` on any I/O error.
///
/// OKH: `okh.connector.fs.target=<path>`, `okh.connector.fs.success`
#[tracing::instrument(
    level = "trace",
    fields(
        okh.connector.fs.target = path,
        okh.connector.fs.success,
    )
)]
pub fn read_file_to_string(path: &str) -> Option<String> {
    let result = fs::read_to_string(path).ok();
    tracing::Span::current().record("okh.connector.fs.success", result.is_some());
    result
}

/// List numeric PID entries in `/proc`. Returns `None` if `/proc`
/// is unreadable.
///
/// OKH: `okh.connector.fs.target=/proc`, `okh.connector.fs.success`
#[tracing::instrument(
    level = "trace",
    fields(
        okh.connector.fs.target = "/proc",
        okh.connector.fs.success,
    )
)]
pub fn list_proc_pids() -> Option<Vec<u32>> {
    let entries = fs::read_dir("/proc").ok()?;
    let mut pids = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.chars().all(|c| c.is_ascii_digit())
            && let Ok(pid) = name_str.parse::<u32>()
        {
            pids.push(pid);
        }
    }
    tracing::Span::current().record("okh.connector.fs.success", true);
    Some(pids)
}

/// Read `/proc/<pid>/stat` for a single PID. Returns `None` on
/// any I/O error (including the process exiting between listing
/// and reading).
#[tracing::instrument(
    level = "trace",
    fields(
        okh.connector.fs.target = tracing::field::Empty,
        okh.connector.fs.success,
    )
)]
pub fn read_proc_stat(pid: u32) -> Option<String> {
    let path = format!("/proc/{pid}/stat");
    tracing::Span::current().record("okh.connector.fs.target", path.as_str());
    let result = fs::read_to_string(&path).ok();
    tracing::Span::current().record("okh.connector.fs.success", result.is_some());
    result
}

/// Run a command and capture its stdout regardless of exit code.
/// Returns `None` only if the command fails to start.
///
/// Use for commands like `systemctl is-system-running` where
/// the exit code IS the signal. For commands that should
/// succeed, use `run_command_stdout_always` but check stderr.
///
/// OKH: `okh.connector.cmd.target=<program>`, `okh.connector.cmd.success`
#[tracing::instrument(
    level = "trace",
    fields(
        okh.connector.cmd.target = tracing::field::Empty,
        okh.connector.cmd.success,
    )
)]
pub fn run_command_stdout_always(cmd: &[&str]) -> Option<String> {
    let program = cmd.first()?;
    tracing::Span::current().record("okh.connector.cmd.target", *program);
    let output = std::process::Command::new(program)
        .args(&cmd[1..])
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    tracing::Span::current().record("okh.connector.cmd.success", output.status.success());
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}
