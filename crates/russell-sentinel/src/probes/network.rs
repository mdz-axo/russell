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

/// Marker struct for TCP connections probe.
pub struct NetTcpConnections;
/// Marker struct for TCP6 connections probe.
pub struct NetTcp6Connections;

impl_probe!(NetTcpConnections, "net_tcp_connections", "count", net_tcp_connections);
impl_probe!(NetTcp6Connections, "net_tcp6_connections", "count", net_tcp6_connections);

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
