// SPDX-License-Identifier: MIT OR Apache-2.0
//! Intervention port — hexagonal architecture abstraction for session engine.
//!
//! Defines the port interface for executing approved interventions.
//! Each surface (ACP, CLI, API) provides its own adapter.

use serde_json::Value as JsonValue;

/// Port for executing approved interventions.
///
/// Implementations provide intervention execution with appropriate
/// visibility enforcement and evidence logging.
#[async_trait::async_trait(?Send)]
pub trait InterventionPort {
    /// Execute an approved intervention.
    ///
    /// Returns the execution result on success, or an error message on failure.
    async fn execute(
        &self,
        skill_id: &str,
        intervention_id: &str,
        args: &serde_json::Value,
    ) -> Result<String, String>;
}
