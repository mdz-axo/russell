// SPDX-License-Identifier: MIT OR Apache-2.0
//! Shared CLI utilities.

use anyhow::Result;

/// Extract a scalar YAML field from a manifest string.
pub fn extract_yaml_field(manifest: &str, key: &str) -> Option<String> {
    for line in manifest.lines() {
        if let Some(rest) = line.trim().strip_prefix(&format!("{key}:")) {
            return Some(rest.trim().to_string());
        }
    }
    None
}

/// Extract a YAML list from a manifest string.
pub fn extract_yaml_list(manifest: &str, key: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut in_section = false;
    for line in manifest.lines() {
        if line.trim_start().starts_with(&format!("{key}:")) {
            in_section = true;
            continue;
        }
        if in_section {
            if let Some(item) = line.trim().strip_prefix("- ") {
                items.push(item.trim().to_string());
            } else if line.trim().starts_with(|c: char| c.is_alphabetic()) {
                break;
            }
        }
    }
    items
}
