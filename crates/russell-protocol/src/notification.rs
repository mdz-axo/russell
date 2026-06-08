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

#[cfg(test)]
mod tests {
    use super::*;

    // REQ: ProprioNotification must round-trip through serialization.
    #[test]
    fn proprio_notification_round_trip() {
        let n = ProprioNotification {
            id: "notif-1".to_string(),
            vital: "journal_stall_s".to_string(),
            severity: "warn".to_string(),
            value: serde_json::json!(120),
            threshold: serde_json::json!(60),
            summary: "Journal writer stall exceeded threshold".to_string(),
            timestamp: "2025-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&n).unwrap();
        let back: ProprioNotification = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "notif-1");
        assert_eq!(back.vital, "journal_stall_s");
        assert_eq!(back.severity, "warn");
        assert_eq!(back.value, serde_json::json!(120));
        assert_eq!(back.threshold, serde_json::json!(60));
        assert_eq!(back.summary, "Journal writer stall exceeded threshold");
        assert_eq!(back.timestamp, "2025-01-01T00:00:00Z");
    }

    // REQ: NotificationsResponse must round-trip.
    #[test]
    fn notifications_response_round_trip() {
        let n = ProprioNotification {
            id: "n1".to_string(),
            vital: "timer_drift_s".to_string(),
            severity: "alert".to_string(),
            value: serde_json::json!(5.0),
            threshold: serde_json::json!(2.0),
            summary: "Timer drift".to_string(),
            timestamp: "2025-06-01T12:00:00Z".to_string(),
        };
        let resp = NotificationsResponse {
            notifications: vec![n],
            total: 1,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let back: NotificationsResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back.total, 1);
        assert_eq!(back.notifications.len(), 1);
        assert_eq!(back.notifications[0].vital, "timer_drift_s");
    }

    // REQ: NotificationsResponse with empty list must round-trip.
    #[test]
    fn notifications_response_empty_round_trip() {
        let resp = NotificationsResponse {
            notifications: vec![],
            total: 0,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let back: NotificationsResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back.total, 0);
        assert!(back.notifications.is_empty());
    }

    // REQ: Missing required field on ProprioNotification causes deserialization error.
    #[test]
    fn proprio_notification_missing_vital_fails() {
        let json = r#"{"id":"n1","severity":"warn","value":1,"threshold":2,"summary":"x","timestamp":"t"}"#;
        let result = serde_json::from_str::<ProprioNotification>(json);
        assert!(result.is_err());
    }
}
