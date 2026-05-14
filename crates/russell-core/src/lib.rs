// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell-core` — foundation crate.
//!
//! Provides the cross-cutting types every other Russell crate depends on:
//!
//! - [`paths`] — XDG-aware state / config / data paths. The one place that
//!   answers "where does Russell's data live?"
//! - [`event`] — `harness.event.v1` record type (see
//!   `cybernetic-health-harness.md` §14).
//! - [`profile`] — `russell.profile.v1` machine chart
//!   ([ADR-0006](../../docs/adr/0006-profile-abstraction.md)).
//! - [`journal`] — SQLite journal behind a typed API
//!   ([ADR-0004](../../docs/adr/0004-sqlite-journal.md)).
//! - [`telemetry`] — `tracing` subscriber setup
//!   ([ADR-0010](../../docs/adr/0010-observability-stack.md)).
//!
//! All I/O into `~/.local/state/harness/` routes through this crate.
//! No other crate opens the journal DB directly.

#![deny(unsafe_code)]
#![deny(rust_2018_idioms)]
#![warn(missing_docs)]

pub mod env;
pub mod error;
pub mod event;
pub mod journal;
pub mod paths;
pub mod profile;
pub mod reflex;
pub mod rule;
pub mod schedule;
pub mod telemetry;
pub mod time;

pub use error::{CoreError, Result};
pub use event::{Event, EventId, Severity};
pub use profile::Profile;
pub use reflex::ReflexSet;
pub use rule::RuleSet;
