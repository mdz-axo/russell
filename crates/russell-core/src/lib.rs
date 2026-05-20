// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell-core` — foundation crate.
//!
//! **TOGAF Phase:** Phase C (Data Architecture) — domain types define the
//! canonical data shape for the single SQLite journal (JR-7).
//!
//! Provides the cross-cutting types every other Russell crate depends on:
//!
//! - [`paths`] — XDG-aware state / config / data paths. The one place that
//!   answers "where does Russell's data live?"
//! - [`event`] — `harness.event.v1` record type (see
//!   `cybernetic-health-harness.md` §14 and
//!   [`docs/standards/safety.md`](../../docs/standards/safety.md) for the
//!   IDRS structured-log requirement).
//! - [`profile`] — `russell.profile.v1` machine chart
//!   ([ADR-0006](../../docs/adr/0006-profile-abstraction.md)).
//! - [`journal`] — SQLite journal behind a typed API
//!   ([ADR-0004](../../docs/adr/0004-sqlite-journal.md)).
//! - [`telemetry`] — `tracing` subscriber setup (logging only in MVP;
//!   full observability stack is deferred per
//!   [ADR-0010](../../docs/adr/deferred/0010-observability-stack.md)).
//!
//! All I/O into `~/.local/state/harness/` routes through this crate.
//! No other crate opens the journal DB directly.

#![deny(unsafe_code)]
#![deny(rust_2018_idioms)]
#![warn(missing_docs)]

pub mod env;
pub mod error;
pub mod event;
pub mod hash_chain;
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
pub use journal::port::{InMemoryJournal, JournalReadPort, JournalWritePort};
pub use profile::Profile;
pub use journal::{JournalReader, JournalWriter};
pub use reflex::{BudgetVerdict, ReflexBudget, ReflexSet};
pub use rule::{ConfigWarning, RuleSet};
pub use time::{Clock, FixedClock, SystemClock};
