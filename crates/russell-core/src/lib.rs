// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell-core` — foundation crate.

#![deny(unsafe_code)]
#![deny(rust_2018_idioms)]
#![warn(missing_docs)]

pub mod channel;
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

pub use channel::{spawn_journal_writer, JournalCommand, JournalHandle, JournalWriterTask};
pub use error::{CoreError, Result};
pub use event::{Event, EventId, Severity};
pub use journal::port::{InMemoryJournal, JournalReadPort, JournalWritePort};
pub use journal::{JournalReader, JournalWriter};
pub use profile::Profile;
pub use reflex::{BudgetVerdict, ReflexBudget, ReflexSet};
pub use rule::{ConfigWarning, RuleSet};
pub use time::{Clock, FixedClock, SystemClock};
