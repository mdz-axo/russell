// SPDX-License-Identifier: MIT OR Apache-2.0
//! Connector layer — I/O boundary functions.
//!
//! Every function in this module performs a side effect (filesystem
//! read, subprocess invocation, syscall). No transformation logic
//! lives here. Each connector is instrumented with a CTHA span.
//!
//! See `docs/specifications/audit-crate.md` Layer 3 for the
//! instrumentation discipline.

use std::fs;

/// Read a file to string. Returns `None` on any I/O error.
///
/// CTHA: `ctha.connector.fs.target=<path>`, `ctha.connector.fs.success`
#[tracing::instrument(
    level = "trace",
    fields(
        ctha.connector.fs.target = path,
        ctha.connector.fs.success,
    )
)]
pub fn read_file_to_string(path: &str) -> Option<String> {
    let result = fs::read_to_string(path).ok();
    tracing::Span::current().record("ctha.connector.fs.success", result.is_some());
    result
}
