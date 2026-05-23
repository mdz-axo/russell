// SPDX-License-Identifier: MIT OR Apache-2.0
//! Agent persona — YAML-parsed identity and charter.

use std::path::Path;
use serde::{Deserialize, Serialize};

/// Agent type enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentType {
    /// Bot — automated agent with defined capabilities
    Bot,
    /// Replicant — human-like agent with episodic memory
    Replicant,
}

/// Agent capabilities manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCapabilities {
    /// List of capability strings (e.g., "tool:system:probe")
    #[serde(default)]
    pub items: Vec<String>,
}

impl AgentCapabilities {
    /// Create empty capabilities.
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }
    
    /// Get capabilities as slice.
    pub fn as_slice(&self) -> &[String] {
        &self.items
    }
    
    /// Check if capability is granted.
    pub fn has(&self, capability: &str) -> bool {
        self.items.iter().any(|c| c == capability)
    }
}

impl Default for AgentCapabilities {
    fn default() -> Self {
        Self::new()
    }
}

/// Agent rights declaration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentRights {
    /// Read rights
    #[serde(default)]
    pub read: Vec<String>,
    /// Write rights
    #[serde(default)]
    pub write: Vec<String>,
}

/// Agent responsibilities declaration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentResponsibilities {
    /// List of responsibility strings
    #[serde(default)]
    pub items: Vec<String>,
}

/// Agent visibility settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentVisibility {
    /// Default visibility for artifacts
    #[serde(default = "default_visibility")]
    pub default: String,
    /// Override for episodic memory
    #[serde(default)]
    pub episodic_override: Option<String>,
}

fn default_visibility() -> String {
    "private".to_string()
}

/// Agent charter — purpose and scope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCharter {
    /// Description of agent purpose
    pub description: String,
    /// Editor (who can modify this agent)
    pub editor: String,
}

/// Agent persona — YAML-parsed identity and charter.
///
/// Loaded from `agent_persona.yaml` in the template crate root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPersona {
    /// Agent metadata
    pub agent: AgentMeta,
    /// Agent charter (purpose and scope)
    pub charter: AgentCharter,
    /// Agent capabilities
    pub capabilities: AgentCapabilities,
    /// Agent rights
    #[serde(default)]
    pub rights: AgentRights,
    /// Agent responsibilities
    pub responsibilities: AgentResponsibilities,
    /// Agent visibility settings
    #[serde(default)]
    pub visibility: AgentVisibility,
}

/// Agent metadata — name, type, version, and identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMeta {
    /// Agent name (e.g., "russell")
    pub name: String,
    /// Agent type (Bot or Replicant)
    #[serde(rename = "type")]
    pub agent_type: AgentType,
    /// Agent version (semver)
    pub version: String,
    /// WebID for decentralized identity
    #[serde(default)]
    pub webid: Option<String>,
}

impl AgentPersona {
    /// Load agent persona from template crate path.
    pub fn load(template_crate_path: &Path) -> Result<Self, PersonaError> {
        let persona_path = template_crate_path.join("agent_persona.yaml");
        
        if !persona_path.exists() {
            return Err(PersonaError::NotFound(persona_path));
        }
        
        let content = std::fs::read_to_string(&persona_path)
            .map_err(|e| PersonaError::IoError(e))?;
        
        let persona: AgentPersona = serde_yaml::from_str(&content)
            .map_err(|e| PersonaError::ParseError(e))?;
        
        // Validate required fields
        persona.validate()?;
        
        Ok(persona)
    }
    
    /// Validate the persona (required fields, consistency).
    pub fn validate(&self) -> Result<(), PersonaError> {
        if self.agent.name.is_empty() {
            return Err(PersonaError::Validation("agent.name is required".to_string()));
        }
        
        if self.charter.description.is_empty() {
            return Err(PersonaError::Validation("charter.description is required".to_string()));
        }
        
        if self.capabilities.items.is_empty() {
            return Err(PersonaError::Validation("capabilities must have at least one item".to_string()));
        }
        
        if self.responsibilities.items.is_empty() {
            return Err(PersonaError::Validation("responsibilities must have at least one item".to_string()));
        }
        
        Ok(())
    }
    
    /// Get the agent name.
    pub fn name(&self) -> &str {
        &self.agent.name
    }
    
    /// Get the agent type.
    pub fn agent_type(&self) -> AgentType {
        self.agent.agent_type
    }
    
    /// Get the agent version.
    pub fn version(&self) -> &str {
        &self.agent.version
    }
    
    /// Get the WebID (if set).
    pub fn webid(&self) -> Option<&str> {
        self.agent.webid.as_deref()
    }
}

impl Default for AgentPersona {
    fn default() -> Self {
        Self {
            agent: AgentMeta {
                name: "unknown".to_string(),
                agent_type: AgentType::Bot,
                version: "0.0.0".to_string(),
                webid: None,
            },
            charter: AgentCharter {
                description: String::new(),
                editor: String::new(),
            },
            capabilities: AgentCapabilities::new(),
            rights: AgentRights::default(),
            responsibilities: AgentResponsibilities::default(),
            visibility: AgentVisibility::default(),
        }
    }
}

/// Agent persona errors.
#[derive(Debug, thiserror::Error)]
pub enum PersonaError {
    /// Persona file not found
    #[error("persona file not found: {0}")]
    NotFound(std::path::PathBuf),
    
    /// IO error reading persona
    #[error("IO error: {0}")]
    IoError(std::io::Error),
    
    /// YAML parse error
    #[error("YAML parse error: {0}")]
    ParseError(serde_yaml::Error),
    
    /// Validation error
    #[error("validation error: {0}")]
    Validation(String),
}
