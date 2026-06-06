// SPDX-License-Identifier: MIT OR Apache-2.0
//! Agent persona — YAML-parsed identity and charter.

use serde::{Deserialize, Serialize};
use std::path::Path;

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

        let content = std::fs::read_to_string(&persona_path).map_err(PersonaError::IoError)?;

        let persona: AgentPersona =
            serde_yaml::from_str(&content).map_err(PersonaError::ParseError)?;

        // Validate required fields
        persona.validate()?;

        Ok(persona)
    }

    /// Validate the persona (required fields, consistency).
    pub fn validate(&self) -> Result<(), PersonaError> {
        if self.agent.name.is_empty() {
            return Err(PersonaError::Validation(
                "agent.name is required".to_string(),
            ));
        }

        if self.charter.description.is_empty() {
            return Err(PersonaError::Validation(
                "charter.description is required".to_string(),
            ));
        }

        if self.capabilities.items.is_empty() {
            return Err(PersonaError::Validation(
                "capabilities must have at least one item".to_string(),
            ));
        }

        if self.responsibilities.items.is_empty() {
            return Err(PersonaError::Validation(
                "responsibilities must have at least one item".to_string(),
            ));
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

/// Validate and parse a WebID string.
///
/// WebIDs follow the Solid specification: an HTTPS URI that identifies an agent.
/// This function performs basic structural validation (scheme, authority, no fragment)
/// and returns the string unchanged on success, or a [`PersonaError`] on failure.
///
/// See: <https://www.w3.org/TR/webid/>
pub fn parse_webid(raw: &str) -> Result<String, PersonaError> {
    // Must be an absolute HTTPS URI.
    if !raw.starts_with("https://") {
        return Err(PersonaError::Validation(format!(
            "WebID must use https scheme: {raw}"
        )));
    }

    // Must have an authority (host).
    let after_scheme = &raw[8..]; // skip "https://"
    let authority_end = after_scheme.find('/').unwrap_or(after_scheme.len());
    let authority = &after_scheme[..authority_end];
    if authority.is_empty() || !authority.contains('.') {
        return Err(PersonaError::Validation(format!(
            "WebID has invalid authority: {raw}"
        )));
    }

    // Must not contain a fragment identifier (fragments identify documents, not agents).
    if raw.contains('#') {
        return Err(PersonaError::Validation(format!(
            "WebID must not contain a fragment: {raw}"
        )));
    }

    Ok(raw.to_string())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_webid_accepts_valid() {
        assert_eq!(
            parse_webid("https://example.com/profile/russell").unwrap(),
            "https://example.com/profile/russell"
        );
        assert_eq!(
            parse_webid("https://russell.example.net/").unwrap(),
            "https://russell.example.net/"
        );
    }

    #[test]
    fn parse_webid_rejects_http() {
        assert!(parse_webid("http://example.com/profile").is_err());
    }

    #[test]
    fn parse_webid_rejects_no_host() {
        assert!(parse_webid("https://").is_err());
        assert!(parse_webid("https://localhost").is_err()); // no dot in authority
    }

    #[test]
    fn parse_webid_rejects_fragment() {
        assert!(parse_webid("https://example.com/profile#me").is_err());
    }

    #[test]
    fn parse_webid_rejects_empty() {
        assert!(parse_webid("").is_err());
    }
}
