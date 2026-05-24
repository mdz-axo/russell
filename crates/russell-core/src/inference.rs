// SPDX-License-Identifier: MIT OR Apache-2.0
//! Inference port — unified LLM backend abstraction.
//!
//! Provides a common interface for different LLM inference backends
//! (hKask, Okapi, local models, etc.) following hexagonal architecture.
//!
//! The Nurse pipeline uses this port to request inference without
//! knowing the specific backend implementation.

use crate::error::Result;

/// SOAP (Subjective-Objective-Assessment-Plan) bundle for inference context.
///
/// Structured clinical-style context passed to the LLM for informed responses.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SoapBundle {
    /// Subjective: operator's description of the issue.
    pub subjective: String,
    /// Objective: telemetry data, probe results, metrics.
    pub objective: Vec<SoapObservation>,
    /// Assessment: preliminary analysis or hypotheses.
    pub assessment: Option<String>,
    /// Plan: proposed actions or questions.
    pub plan: Option<Vec<String>>,
}

/// A single observation in the SOAP Objective section.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SoapObservation {
    /// Observation name (e.g., "cpu_usage", "memory_pressure").
    pub name: String,
    /// Observation value.
    pub value: serde_json::Value,
    /// Unit of measurement, if applicable.
    pub unit: Option<String>,
    /// Severity or anomaly indicator.
    pub severity: Option<String>,
}

impl SoapBundle {
    /// Create a new SOAP bundle with only subjective input.
    pub fn new(subjective: impl Into<String>) -> Self {
        Self {
            subjective: subjective.into(),
            objective: Vec::new(),
            assessment: None,
            plan: None,
        }
    }

    /// Add an observation to the objective section.
    pub fn with_observation(mut self, name: impl Into<String>, value: serde_json::Value) -> Self {
        self.objective.push(SoapObservation {
            name: name.into(),
            value,
            unit: None,
            severity: None,
        });
        self
    }

    /// Add an observation with unit and severity.
    pub fn with_full_observation(
        mut self,
        name: impl Into<String>,
        value: serde_json::Value,
        unit: Option<String>,
        severity: Option<String>,
    ) -> Self {
        self.objective.push(SoapObservation {
            name: name.into(),
            value,
            unit,
            severity,
        });
        self
    }

    /// Set the assessment section.
    pub fn with_assessment(mut self, assessment: impl Into<String>) -> Self {
        self.assessment = Some(assessment.into());
        self
    }

    /// Set the plan section.
    pub fn with_plan(mut self, plan: Vec<String>) -> Self {
        self.plan = Some(plan);
        self
    }

    /// Format as a prompt string for the LLM.
    pub fn to_prompt(&self) -> String {
        let mut prompt = format!("## Subjective\n{}\n\n", self.subjective);

        if !self.objective.is_empty() {
            prompt.push_str("## Objective\n");
            for obs in &self.objective {
                let unit_str = obs.unit.as_deref().unwrap_or("");
                let severity_str = obs
                    .severity
                    .as_ref()
                    .map(|s| format!(" [{}]", s))
                    .unwrap_or_default();
                prompt.push_str(&format!(
                    "- {}: {} {}{}\n",
                    obs.name, obs.value, unit_str, severity_str
                ));
            }
            prompt.push('\n');
        }

        if let Some(ref assessment) = self.assessment {
            prompt.push_str(&format!("## Assessment\n{}\n\n", assessment));
        }

        if let Some(ref plan) = self.plan {
            prompt.push_str("## Plan\n");
            for item in plan {
                prompt.push_str(&format!("- {}\n", item));
            }
            prompt.push('\n');
        }

        prompt
    }
}

/// Response from an inference backend.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InferenceResponse {
    /// The generated text response.
    pub text: String,
    /// Backend identifier (e.g., "hkask", "okapi", "local").
    pub backend: String,
    /// Model identifier, if applicable.
    pub model: Option<String>,
    /// Latency in milliseconds.
    pub latency_ms: Option<u64>,
    /// Token usage statistics, if available.
    pub token_usage: Option<TokenUsage>,
    /// Extracted ACTION: proposals from LLM output (hKask returns these).
    #[serde(default)]
    pub actions: Vec<String>,
}

/// Token usage statistics from an inference call.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TokenUsage {
    /// Input tokens consumed.
    pub input_tokens: u64,
    /// Output tokens generated.
    pub output_tokens: u64,
    /// Total tokens.
    pub total_tokens: u64,
}

/// Unified inference port for LLM backends.
///
/// Implementations provide inference capabilities for different backends
/// (hKask REST API, Okapi local inference, etc.).
#[async_trait::async_trait]
pub trait InferencePort: Send + Sync {
    /// Perform inference with a prompt and optional SOAP context.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError`] if the inference fails.
    async fn infer(&self, prompt: &str, context: Option<&SoapBundle>) -> Result<InferenceResponse>;

    /// Check if the backend is available and healthy.
    async fn health_check(&self) -> Result<bool>;

    /// Get the backend identifier (e.g., "hkask", "okapi").
    fn backend_id(&self) -> &str;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_soap_bundle_creation() {
        let bundle = SoapBundle::new("System is slow");
        assert_eq!(bundle.subjective, "System is slow");
        assert!(bundle.objective.is_empty());
        assert!(bundle.assessment.is_none());
        assert!(bundle.plan.is_none());
    }

    #[test]
    fn test_soap_bundle_builder() {
        let bundle = SoapBundle::new("High CPU usage")
            .with_observation("cpu_usage", serde_json::json!(95.5))
            .with_full_observation(
                "memory_usage",
                serde_json::json!(8192),
                Some("MB".to_string()),
                Some("warn".to_string()),
            )
            .with_assessment("CPU saturation detected")
            .with_plan(vec!["Identify top processes".to_string()]);

        assert_eq!(bundle.objective.len(), 2);
        assert!(bundle.assessment.is_some());
        assert!(bundle.plan.is_some());
    }

    #[test]
    fn test_soap_prompt_formatting() {
        let bundle = SoapBundle::new("Test issue")
            .with_observation("metric", serde_json::json!(42))
            .with_assessment("Test assessment");

        let prompt = bundle.to_prompt();
        assert!(prompt.contains("## Subjective"));
        assert!(prompt.contains("Test issue"));
        assert!(prompt.contains("## Objective"));
        assert!(prompt.contains("metric"));
        assert!(prompt.contains("## Assessment"));
        assert!(prompt.contains("Test assessment"));
    }
}
