// SPDX-License-Identifier: MIT OR Apache-2.0
//! Risk band classification and hKask tool info.

use serde::{Deserialize, Serialize};

/// Risk band for an intervention or tool.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum RiskBand {
    None,
    Low,
    Medium,
    High,
    Critical,
}

impl RiskBand {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            RiskBand::None => "none",
            RiskBand::Low => "low",
            RiskBand::Medium => "medium",
            RiskBand::High => "high",
            RiskBand::Critical => "critical",
        }
    }
}

impl Default for RiskBand {
    fn default() -> Self {
        Self::None
    }
}

/// Tool info for Jack's nurse persona.
#[derive(Debug, Clone)]
pub struct HKaskToolInfo {
    pub name: String,
    pub risk_band: RiskBand,
    pub input_schema: Option<serde_json::Value>,
}
