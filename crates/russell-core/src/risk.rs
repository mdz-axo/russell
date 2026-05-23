// SPDX-License-Identifier: MIT OR Apache-2.0
//! Risk band classification — canonical definition.
//!
//! `RiskBand` is the single source of truth for risk classification
//! across all Russell crates (C4: repetition is a missing primitive).
//! All other crates re-export this type.

use serde::{Deserialize, Serialize};

/// Risk band for an intervention, probe, or tool.
///
/// Ordered from least to most dangerous:
/// `None < Low < Medium < High < Critical`.
#[derive(
    Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
#[serde(rename_all = "snake_case")]
pub enum RiskBand {
    /// Read-only observation — no state mutation.
    #[default]
    None,
    /// Reversible, low impact.
    Low,
    /// Reversible, moderate impact. May require operator consent.
    Medium,
    /// May require reboot or session loss. Requires explicit approval.
    High,
    /// Data loss possible. System-affecting.
    Critical,
}

impl RiskBand {
    /// Human-readable lowercase string for journaling.
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

impl std::fmt::Display for RiskBand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for RiskBand {
    type Err = RiskBandParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(Self::None),
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            "critical" => Ok(Self::Critical),
            other => Err(RiskBandParseError(other.to_owned())),
        }
    }
}

/// Error parsing a [`RiskBand`] from a string.
#[derive(Debug, Clone)]
pub struct RiskBandParseError(String);

impl std::fmt::Display for RiskBandParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown risk band: {:?}", self.0)
    }
}

impl std::error::Error for RiskBandParseError {}
