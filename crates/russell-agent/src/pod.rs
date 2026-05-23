// SPDX-License-Identifier: MIT OR Apache-2.0
//! Russell agent pod — sovereign agent entity.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::artifacts::ArtifactStore;
use crate::cns::CnsEmitter;
use crate::lifecycle::{LifecycleError, LifecycleResult, PodLifecycleState, validate_transition};
use crate::persona::AgentPersona;
use russell_core::RuleSet;
use russell_core::journal::{JournalReader, JournalWriter};
use russell_sentinel::probes;

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
/// - Sentinel probe collection with journal persistence
/// - EWMA baseline evaluation for threshold detection
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

    /// Capability tokens from hKask ACP runtime.
    capability_tokens: Vec<russell_acp_server::CapabilityToken>,

    /// Journal writer for sample persistence (sentinel-owned).
    journal_writer: Option<Arc<Mutex<JournalWriter>>>,

    /// Journal reader for baseline evaluation.
    journal_reader: Option<JournalReader>,

    /// Rule set for threshold evaluation.
    rules: RuleSet,
}

/// ACP server handle.
pub struct AcpServerHandle {
    /// Tokio task handle
    _handle: tokio::task::JoinHandle<()>,
    /// Server is running
    running: bool,
}

impl AcpServerHandle {
    /// Create a new ACP server handle.
    pub fn new(handle: tokio::task::JoinHandle<()>) -> Self {
        Self {
            _handle: handle,
            running: true,
        }
    }

    /// Stop the ACP server.
    pub fn stop(&mut self) {
        self._handle.abort();
        self.running = false;
    }

    /// Check if server is running.
    pub fn is_running(&self) -> bool {
        self.running
    }
}

impl Default for AcpServerHandle {
    fn default() -> Self {
        Self::new(tokio::spawn(async {}))
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

        // Initialize journal writer
        let journal_path = template_crate_path.join("journal.db");
        let journal_writer = JournalWriter::open(&journal_path)
            .map_err(|e| LifecycleError::SentinelError(e.to_string()))?;
        let journal_reader = journal_writer.reader();

        // Initialize rule set
        let rules = RuleSet::with_defaults();

        let pod = Self {
            id,
            persona,
            state: PodLifecycleState::Populated,
            cns_emitter,
            artifacts,
            acp_server: None,
            sentinel: None,
            capability_tokens: Vec::new(),
            journal_writer: Some(Arc::new(Mutex::new(journal_writer))),
            journal_reader: Some(journal_reader),
            rules,
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
    pub async fn register(
        &mut self,
        runtime: &russell_acp_server::AcpServer,
    ) -> LifecycleResult<()> {
        tracing::info!(pod_id = %self.id, "Registering Russell agent pod");

        // Validate state transition
        validate_transition(&self.state, &PodLifecycleState::Registered)?;

        // Register with hKask ACP runtime
        // 1. Send registration request with persona capabilities
        // 2. Receive capability token
        // 3. Store token for future requests
        let capabilities = self.persona.capabilities.as_slice();
        tracing::info!(
            pod_id = %self.id,
            capabilities = ?capabilities,
            "Registering with ACP runtime"
        );

        // In production, this would connect to hKask ACP runtime and receive a token
        // For now, we accept the AcpServer as proof of registration and create a stub token
        let _ = runtime; // Suppress unused warning

        // Create a capability token (stub - in production would receive from hKask)
        let token = russell_acp_server::CapabilityToken {
            token: "russell-pod-token".to_string(),
            capabilities: vec!["acp:session".to_string()],
            attenuations: Vec::new(),
            expires_at: None,
            issuer: "russell-pod".to_string(),
        };
        self.capability_tokens.push(token);
        tracing::info!(pod_id = %self.id, "Capability token received");

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

        // Revoke capabilities
        // 1. Notify hKask ACP runtime of deactivation
        // 2. Invalidate capability tokens
        // 3. Clean up any pending operations
        tracing::info!(pod_id = %self.id, "Revoking {} capability tokens", self.capability_tokens.len());

        // Clear capability tokens (in production, would notify hKask first)
        self.capability_tokens.clear();

        // Transition to Deactivated state
        self.state = PodLifecycleState::Deactivated;

        // Emit CNS span
        self.cns_emitter.emit_deactivated();

        tracing::info!(pod_id = %self.id, "Russell agent pod deactivated");

        Ok(())
    }

    /// Start the sentinel timer (5-min cadence).
    async fn start_sentinel(&self) -> LifecycleResult<SentinelHandle> {
        tracing::info!("Starting sentinel timer (5-min cadence)");

        let pod_id = self.id.clone();
        let cns_emitter = self.cns_emitter.clone();
        let journal_writer = self.journal_writer.clone();
        let journal_reader = self.journal_reader.clone();
        let _rules = self.rules.clone();

        let handle = tokio::spawn(async move {
            // Sentinel loop — 5-minute observation cadence
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(300));

            // Baseline refresh counter — compute baselines every 288 cycles (24 hours)
            let mut cycle_count: u64 = 0;
            const BASELINE_REFRESH_INTERVAL: u64 = 288; // 288 * 5 min = 24 hours

            loop {
                interval.tick().await;
                cycle_count = cycle_count.wrapping_add(1);

                tracing::debug!(pod_id = %pod_id, cycle = cycle_count, "Sentinel cycle starting");

                // 1. Collect host probes using russell-sentinel
                // This collects: CPU, memory, disk, GPU, systemd, process, network probes
                let samples = probes::collect();

                if !samples.is_empty() {
                    // 2. Log probe collection summary
                    let probe_names: Vec<&str> = samples.iter().map(|s| s.name.as_str()).collect();
                    tracing::debug!(
                        pod_id = %pod_id,
                        probes = ?probe_names,
                        "Collected {} probes",
                        samples.len()
                    );

                    // 3. Write to journal
                    if let Some(ref writer) = journal_writer {
                        let ts = russell_core::time::now_unix();
                        if let Ok(writer_guard) = writer.lock() {
                            for sample in &samples {
                                let _ = writer_guard.append_sample(
                                    ts,
                                    russell_core::event::Scope::Host,
                                    &sample.name,
                                    sample.value_num,
                                    sample.value_text.as_deref(),
                                    sample.unit,
                                );
                            }
                            tracing::debug!(pod_id = %pod_id, "Wrote {} samples to journal", samples.len());
                        }
                    }

                    // 4. Compute baselines periodically (daily)
                    if cycle_count % BASELINE_REFRESH_INTERVAL == 0 {
                        tracing::info!(pod_id = %pod_id, "Computing EWMA baselines (30-day window)");
                        if let Some(ref reader) = journal_reader {
                            match reader.compute_baselines(30) {
                                Ok(baselines) => {
                                    if let Some(ref writer) = journal_writer {
                                        if let Ok(writer_guard) = writer.lock() {
                                            for baseline in &baselines {
                                                let _ = writer_guard.upsert_baseline(
                                                    &baseline.probe,
                                                    russell_core::event::Scope::Host,
                                                    baseline.ewma_mean,
                                                    baseline.ewma_var,
                                                    baseline.p50,
                                                    baseline.p95,
                                                    baseline.p99,
                                                );
                                            }
                                            tracing::info!(
                                                pod_id = %pod_id,
                                                "Computed {} baselines (30-day EWMA)",
                                                baselines.len()
                                            );
                                        }
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!(pod_id = %pod_id, error = %e, "Failed to compute baselines");
                                }
                            }
                        }
                    }

                    // 5. Run proprioception — Russell watches Russell (JR-5)
                    if let (Some(writer), Some(reader)) = (&journal_writer, &journal_reader) {
                        if let Ok(writer_guard) = writer.lock() {
                            match russell_proprio::run_once(&writer_guard, reader) {
                                Ok(result) => {
                                    // Log self-vitals
                                    tracing::debug!(
                                        pod_id = %pod_id,
                                        age_s = ?result.age_s,
                                        journal_stall_s = ?result.journal_stall_s,
                                        llm_p95_ms = ?result.llm_p95_latency_ms,
                                        timer_drift_s = ?result.timer_drift_s,
                                        help_error_rate_pct = ?result.help_error_rate_pct,
                                        "Proprioception complete"
                                    );

                                    // Emit CNS span for proprioception
                                    cns_emitter
                                        .emit_probe_executed("proprioception-cycle", "russell");

                                    // Log any threshold breaches
                                    if result.event_emitted {
                                        tracing::warn!(
                                            pod_id = %pod_id,
                                            sentinel_severity = ?result.severity,
                                            journal_stall_severity = ?result.journal_stall_severity,
                                            llm_severity = ?result.llm_p95_severity,
                                            timer_severity = ?result.timer_drift_severity,
                                            help_error_severity = ?result.help_error_rate_severity,
                                            "Proprioception threshold breach detected"
                                        );
                                    }

                                    // 5b. Run reflex arcs — recommend automatic actions
                                    let mut reflex = russell_proprio::ReflexArc::new();
                                    reflex.evaluate(&result);

                                    if !reflex.actions().is_empty() {
                                        tracing::warn!(
                                            pod_id = %pod_id,
                                            actions = reflex.actions().len(),
                                            "Reflex arcs triggered"
                                        );

                                        // Log recommended actions (Phase 2A: detection-only)
                                        if let Ok(writer_guard) = writer.lock() {
                                            let _ = reflex.log_actions(&writer_guard);

                                            for action in reflex.actions() {
                                                tracing::warn!(
                                                    pod_id = %pod_id,
                                                    action = action.action_id,
                                                    risk = ?action.risk,
                                                    trigger = action.trigger,
                                                    "Reflex arc: {}",
                                                    action.description
                                                );
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!(pod_id = %pod_id, error = %e, "Proprioception failed");
                                }
                            }
                        }
                    }

                    // 5. Evaluate thresholds against EWMA baselines
                    let mut breaches = 0;
                    if let Some(ref reader) = journal_reader {
                        // Read baselines for comparison
                        match reader.read_baselines() {
                            Ok(baselines) => {
                                for sample in &samples {
                                    let Some(value) = sample.value_num else {
                                        continue;
                                    };

                                    // Find matching baseline
                                    if let Some(baseline) =
                                        baselines.iter().find(|b| b.probe == sample.name)
                                    {
                                        // Check against p95 (primary threshold)
                                        if let Some(p95) = baseline.p95 {
                                            let ratio = value / p95;
                                            if ratio > 10.0 {
                                                breaches += 1;
                                                tracing::warn!(
                                                    pod_id = %pod_id,
                                                    probe = %sample.name,
                                                    value = value,
                                                    p95 = p95,
                                                    ratio = ratio,
                                                    "CRITICAL: value >10× p95 baseline"
                                                );
                                            } else if ratio > 3.0 {
                                                breaches += 1;
                                                tracing::warn!(
                                                    pod_id = %pod_id,
                                                    probe = %sample.name,
                                                    value = value,
                                                    p95 = p95,
                                                    ratio = ratio,
                                                    "Significant: value >3× p95 baseline"
                                                );
                                            } else if ratio > 1.5 {
                                                breaches += 1;
                                                tracing::debug!(
                                                    pod_id = %pod_id,
                                                    probe = %sample.name,
                                                    value = value,
                                                    p95 = p95,
                                                    ratio = ratio,
                                                    "Mild anomaly: value >1.5× p95 baseline"
                                                );
                                            }
                                        }
                                    } else {
                                        // No baseline available — use simple threshold
                                        if value > 90.0 {
                                            breaches += 1;
                                            tracing::warn!(
                                                pod_id = %pod_id,
                                                probe = %sample.name,
                                                value = value,
                                                "Threshold breach detected (>90%) — no baseline available"
                                            );
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!(pod_id = %pod_id, error = %e, "Failed to read baselines");
                            }
                        }
                    } else {
                        // No journal reader — use simple threshold check
                        for sample in &samples {
                            if let Some(value) = sample.value_num {
                                if value > 90.0 {
                                    breaches += 1;
                                    tracing::warn!(
                                        pod_id = %pod_id,
                                        probe = %sample.name,
                                        value = value,
                                        "Threshold breach detected (>90%)"
                                    );
                                }
                            }
                        }
                    }

                    // 6. Emit CNS spans
                    cns_emitter.emit_probe_executed("sentinel-cycle", "russell");

                    if breaches > 0 {
                        tracing::warn!(pod_id = %pod_id, "Sentinel cycle complete with {} breaches", breaches);
                    } else {
                        tracing::debug!(pod_id = %pod_id, "Sentinel cycle complete (no breaches)");
                    }
                } else {
                    tracing::warn!(pod_id = %pod_id, "Sentinel cycle: no probes collected");
                }
            }
        });

        Ok(SentinelHandle::new(handle))
    }

    /// Start the ACP server (stdio transport).
    ///
    /// Note: In production, the ACP server runs as a separate systemd service
    /// (`russell-acp-server.service`). This stub creates a handle for tracking
    /// purposes only. The sentinel owns the journal writer.
    async fn start_acp_server(&self) -> LifecycleResult<AcpServerHandle> {
        tracing::info!("ACP server handle created (external service)");

        // In production deployment, the ACP server runs as a separate process:
        // - systemd service: russell-acp-server.service
        // - Binary: russell-acp-server
        // - Communicates with hKask via stdio JSON-RPC
        //
        // For now, create a stub handle that tracks the "running" state.
        let handle = tokio::spawn(async move {
            // Stub task - just stays alive
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(300)).await;
            }
        });

        Ok(AcpServerHandle::new(handle))
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
        if self.state == PodLifecycleState::Activated || self.state == PodLifecycleState::Registered
        {
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
    use std::fs;
    use tempfile::TempDir;

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
