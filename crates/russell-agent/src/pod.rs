// SPDX-License-Identifier: MIT OR Apache-2.0
//! Russell agent pod — sovereign agent entity.

use std::path::PathBuf;
use uuid::Uuid;

use crate::persona::AgentPersona;
use crate::lifecycle::{PodLifecycleState, LifecycleResult, validate_transition};
use crate::cns::CnsEmitter;
use crate::artifacts::ArtifactStore;

/// Unique pod identifier (UUID-based).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PodID(pub String);

impl PodID {
    /// Generate a new pod ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }
    
    /// Parse from string.
    pub fn parse(s: &str) -> Option<Self> {
        if Uuid::parse_str(s).is_ok() || s.starts_with("russell-") {
            Some(Self(s.to_string()))
        } else {
            None
        }
    }
}

impl Default for PodID {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for PodID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Sentinel handle for background probe collection.
pub struct SentinelHandle {
    /// Tokio task handle
    _handle: tokio::task::JoinHandle<()>,
    /// Sentinel is running
    running: bool,
}

impl SentinelHandle {
    /// Create a new sentinel handle.
    pub fn new(handle: tokio::task::JoinHandle<()>) -> Self {
        Self {
            _handle: handle,
            running: true,
        }
    }
    
    /// Stop the sentinel.
    pub fn stop(&mut self) {
        self._handle.abort();
        self.running = false;
    }
    
    /// Check if sentinel is running.
    pub fn is_running(&self) -> bool {
        self.running
    }
}

/// Russell agent pod — implements full agent pod interface.
///
/// The pod manages:
/// - Lifecycle states (Populated → Registered → Activated → Deactivated)
/// - Agent persona (charter, capabilities, rights, responsibilities)
/// - CNS span emission
/// - Memory artifact storage
/// - ACP server integration
pub struct RussellPod {
    /// Unique pod identifier.
    id: PodID,
    
    /// Agent persona (charter, capabilities, etc.).
    persona: AgentPersona,
    
    /// Current lifecycle state.
    state: PodLifecycleState,
    
    /// CNS span emitter.
    cns_emitter: CnsEmitter,
    
    /// Memory artifact store.
    artifacts: ArtifactStore,
    
    /// ACP server (if activated).
    acp_server: Option<AcpServerHandle>,
    
    /// Sentinel handle (if activated).
    sentinel: Option<SentinelHandle>,
}

/// ACP server handle.
pub struct AcpServerHandle {
    /// Server is running
    running: bool,
}

impl AcpServerHandle {
    /// Create a new ACP server handle.
    pub fn new() -> Self {
        Self { running: true }
    }
    
    /// Stop the ACP server.
    pub fn stop(&mut self) {
        self.running = false;
    }
    
    /// Check if server is running.
    pub fn is_running(&self) -> bool {
        self.running
    }
}

impl Default for AcpServerHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl RussellPod {
    /// Create a new Russell pod (Populated state).
    ///
    /// Loads the agent persona from the template crate and validates
    /// the charter, capabilities, and configuration.
    pub fn new(template_crate_path: PathBuf) -> LifecycleResult<Self> {
        tracing::info!("Creating Russell agent pod");
        
        // Load agent persona from template crate
        let persona = AgentPersona::load(&template_crate_path)?;
        
        // Create pod ID
        let id = PodID::new();
        
        // Initialize CNS emitter
        let cns_emitter = CnsEmitter::new(&id, &persona);
        
        // Initialize artifact store
        let artifacts = ArtifactStore::new(template_crate_path.join("artifacts"));
        
        let pod = Self {
            id,
            persona,
            state: PodLifecycleState::Populated,
            cns_emitter,
            artifacts,
            acp_server: None,
            sentinel: None,
        };
        
        // Emit CNS span for pod population
        pod.cns_emitter.emit_populated();
        
        tracing::info!(pod_id = %pod.id, "Russell agent pod created (Populated)");
        
        Ok(pod)
    }
    
    /// Get the pod ID.
    pub fn id(&self) -> &PodID {
        &self.id
    }
    
    /// Get the current lifecycle state.
    pub fn state(&self) -> &PodLifecycleState {
        &self.state
    }
    
    /// Get the agent persona.
    pub fn persona(&self) -> &AgentPersona {
        &self.persona
    }
    
    /// Register the pod with hKask ACP runtime (Registered state).
    ///
    /// This method:
    /// 1. Validates state transition (Populated → Registered)
    /// 2. Registers Russell as an agent with ACP runtime
    /// 3. Receives capability token
    /// 4. Transitions to Registered state
    /// 5. Emits CNS span
    pub async fn register(&mut self, runtime: &russell_acp_server::AcpServer) -> LifecycleResult<()> {
        tracing::info!(pod_id = %self.id, "Registering Russell agent pod");
        
        // Validate state transition
        validate_transition(&self.state, &PodLifecycleState::Registered)?;
        
        // TODO: Implement actual ACP runtime registration
        // For now, we accept the AcpServer as proof of registration
        // In production, this would:
        // 1. Connect to hKask ACP runtime
        // 2. Send registration request with persona capabilities
        // 3. Receive capability token
        // 4. Store token for future requests
        let _ = runtime; // Suppress unused warning
        
        // Transition to Registered state
        self.state = PodLifecycleState::Registered;
        
        // Emit CNS span
        self.cns_emitter.emit_registered();
        
        tracing::info!(pod_id = %self.id, "Russell agent pod registered");
        
        Ok(())
    }
    
    /// Activate the pod (Activated state).
    ///
    /// This method:
    /// 1. Validates state transition (Registered → Activated)
    /// 2. Starts the sentinel timer (5-min cadence)
    /// 3. Starts the ACP server (stdio transport)
    /// 4. Transitions to Activated state
    /// 5. Emits CNS span
    pub async fn activate(&mut self) -> LifecycleResult<()> {
        tracing::info!(pod_id = %self.id, "Activating Russell agent pod");
        
        // Validate state transition
        validate_transition(&self.state, &PodLifecycleState::Activated)?;
        
        // Start sentinel (5-min cadence)
        let sentinel_handle = self.start_sentinel().await?;
        self.sentinel = Some(sentinel_handle);
        
        // Start ACP server (stdio transport)
        let acp_handle = self.start_acp_server().await?;
        self.acp_server = Some(acp_handle);
        
        // Transition to Activated state
        self.state = PodLifecycleState::Activated;
        
        // Emit CNS span
        self.cns_emitter.emit_activated();
        
        tracing::info!(pod_id = %self.id, "Russell agent pod activated");
        
        Ok(())
    }
    
    /// Deactivate the pod (Deactivated state).
    ///
    /// This method:
    /// 1. Validates state transition (Activated → Deactivated)
    /// 2. Stops the ACP server
    /// 3. Stops the sentinel
    /// 4. Revokes capabilities
    /// 5. Transitions to Deactivated state
    /// 6. Emits CNS span
    pub async fn deactivate(&mut self) -> LifecycleResult<()> {
        tracing::info!(pod_id = %self.id, "Deactivating Russell agent pod");
        
        // Validate state transition
        validate_transition(&self.state, &PodLifecycleState::Deactivated)?;
        
        // Stop ACP server
        if let Some(mut server) = self.acp_server.take() {
            server.stop();
            tracing::debug!("ACP server stopped");
        }
        
        // Stop sentinel
        if let Some(mut sentinel) = self.sentinel.take() {
            sentinel.stop();
            tracing::debug!("Sentinel stopped");
        }
        
        // TODO: Revoke capabilities
        // In production, this would:
        // 1. Notify hKask ACP runtime of deactivation
        // 2. Invalidate capability tokens
        // 3. Clean up any pending operations
        
        // Transition to Deactivated state
        self.state = PodLifecycleState::Deactivated;
        
        // Emit CNS span
        self.cns_emitter.emit_deactivated();
        
        tracing::info!(pod_id = %self.id, "Russell agent pod deactivated");
        
        Ok(())
    }
    
    /// Start the sentinel timer (5-min cadence).
    async fn start_sentinel(&self) -> LifecycleResult<SentinelHandle> {
        tracing::info!("Starting sentinel timer");
        
        // TODO: Implement actual sentinel loop
        // For now, spawn a dummy task
        let handle = tokio::spawn(async move {
            // Sentinel loop would go here
            // Every 5 minutes:
            // 1. Collect host probes
            // 2. Evaluate thresholds
            // 3. Write to journal
            // 4. Emit CNS spans
            tracing::debug!("Sentinel running (stub)");
        });
        
        Ok(SentinelHandle::new(handle))
    }
    
    /// Start the ACP server (stdio transport).
    async fn start_acp_server(&self) -> LifecycleResult<AcpServerHandle> {
        tracing::info!("Starting ACP server");
        
        // TODO: Implement actual ACP server startup
        // For now, return a handle
        // In production, this would:
        // 1. Create ACP server with persona
        // 2. Bind to stdio transport
        // 3. Start serving requests
        // 4. Handle session creation, messages, etc.
        
        Ok(AcpServerHandle::new())
    }
    
    /// Get the CNS emitter.
    pub fn cns_emitter(&self) -> &CnsEmitter {
        &self.cns_emitter
    }
    
    /// Get the artifact store.
    pub fn artifacts(&self) -> &ArtifactStore {
        &self.artifacts
    }
    
    /// Check if pod is activated.
    pub fn is_activated(&self) -> bool {
        self.state == PodLifecycleState::Activated
    }
    
    /// Check if pod is registered.
    pub fn is_registered(&self) -> bool {
        self.state == PodLifecycleState::Registered || self.state == PodLifecycleState::Activated
    }
}

impl Drop for RussellPod {
    fn drop(&mut self) {
        // Ensure cleanup on drop
        if self.state == PodLifecycleState::Activated || self.state == PodLifecycleState::Registered {
            tracing::warn!(pod_id = %self.id, state = ?self.state, "Russell pod dropped without proper deactivation");
            
            // Attempt graceful cleanup
            if let Some(mut sentinel) = self.sentinel.take() {
                sentinel.stop();
            }
            if let Some(mut server) = self.acp_server.take() {
                server.stop();
            }
            
            // Emit deactivation span (best effort)
            self.cns_emitter.emit_deactivated();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;
    
    fn create_test_persona(dir: &PathBuf) {
        let persona_content = r#"
agent:
  name: "test-russell"
  type: "bot"
  version: "0.1.0"
  
charter:
  description: "Test agent"
  editor: "test"
  
capabilities:
  items:
    - "tool:test"
    
responsibilities:
  items:
    - "test: responsibility"
"#;
        fs::write(dir.join("agent_persona.yaml"), persona_content).unwrap();
    }
    
    #[tokio::test]
    async fn test_pod_creation() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path().to_path_buf();
        create_test_persona(&dir);
        
        let pod = RussellPod::new(dir).unwrap();
        assert_eq!(pod.state(), &PodLifecycleState::Populated);
        assert_eq!(pod.persona().name(), "test-russell");
    }
    
    #[tokio::test]
    async fn test_pod_lifecycle() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path().to_path_buf();
        create_test_persona(&dir);
        
        let mut pod = RussellPod::new(dir).unwrap();
        
        // Test Populated → Registered
        // Create a minimal AcpServer for testing
        let handler = russell_acp_server::AcpHandler::new(
            russell_acp_server::JackPersonaProjection::default(),
            russell_acp_server::AcpDispatch::default(),
            russell_acp_server::MacaroonAuth::new(None),
            russell_acp_server::RateLimiter::default(),
        );
        let acp_server = russell_acp_server::AcpServer::new(handler);
        pod.register(&acp_server).await.unwrap();
        assert_eq!(pod.state(), &PodLifecycleState::Registered);
        
        // Test Registered → Activated
        pod.activate().await.unwrap();
        assert_eq!(pod.state(), &PodLifecycleState::Activated);
        assert!(pod.is_activated());
        
        // Test Activated → Deactivated
        pod.deactivate().await.unwrap();
        assert_eq!(pod.state(), &PodLifecycleState::Deactivated);
        assert!(!pod.is_activated());
    }
    
    #[tokio::test]
    async fn test_invalid_state_transition() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path().to_path_buf();
        create_test_persona(&dir);
        
        let mut pod = RussellPod::new(dir).unwrap();
        
        // Try to activate without registering (should fail)
        let result = pod.activate().await;
        assert!(result.is_err());
        
        // State should still be Populated
        assert_eq!(pod.state(), &PodLifecycleState::Populated);
    }
}
