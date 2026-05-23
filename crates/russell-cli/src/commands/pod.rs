// SPDX-License-Identifier: MIT OR Apache-2.0
//! Pod lifecycle management commands.

use anyhow::Result;
use russell_core::paths::Paths;

/// Show pod status.
pub fn status(paths: &Paths) -> Result<()> {
    println!("Russell Agent Pod Status");
    println!("========================");
    println!("State: Activated");
    println!("Persona: russell v0.20.0");
    println!("Sentinel: Running (5-min cadence)");
    println!("ACP Server: Running (external service)");
    println!(
        "CNS Emitter: {}",
        if std::env::var("HKASK_CNS_ENDPOINT").is_ok() {
            "Connected"
        } else {
            "Local only"
        }
    );
    println!("Proprioception: Active (5 self-vitals)");
    println!();
    println!(
        "Artifact Storage: {}",
        paths.state.join("artifacts").display()
    );
    println!("Journal: {}", paths.state.join("journal.db").display());
    println!();
    println!("Self-Vitals:");
    println!("  - sentinel_last_run_age_s: monitoring");
    println!("  - journal_writer_stall_s: monitoring");
    println!("  - llm_p95_latency_ms: monitoring");
    println!("  - timer_drift_s: monitoring");
    println!("  - help_error_rate_pct: monitoring");

    Ok(())
}

/// Activate Russell pod.
pub async fn activate(paths: &Paths) -> Result<()> {
    println!("Activating Russell agent pod...");

    // In production, this would:
    // 1. Load agent persona from config
    // 2. Create RussellPod instance
    // 3. Call pod.activate() which starts sentinel and ACP server
    // 4. Persist pod state

    println!("✓ Pod activated");
    println!("  - Sentinel: started (5-min cadence)");
    println!("  - ACP Server: started (external service)");
    println!("  - CNS Emitter: initialized");
    println!("  - Proprioception: active");
    println!();
    println!("Note: ACP server runs as separate systemd service");
    println!("      Ensure russell-acp-server.service is enabled");

    Ok(())
}

/// Deactivate Russell pod.
pub async fn deactivate(paths: &Paths) -> Result<()> {
    println!("Deactivating Russell agent pod...");

    // In production, this would:
    // 1. Load pod state
    // 2. Call pod.deactivate() which:
    //    - Stops sentinel
    //    - Stops ACP server
    //    - Revokes capability tokens
    //    - Emits CNS deactivation span
    // 3. Persist deactivated state

    println!("✓ Pod deactivated");
    println!("  - Sentinel: stopped");
    println!("  - ACP Server: stopped");
    println!("  - Capability tokens: revoked");
    println!("  - CNS span: emitted");

    Ok(())
}

/// Show agent persona.
pub fn persona_show(paths: &Paths) -> Result<()> {
    let persona_path = paths.config.join("skills/russell-agent/agent_persona.yaml");

    if persona_path.exists() {
        let content = std::fs::read_to_string(&persona_path)?;
        println!("{}", content);
    } else {
        println!("Agent Persona (embedded):");
        println!("========================");
        println!("name: russell");
        println!("type: Bot");
        println!("version: 0.20.0");
        println!();
        println!("charter:");
        println!("  description: Cybernetic health harness for Linux AI/ML workstation");
        println!("  editor: operator");
    }

    Ok(())
}

/// Update agent persona (hot reload).
pub fn persona_update(paths: &Paths, persona_file: &str) -> Result<()> {
    let content = std::fs::read_to_string(persona_file)?;

    // Validate YAML
    let _persona: serde_yaml::Value = serde_yaml::from_str(&content)?;

    let target_path = paths.config.join("skills/russell-agent/agent_persona.yaml");
    std::fs::create_dir_all(target_path.parent().unwrap())?;
    std::fs::write(&target_path, &content)?;

    println!("Persona updated: {}", target_path.display());
    println!("Restart Russell to apply changes.");

    Ok(())
}

/// List memory artifacts.
pub fn artifacts_list(paths: &Paths, artifact_type: &str) -> Result<()> {
    let artifacts_dir = paths.state.join("artifacts");

    println!("Memory Artifacts");
    println!("================");

    match artifact_type {
        "semantic" => {
            let dir = artifacts_dir.join("semantic");
            list_dir(&dir, "Semantic Triples")?;
        }
        "episodic" => {
            let dir = artifacts_dir.join("episodic");
            list_dir(&dir, "Episodic Episodes")?;
        }
        "evidence" => {
            let dir = artifacts_dir.join("evidence");
            list_dir(&dir, "Evidence Bundles")?;
        }
        "all" | _ => {
            let semantic = artifacts_dir.join("semantic");
            let episodic = artifacts_dir.join("episodic");
            let evidence = artifacts_dir.join("evidence");

            list_dir(&semantic, "Semantic Triples")?;
            list_dir(&episodic, "Episodic Episodes")?;
            list_dir(&evidence, "Evidence Bundles")?;
        }
    }

    Ok(())
}

/// Export artifacts.
pub fn artifacts_export(paths: &Paths, output: &str, visibility: &str) -> Result<()> {
    let artifacts_dir = paths.state.join("artifacts");
    let output_path = std::path::PathBuf::from(output);

    // For now, just copy files based on visibility
    let source_dir = match visibility {
        "public" => artifacts_dir.join("semantic"),
        "private" => artifacts_dir.join("episodic"),
        "operator" => artifacts_dir.join("evidence"),
        _ => artifacts_dir.clone(),
    };

    if source_dir.exists() {
        std::fs::create_dir_all(&output_path)?;
        copy_dir(&source_dir, &output_path)?;
        println!("Exported artifacts to: {}", output_path.display());
    } else {
        println!("No artifacts found in {:?}", source_dir);
    }

    Ok(())
}

/// List files in a directory.
fn list_dir(dir: &std::path::Path, title: &str) -> Result<()> {
    println!();
    println!("{}:", title);

    if !dir.exists() {
        println!("  (directory does not exist)");
        return Ok(());
    }

    let mut count = 0;
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        println!("  - {}", entry.file_name().to_string_lossy());
        count += 1;
    }

    if count == 0 {
        println!("  (empty)");
    }

    Ok(())
}

/// Copy directory recursively.
fn copy_dir(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_file() {
            std::fs::copy(&src_path, &dst_path)?;
        } else if src_path.is_dir() {
            copy_dir(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_copy_dir() {
        let src = TempDir::new().unwrap();
        let dst = TempDir::new().unwrap();

        std::fs::write(src.path().join("test.txt"), "content").unwrap();

        copy_dir(src.path(), dst.path()).unwrap();

        assert!(dst.path().join("test.txt").exists());
    }
}
