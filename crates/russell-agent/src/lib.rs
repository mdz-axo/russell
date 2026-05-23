// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell-agent` — Russell as a sovereign agent pod.
//!
//! This crate implements Russell as a first-class agent entity with:
//! - **Agent persona** — Charter, capabilities, rights, responsibilities
//! - **Lifecycle states** — Populated → Registered → Activated → Deactivated
//! - **CNS integration** — Span emission for observability
//! - **Memory artifacts** — Semantic/episodic storage
//! - **ACP interface** — Bidirectional communication with hKask
//!
//! # Architecture
//!
//! Russell is an **external ACP agent** that implements the full agent pod interface:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    Russell Agent Pod                         │
//! │  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
//! │  │ AgentPod     │  │ AgentPersona │  │ Lifecycle        │  │
//! │  │ - state      │  │ - charter    │  │ - Populated      │  │
//! │  │ - persona    │  │ - capabilities│  │ - Registered     │  │
//! │  │ - lifecycle  │  │ - rights     │  │ - Activated      │  │
//! │  │              │  │ - resp       │  │ - Deactivated    │  │
//! │  └──────────────┘  └──────────────┘  └──────────────────┘  │
//! │                                                              │
//! │  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
//! │  │ CnsEmitter   │  │ AcpServer    │  │ Sentinel         │  │
//! │  │ - spans      │  │ - sessions   │  │ - probes         │  │
//! │  │ - events     │  │ - dispatch   │  │ - cadence        │  │
//! │  └──────────────┘  └──────────────┘  └──────────────────┘  │
//! └─────────────────────────────────────────────────────────────┘
//!                            │
//!                            │ ACP (stdio)
//!                            ▼
//!                    hKask Platform
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! // Example usage (types not yet implemented):
//! use russell_agent::{RussellPod, AgentPersona};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Load template crate (skills directory)
//!     let template_crate_path = std::path::PathBuf::from("~/.local/share/harness/skills");
//!     
//!     // Create pod (Populated state)
//!     let mut pod = RussellPod::new(&template_crate_path)?;
//!     
//!     // Register with hKask ACP runtime (Registered state)
//!     // let runtime = AcpRuntime::connect("stdio").await?;
//!     // pod.register(&runtime).await?;
//!     
//!     // Activate pod (Activated state)
//!     // pod.activate().await?;
//!     
//!     // Pod is now running: sentinel probing, ACP serving
//!     // ...
//!     
//!     // Deactivate when done (Deactivated state)
//!     // pod.deactivate().await?;
//!     
//!     Ok(())
//! }
//! ```
//!
//! # Lifecycle States
//!
//! | State | Description |
//! |-------|-------------|
//! | **Populated** | Crate loaded, persona validated |
//! | **Registered** | ACP runtime registration complete |
//! | **Activated** | Sentinel running, ACP serving |
//! | **Deactivated** | Capabilities revoked, cleanup pending |
//!
//! # CNS Spans
//!
//! Russell emits the following CNS spans:
//! - `cns.russell.populated` — Pod populated
//! - `cns.russell.registered` — Pod registered
//! - `cns.russell.activated` — Pod activated
//! - `cns.russell.deactivated` — Pod deactivated
//! - `cns.russell.probe.executed` — Probe executed
//! - `cns.russell.skill.dispatch` — Skill dispatched
//! - `cns.russell.llm.escalation` — LLM escalation
//!
//! # Feature Flags
//!
//! - `cns` — Enable CNS span emission (requires hKask crates)

#![deny(unsafe_code)]
#![deny(rust_2018_idioms)]
#![warn(missing_docs)]

pub mod pod;
pub mod persona;
pub mod lifecycle;
pub mod cns;
pub mod artifacts;

// Re-export main types for convenience.
pub use pod::{RussellPod, PodID};
pub use lifecycle::{PodLifecycleState, LifecycleError, LifecycleResult};
pub use persona::{AgentPersona, AgentType, AgentCharter, AgentCapabilities};
pub use cns::{CnsEmitter, CnsSpan};
pub use artifacts::{ArtifactStore, ArtifactType, ArtifactVisibility};
