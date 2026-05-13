// SPDX-License-Identifier: MIT OR Apache-2.0
//! Network probe compositions.
//!
//! Monitors socket counts from `/proc/net/sockstat` and
//! `/proc/net/sockstat6`. Lightweight, no subprocess needed.

use super::connectors;
use super::tools;

/// Probe: total TCP sockets in use (IPv4).
///
/// From `/proc/net/sockstat`, `TCP: inuse` field.
pub fn net_tcp_connections() -> Option<f64> {
    let content = connectors::read_file_to_string("/proc/net/sockstat")?;
    tools::parse_sockstat(&content, "TCP")
}

/// Probe: total TCP6 sockets in use (IPv6).
///
/// From `/proc/net/sockstat6`, `TCP6: inuse` field.
pub fn net_tcp6_connections() -> Option<f64> {
    let content = connectors::read_file_to_string("/proc/net/sockstat6")?;
    tools::parse_sockstat(&content, "TCP6")
}

/// Return both network probes in a single collection.
pub(crate) fn net_samples() -> Vec<super::Sample> {
    let mut out = Vec::new();
    if let Some(v) = net_tcp_connections() {
        out.push(super::Sample {
            name: "net_tcp_connections".into(),
            value_num: Some(v),
            value_text: None,
            unit: Some("count"),
        });
    }
    if let Some(v) = net_tcp6_connections() {
        out.push(super::Sample {
            name: "net_tcp6_connections".into(),
            value_num: Some(v),
            value_text: None,
            unit: Some("count"),
        });
    }
    out
}

// -- ProbeDescriptor impls --

use super::descriptor::ProbeDescriptor;

pub struct NetTcpConnections;
impl ProbeDescriptor for NetTcpConnections {
    fn name(&self) -> &'static str { "net_tcp_connections" }
    fn unit(&self) -> Option<&'static str> { Some("count") }
    fn collect(&self) -> Option<f64> { net_tcp_connections() }
}

pub struct NetTcp6Connections;
impl ProbeDescriptor for NetTcp6Connections {
    fn name(&self) -> &'static str { "net_tcp6_connections" }
    fn unit(&self) -> Option<&'static str> { Some("count") }
    fn collect(&self) -> Option<f64> { net_tcp6_connections() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn net_tcp_returns_on_linux() {
        if !std::path::Path::new("/proc/net/sockstat").exists() {
            return;
        }
        let val = net_tcp_connections();
        assert!(val.is_some(), "tcp connections should be readable");
        assert!(val.unwrap() >= 0.0);
    }
}
