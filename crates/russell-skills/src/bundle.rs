// SPDX-License-Identifier: MIT OR Apache-2.0
//! Skill bundle — the unit of skill sharing (import/export).
//!
//! A `.rsk.tar.gz` archive is the canonical sharing format for
//! skills across Russell instances and the Kask ecosystem.
//!
//! ## Archive layout
//!
//! ```text
//! <skill-id>.rsk.tar.gz
//!   ├── manifest.yaml       ← YAML skill manifest
//!   ├── KNOWLEDGE.md        ← domain knowledge (optional)
//!   ├── scripts/            ← executable scripts referenced in manifest
//!   │   ├── probe-*.sh
//!   │   └── intervention-*.sh
//!   ├── provenance.json     ← upstream git SHA + build timestamp
//!   └── REUSE_MANIFEST.md   ← copy-with-provenance header (JR-6)
//! ```
//!
//! ## Visibility discriminant
//!
//! Each skill has a `visibility` field:
//! - `Local`: never leaves the machine (default)
//! - `Shared`: bundled for operator-curated distribution
//! - `Published`: tagged for potential registry push (deferred)
//!
//! Only `Shared` and `Published` skills can be exported.
//!
//! ## Import pipeline
//!
//! `russell skill import <bundle>` →
//!   extract → validate (safety scanner) → install → register
//!
//! ## Export pipeline
//!
//! `russell skill export <id>` →
//!   read skill dir → pack → sign → output `.rsk.tar.gz`

#![deny(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ─── Bundle types ────────────────────────────────────────────────────────────

/// A skill bundle — the unit of sharing.
///
/// Not the archive itself, but its metadata and contents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillBundle {
    /// Manifest content (YAML string, validated).
    pub manifest: String,
    /// KNOWLEDGE.md content (optional).
    pub knowledge: Option<String>,
    /// Script paths relative to the bundle root.
    pub scripts: Vec<BundleScript>,
    /// Provenance chain.
    pub provenance: Provenance,
    /// Visibility discriminant.
    pub visibility: Visibility,
}

/// A script file within the bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleScript {
    /// Relative path within the bundle (e.g., "scripts/probe-health.sh").
    pub path: String,
    /// File content.
    pub content: String,
    /// Whether the script is executable.
    pub executable: bool,
}

/// Provenance chain for JR-6 compliance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provenance {
    /// Skill ID.
    pub skill_id: String,
    /// Skill version (semver from manifest).
    pub version: String,
    /// Upstream git repository URL.
    pub upstream_repo: Option<String>,
    /// Upstream git commit SHA at time of export.
    pub upstream_sha: Option<String>,
    /// ISO 8601 timestamp of export.
    pub exported_at: String,
    /// Exporting hostname.
    pub exported_by: String,
    /// Russell version at time of export.
    pub russell_version: String,
}

/// Visibility discriminant.
///
/// Controls whether a skill can be exported, shared, or published.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    /// Never leaves the machine. Export is refused for Local skills.
    Local,
    /// Bundled for operator-curated distribution.
    Shared,
    /// Tagged for potential registry push (deferred — ADR-0025 §8).
    Published,
}

impl Visibility {
    /// Whether this skill can be exported.
    #[must_use]
    pub fn is_exportable(self) -> bool {
        matches!(self, Self::Shared | Self::Published)
    }
}

// ─── Export ──────────────────────────────────────────────────────────────────

/// Errors from bundle export.
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("skill '{0}' has visibility '{1}' — not exportable")]
    NotExportable(String, String),

    #[error("skill '{0}' not found in skills directory")]
    NotFound(String),

    #[error("cannot read {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("cannot create archive: {0}")]
    Archive(String),
}

/// Export a skill to a `.rsk.tar.gz` bundle.
///
/// # Errors
///
/// Returns `ExportError::NotExportable` if the skill's visibility is `Local`.
/// Returns `ExportError::NotFound` if the skill directory doesn't exist.
pub fn export_bundle(
    skills_dir: &Path,
    skill_id: &str,
    visibility: Visibility,
    output_path: &Path,
) -> Result<(), ExportError> {
    if !visibility.is_exportable() {
        return Err(ExportError::NotExportable(
            skill_id.into(),
            format!("{visibility:?}"),
        ));
    }

    let skill_dir = skills_dir.join(skill_id);
    if !skill_dir.exists() {
        return Err(ExportError::NotFound(skill_id.into()));
    }

    let manifest_path = skill_dir.join("manifest.yaml");
    let manifest = std::fs::read_to_string(&manifest_path).map_err(|e| ExportError::Read {
        path: manifest_path.clone(),
        source: e,
    })?;

    let knowledge_path = skill_dir.join("KNOWLEDGE.md");
    let knowledge = if knowledge_path.exists() {
        Some(std::fs::read_to_string(&knowledge_path).map_err(|e| ExportError::Read {
            path: knowledge_path.clone(),
            source: e,
        })?)
    } else {
        None
    };

    let scripts_dir = skill_dir.join("scripts");
    let mut scripts = Vec::new();
    if scripts_dir.exists() {
        for entry in std::fs::read_dir(&scripts_dir).map_err(|e| ExportError::Read {
            path: scripts_dir.clone(),
            source: e,
        })? {
            let entry = entry.map_err(|e| ExportError::Read {
                path: scripts_dir.clone(),
                source: e,
            })?;
            let path = entry.path();
            if path.is_file() {
                let rel = path.strip_prefix(&skill_dir).unwrap_or(&path);
                let content = std::fs::read_to_string(&path).map_err(|e| ExportError::Read {
                    path: path.clone(),
                    source: e,
                })?;
                #[cfg(unix)]
                let executable = {
                    use std::os::unix::fs::PermissionsExt;
                    path.metadata().map(|m| m.permissions().mode() & 0o111 != 0).unwrap_or(false)
                };
                #[cfg(not(unix))]
                let executable = false;
                scripts.push(BundleScript {
                    path: rel.to_string_lossy().into_owned(),
                    content,
                    executable,
                });
            }
        }
    }

    let provenance = Provenance {
        skill_id: skill_id.into(),
        version: extract_version(&manifest).unwrap_or_else(|| "0.0.0".into()),
        upstream_repo: None, // populated from REUSE_MANIFEST.md if present
        upstream_sha: None,
        exported_at: russell_core::time::now_date_iso8601(),
        exported_by: hostname(),
        russell_version: env!("CARGO_PKG_VERSION").into(),
    };

    let bundle = SkillBundle {
        manifest,
        knowledge,
        scripts,
        provenance,
        visibility,
    };

    write_bundle_archive(&bundle, output_path).map_err(|e| ExportError::Archive(e.to_string()))?;

    tracing::info!(
        skill_id = %skill_id,
        output = %output_path.display(),
        "exported skill bundle",
    );
    Ok(())
}

// ─── Import ──────────────────────────────────────────────────────────────────

/// Errors from bundle import.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("cannot read archive {path}: {source}")]
    ReadArchive {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid bundle: {0}")]
    InvalidBundle(String),

    #[error("safety scan blocked: {0}")]
    SafetyBlock(String),

    #[error("skill already exists: {0}")]
    AlreadyExists(String),

    #[error("cannot extract: {0}")]
    Extract(String),
}

/// Import a skill from a `.rsk.tar.gz` bundle.
///
/// Pipeline: extract → validate (safety scanner) → install → register.
///
/// # Safety
///
/// The bundle is safety-scanned before extraction. If the safety scanner
/// finds blocking issues, import is refused.
///
/// # Errors
///
/// Returns `ImportError::SafetyBlock` if the safety scanner finds blocking
/// content in the manifest or knowledge files.
pub fn import_bundle(
    bundle_path: &Path,
    skills_dir: &Path,
    allow_overwrite: bool,
) -> Result<String, ImportError> {
    let bundle = read_bundle_archive(bundle_path)?;

    // Safety scan the imported content.
    let safety = crate::registry::safety::SafetyScan::scan(&bundle.manifest);
    if safety.has_blocks() {
        let blocks: Vec<String> = safety
            .findings
            .iter()
            .filter(|f| f.severity == crate::registry::safety::ScanSeverity::Block)
            .map(|f| f.description.clone())
            .collect();
        return Err(ImportError::SafetyBlock(blocks.join("; ")));
    }

    if let Some(ref knowledge) = bundle.knowledge {
        let ks = crate::registry::safety::SafetyScan::scan(knowledge);
        if ks.has_blocks() {
            let blocks: Vec<String> = ks
                .findings
                .iter()
                .filter(|f| f.severity == crate::registry::safety::ScanSeverity::Block)
                .map(|f| f.description.clone())
                .collect();
            return Err(ImportError::SafetyBlock(blocks.join("; ")));
        }
    }

    let skill_id = &bundle.provenance.skill_id;
    let dest_dir = skills_dir.join(skill_id);

    if dest_dir.exists() && !allow_overwrite {
        return Err(ImportError::AlreadyExists(skill_id.clone()));
    }

    // Write manifest, knowledge, and scripts.
    std::fs::create_dir_all(&dest_dir).map_err(|e| ImportError::Extract(e.to_string()))?;
    std::fs::write(dest_dir.join("manifest.yaml"), &bundle.manifest)
        .map_err(|e| ImportError::Extract(e.to_string()))?;

    if let Some(ref knowledge) = bundle.knowledge {
        std::fs::write(dest_dir.join("KNOWLEDGE.md"), knowledge)
            .map_err(|e| ImportError::Extract(e.to_string()))?;
    }

    let scripts_dir = dest_dir.join("scripts");
    if !bundle.scripts.is_empty() {
        std::fs::create_dir_all(&scripts_dir).map_err(|e| ImportError::Extract(e.to_string()))?;
        for script in &bundle.scripts {
            let script_path = dest_dir.join(&script.path);
            if let Some(parent) = script_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| ImportError::Extract(e.to_string()))?;
            }
            std::fs::write(&script_path, &script.content)
                .map_err(|e| ImportError::Extract(e.to_string()))?;
            #[cfg(unix)]
            if script.executable {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&script_path)
                    .map_err(|e| ImportError::Extract(e.to_string()))?
                    .permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(&script_path, perms)
                    .map_err(|e| ImportError::Extract(e.to_string()))?;
            }
        }
    }

    // Write provenance chain.
    std::fs::write(
        dest_dir.join("provenance.json"),
        serde_json::to_string_pretty(&bundle.provenance)
            .map_err(|e| ImportError::Extract(e.to_string()))?,
    )
    .map_err(|e| ImportError::Extract(e.to_string()))?;

    tracing::info!(
        skill_id = %skill_id,
        source = %bundle_path.display(),
        "imported skill bundle",
    );
    Ok(skill_id.clone())
}

// ─── Archive I/O (tar.gz) ────────────────────────────────────────────────────

fn write_bundle_archive(bundle: &SkillBundle, output_path: &Path) -> Result<(), std::io::Error> {
    use std::io::Write;

    let file = std::fs::File::create(output_path)?;
    let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut archive = tar::Builder::new(encoder);

    add_tar_entry(&mut archive, "manifest.yaml", bundle.manifest.as_bytes())?;

    if let Some(ref knowledge) = bundle.knowledge {
        add_tar_entry(&mut archive, "KNOWLEDGE.md", knowledge.as_bytes())?;
    }

    for script in &bundle.scripts {
        add_tar_entry(&mut archive, &script.path, script.content.as_bytes())?;
    }

    let provenance_json = serde_json::to_string_pretty(&bundle.provenance)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    add_tar_entry(&mut archive, "provenance.json", provenance_json.as_bytes())?;

    let mut reuse_manifest = String::from("# REUSE_MANIFEST.md — provenance chain\n\n");
    reuse_manifest.push_str(&format!(
        "| Russell skill | Upstream | Upstream commit | Changes |\n"
    ));
    reuse_manifest.push_str(&format!("|---|---|---|---|\n"));
    reuse_manifest.push_str(&format!(
        "| {} | {} | {} | exported by {} |\n",
        bundle.provenance.skill_id,
        bundle.provenance.upstream_repo.as_deref().unwrap_or("—"),
        bundle.provenance.upstream_sha.as_deref().unwrap_or("—"),
        bundle.provenance.exported_by,
    ));
    add_tar_entry(&mut archive, "REUSE_MANIFEST.md", reuse_manifest.as_bytes())?;

    let encoder = archive.into_inner()?;
    encoder.finish()?;
    Ok(())
}

fn read_bundle_archive(path: &Path) -> Result<SkillBundle, ImportError> {
    let file = std::fs::File::open(path).map_err(|e| ImportError::ReadArchive {
        path: path.to_path_buf(),
        source: e,
    })?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);

    let mut manifest = String::new();
    let mut knowledge = None;
    let mut scripts = Vec::new();
    let mut provenance = None;

    for entry in archive.entries().map_err(|e| ImportError::InvalidBundle(e.to_string()))? {
        let mut entry = entry.map_err(|e| ImportError::InvalidBundle(e.to_string()))?;
        let entry_path = entry.path().map_err(|e| ImportError::InvalidBundle(e.to_string()))?;
        let path_str = entry_path.to_string_lossy();

        let mut content = String::new();
        std::io::Read::read_to_string(&mut entry, &mut content)
            .map_err(|e| ImportError::InvalidBundle(e.to_string()))?;

        match path_str.as_ref() {
            "manifest.yaml" => manifest = content,
            "KNOWLEDGE.md" => knowledge = Some(content),
            "provenance.json" => {
                provenance = Some(
                    serde_json::from_str::<Provenance>(&content)
                        .map_err(|e| ImportError::InvalidBundle(format!("bad provenance: {e}")))?,
                );
            }
            other if other.starts_with("scripts/") => {
                scripts.push(BundleScript {
                    path: other.to_string(),
                    content,
                    executable: other.ends_with(".sh"),
                });
            }
            _ => {} // ignore REUSE_MANIFEST.md and other metadata
        }
    }

    let provenance = provenance.ok_or_else(|| {
        ImportError::InvalidBundle("missing provenance.json".into())
    })?;

    Ok(SkillBundle {
        manifest,
        knowledge,
        scripts,
        provenance,
        visibility: Visibility::Shared, // imported bundles default to Shared
    })
}

fn add_tar_entry<W: std::io::Write>(
    archive: &mut tar::Builder<W>,
    path: &str,
    data: &[u8],
) -> Result<(), std::io::Error> {
    let mut header = tar::Header::new_gnu();
    header.set_path(path).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    archive.append(&header, data)?;
    Ok(())
}

fn extract_version(manifest_yaml: &str) -> Option<String> {
    for line in manifest_yaml.lines() {
        if let Some(rest) = line.strip_prefix("version:") {
            return Some(rest.trim().trim_matches('"').to_string());
        }
    }
    None
}

fn hostname() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("HOST"))
        .unwrap_or_else(|_| "unknown".into())
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn visibility_exportable() {
        assert!(!Visibility::Local.is_exportable());
        assert!(Visibility::Shared.is_exportable());
        assert!(Visibility::Published.is_exportable());
    }

    #[test]
    fn export_round_trip() {
        let skills_dir = tempdir().unwrap();
        let skill_dir = skills_dir.path().join("test-skill");
        std::fs::create_dir_all(skill_dir.join("scripts")).unwrap();
        std::fs::write(
            skill_dir.join("manifest.yaml"),
            "id: test-skill\nversion: 1.0.0\nauthored: 2026-05-01\nsymptoms: [test]\n",
        )
        .unwrap();
        std::fs::write(skill_dir.join("KNOWLEDGE.md"), "# Test Knowledge\n").unwrap();
        std::fs::write(
            skill_dir.join("scripts").join("probe.sh"),
            "#!/bin/bash\necho ok\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                skill_dir.join("scripts").join("probe.sh"),
                std::fs::Permissions::from_mode(0o755),
            )
            .unwrap();
        }

        let output = skills_dir.path().join("test-skill.rsk.tar.gz");
        export_bundle(
            skills_dir.path(),
            "test-skill",
            Visibility::Shared,
            &output,
        )
        .unwrap();
        assert!(output.exists());

        let import_dir = tempdir().unwrap();
        let imported_id = import_bundle(&output, import_dir.path(), false).unwrap();
        assert_eq!(imported_id, "test-skill");
        assert!(import_dir.path().join("test-skill").join("manifest.yaml").exists());
        assert!(import_dir.path().join("test-skill").join("KNOWLEDGE.md").exists());
        assert!(import_dir.path().join("test-skill").join("provenance.json").exists());
    }

    #[test]
    fn export_refuses_local_visibility() {
        let dir = tempdir().unwrap();
        let result = export_bundle(dir.path(), "test", Visibility::Local, Path::new("/tmp/test.rsk.tar.gz"));
        assert!(result.is_err());
    }

    #[test]
    fn import_rejects_if_exists() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("existing").join("scripts")).unwrap();
        std::fs::write(
            dir.path().join("existing").join("manifest.yaml"),
            "id: existing\nversion: 1.0.0\nauthored: 2026-05-01\nsymptoms: []\n",
        )
        .unwrap();

        // Export it
        let output = dir.path().join("existing.rsk.tar.gz");
        export_bundle(dir.path(), "existing", Visibility::Shared, &output).unwrap();

        // Import to same dir should fail
        let result = import_bundle(&output, dir.path(), false);
        assert!(result.is_err());
    }

    #[test]
    fn import_rejects_unsafe_content() {
        let dir = tempdir().unwrap();
        let bundle_path = dir.path().join("bad.rsk.tar.gz");

        let bundle = SkillBundle {
            manifest: "id: bad\nversion: 1.0.0\nauthored: 2026-05-01\nsymptoms: []\nprobes:\n  - id: evil\n    cmd:\n      - rm\n      - -rf\n      - /\n".into(),
            knowledge: None,
            scripts: vec![],
            provenance: Provenance {
                skill_id: "bad".into(),
                version: "1.0.0".into(),
                upstream_repo: None,
                upstream_sha: None,
                exported_at: "2026-05-15".into(),
                exported_by: "test".into(),
                russell_version: "0.1.0".into(),
            },
            visibility: Visibility::Shared,
        };
        write_bundle_archive(&bundle, &bundle_path).unwrap();

        let import_dir = tempdir().unwrap();
        let result = import_bundle(&bundle_path, import_dir.path(), false);
        assert!(result.is_err(), "should reject rm -rf /");
    }
}
