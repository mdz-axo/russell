// SPDX-License-Identifier: MIT OR Apache-2.0
//! Memory artifact storage for Russell agent.

use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

/// Artifact visibility levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArtifactVisibility {
    /// Public — visible to all agents
    Public,
    /// Private — Russell-only
    Private,
    /// Operator-only — visible to operator
    OperatorOnly,
}

impl std::fmt::Display for ArtifactVisibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Public => write!(f, "public"),
            Self::Private => write!(f, "private"),
            Self::OperatorOnly => write!(f, "operator_only"),
        }
    }
}

/// Artifact types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArtifactType {
    /// Semantic memory triples
    Semantic,
    /// Episodic memory episodes
    Episodic,
    /// Evidence bundles
    Evidence,
    /// Skill artifacts
    Skill,
}

/// Memory artifact store.
pub struct ArtifactStore {
    /// Base directory for artifacts
    base_dir: PathBuf,
}

impl ArtifactStore {
    /// Create a new artifact store.
    pub fn new(base_dir: PathBuf) -> Self {
        // Create base directory and subdirectories
        let _ = std::fs::create_dir_all(&base_dir);
        let _ = std::fs::create_dir_all(base_dir.join("semantic"));
        let _ = std::fs::create_dir_all(base_dir.join("episodic"));
        let _ = std::fs::create_dir_all(base_dir.join("evidence"));
        let _ = std::fs::create_dir_all(base_dir.join("skills"));
        
        Self { base_dir }
    }
    
    /// Get the semantic memory directory.
    pub fn semantic_dir(&self) -> PathBuf {
        self.base_dir.join("semantic")
    }
    
    /// Get the episodic memory directory.
    pub fn episodic_dir(&self) -> PathBuf {
        self.base_dir.join("episodic")
    }
    
    /// Get the evidence bundles directory.
    pub fn evidence_dir(&self) -> PathBuf {
        self.base_dir.join("evidence")
    }
    
    /// Get the skill artifacts directory.
    pub fn skill_dir(&self, skill_id: &str) -> PathBuf {
        self.base_dir.join("skills").join(skill_id)
    }
    
    /// Store a semantic memory triple.
    pub fn store_semantic(&self, date: &str, triples: &str) -> std::io::Result<PathBuf> {
        let dir = self.semantic_dir();
        std::fs::create_dir_all(&dir)?;
        
        let path = dir.join(format!("{}.triples", date));
        std::fs::write(&path, triples)?;
        
        Ok(path)
    }
    
    /// Store an episodic memory episode.
    pub fn store_episodic(&self, date: &str, episode: &str) -> std::io::Result<PathBuf> {
        let dir = self.episodic_dir();
        std::fs::create_dir_all(&dir)?;
        
        let path = dir.join(format!("{}.episodes", date));
        std::fs::write(&path, episode)?;
        
        Ok(path)
    }
    
    /// Store an evidence bundle.
    pub fn store_evidence(&self, date: &str, bundle: &serde_json::Value) -> std::io::Result<PathBuf> {
        let dir = self.evidence_dir().join(date);
        std::fs::create_dir_all(&dir)?;
        
        let path = dir.join("bundle.json");
        let content = serde_json::to_string_pretty(bundle)?;
        std::fs::write(&path, content)?;
        
        Ok(path)
    }
    
    /// Store a skill artifact.
    pub fn store_skill_artifact(&self, skill_id: &str, name: &str, data: &[u8]) -> std::io::Result<PathBuf> {
        let dir = self.skill_dir(skill_id);
        std::fs::create_dir_all(&dir)?;
        
        let path = dir.join(name);
        std::fs::write(&path, data)?;
        
        Ok(path)
    }
    
    /// List semantic memory files.
    pub fn list_semantic(&self) -> std::io::Result<Vec<PathBuf>> {
        self.list_files(&self.semantic_dir())
    }
    
    /// List episodic memory files.
    pub fn list_episodic(&self) -> std::io::Result<Vec<PathBuf>> {
        self.list_files(&self.episodic_dir())
    }
    
    /// List evidence bundles.
    pub fn list_evidence(&self) -> std::io::Result<Vec<PathBuf>> {
        let dir = self.evidence_dir();
        let mut files = Vec::new();
        
        if dir.exists() {
            for entry in std::fs::read_dir(&dir)? {
                let entry = entry?;
                let bundle_dir = entry.path();
                if bundle_dir.is_dir() {
                    let bundle_path = bundle_dir.join("bundle.json");
                    if bundle_path.exists() {
                        files.push(bundle_path);
                    }
                }
            }
        }
        
        Ok(files)
    }
    
    /// Helper to list files in a directory.
    fn list_files(&self, dir: &Path) -> std::io::Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        
        if dir.exists() {
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                files.push(entry.path());
            }
        }
        
        Ok(files)
    }
    
    /// Export artifacts to a tarball.
    pub fn export(&self, output_path: &Path, visibility: ArtifactVisibility) -> std::io::Result<()> {
        // Create output directory if needed
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        // For now, just copy relevant files based on visibility
        let source_dir = match visibility {
            ArtifactVisibility::Public => self.semantic_dir(),
            ArtifactVisibility::Private => self.episodic_dir(),
            ArtifactVisibility::OperatorOnly => self.evidence_dir(),
        };
        
        if source_dir.exists() {
            for entry in std::fs::read_dir(&source_dir)? {
                let entry = entry?;
                let src = entry.path();
                let dst = output_path.join(entry.file_name());
                
                if src.is_file() {
                    std::fs::copy(&src, &dst)?;
                } else if src.is_dir() {
                    // Copy directory recursively
                    self.copy_dir(&src, &dst)?;
                }
            }
        }
        
        Ok(())
    }
    
    /// Helper to copy directory recursively.
    fn copy_dir(&self, src: &Path, dst: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dst)?;
        
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());
            
            if src_path.is_file() {
                std::fs::copy(&src_path, &dst_path)?;
            } else if src_path.is_dir() {
                self.copy_dir(&src_path, &dst_path)?;
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_artifact_store_creation() {
        let temp_dir = TempDir::new().unwrap();
        let store = ArtifactStore::new(temp_dir.path().to_path_buf());
        
        assert!(store.semantic_dir().exists());
        assert!(store.episodic_dir().exists());
        assert!(store.evidence_dir().exists());
    }
    
    #[test]
    fn test_store_semantic() {
        let temp_dir = TempDir::new().unwrap();
        let store = ArtifactStore::new(temp_dir.path().to_path_buf());
        
        let path = store.store_semantic("2026-05-22", "subject predicate object").unwrap();
        assert!(path.exists());
        
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "subject predicate object");
    }
    
    #[test]
    fn test_visibility_display() {
        assert_eq!(ArtifactVisibility::Public.to_string(), "public");
        assert_eq!(ArtifactVisibility::Private.to_string(), "private");
        assert_eq!(ArtifactVisibility::OperatorOnly.to_string(), "operator_only");
    }
}
