// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell-protocol` — shared ACP/JSON-RPC protocol types.
//!
//! This crate contains the wire types for the Agent Client Protocol (ACP)
//! used between Russell and hKask. Both projects depend on this crate to
//! ensure protocol-level type alignment.
//!
//! ## ACP Protocol Version
//!
//! The current ACP protocol version is [`ACP_VERSION`].
//!
//! ## Type Organization
//!
//! - [`jsonrpc`] — JSON-RPC 2.0 request/response envelopes
//! - [`skill`] — Skill metadata types (SkillInfo, ProbeInfo, etc.)
//! - [`auth`] — Capability token and attenuation types
//! - [`notification`] — Proprioception notification types

#![deny(unsafe_code)]
#![deny(rust_2018_idioms)]
#![warn(missing_docs)]

pub mod auth;
pub mod jsonrpc;
pub mod notification;
pub mod skill;

/// ACP protocol version.
pub const ACP_VERSION: &str = "0.1.0";
