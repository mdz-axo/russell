// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell-core` — foundation crate.

#![deny(unsafe_code)]
#![deny(rust_2018_idioms)]
#![warn(missing_docs)]

pub mod channel;
pub mod config;
pub mod encryption;
pub mod env;
pub mod error;
pub mod event;
pub mod hash_chain;
pub mod identity;
pub mod inference;
pub mod journal;
pub mod paths;
pub mod profile;
pub mod reflex;
pub mod risk;
pub mod rule;
pub mod schedule;
pub mod telemetry;
pub mod time;

pub use channel::{JournalCommand, JournalHandle, JournalWriterTask, spawn_journal_writer};
pub use config::RuntimeConfig;
pub use error::{CoreError, Result};
pub use event::{Event, EventId, Severity};
pub use journal::port::{InMemoryJournal, JournalReadPort, JournalWritePort};
pub use journal::{JournalReader, JournalWriter};
pub use profile::Profile;
pub use reflex::{BudgetVerdict, ReflexBudget, ReflexSet};
pub use risk::RiskBand;
pub use rule::{ConfigWarning, RuleSet};
pub use time::{Clock, FixedClock, SystemClock};
