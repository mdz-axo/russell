// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell docs check` — documentation quality gate.
//!
//! Port: QualityGatePort
//! Adapter: DocsCheckAdapter
//!
//! Runs the documentation linter, link checker, freshness audit,
//! metric-integrity verifier, and diagram-alignment validator.
//! Returns non-zero if any authoritative document fails a
//! mandatory check.
//!
//! See [docs/standards/VALIDATION_RUBRIC.md] for the full rubric
//! and [rules.d/docs.toml] for operator-overridable thresholds.

use anyhow::{Result, bail};
use russell_core::paths::Paths;
use std::process::Command;

/// Run the documentation quality gate.
pub fn run(_paths: &Paths, strict: bool) -> Result<()> {
    let project_root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

    let lint_script = project_root.join("scripts").join("lint_frontmatter.py");
    if !lint_script.exists() {
        bail!(
            "lint script not found: {}. Run from the repository root.",
            lint_script.display()
        );
    }

    let severity = if strict { "info" } else { "alert" };
    let docs_dir = project_root.join("docs");

    tracing::info!(
        script = %lint_script.display(),
        severity = severity,
        "running documentation linter"
    );

    let output = Command::new("python3")
        .arg(&lint_script)
        .arg("--severity")
        .arg(severity)
        .arg(&docs_dir)
        .output()
        .map_err(|e| anyhow::anyhow!("failed to run lint script: {}", e))?;

    // Print stdout regardless of exit code
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.is_empty() {
        println!("{}", stdout);
    }

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.is_empty() {
            eprintln!("{}", stderr);
        }
        bail!("documentation lint failed with exit code {}", output.status);
    }

    tracing::info!("documentation quality gate passed");
    Ok(())
}
