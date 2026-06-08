// SPDX-License-Identifier: MIT OR Apache-2.0
//! Proprioception notification types for ACP integration.

use serde::{Deserialize, Serialize};

/// Proprioception notification — pushed to agents when Russell detects
/// degradation in its own health.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProprioNotification {
    /// Notification ID (UUID v4).
    pub id: String,
    /// Vital that breached threshold (e.g., "journal_stall_s").
    pub vital: String,
    /// Severity ("warn", "alert", "critical").
    pub severity: String,
    /// Current value of the vital.
    pub value: serde_json::Value,
    /// Threshold that was breached.
    pub threshold: serde_json::Value,
    /// Human-readable summary.
    pub summary: String,
    /// Timestamp (ISO 8601).
    pub timestamp: String,
}

/// Notifications list response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationsResponse {
    /// Recent proprioception notifications.
    pub notifications: Vec<ProprioNotification>,
    /// Total count of notifications in the time window.
    pub total: usize,
}
