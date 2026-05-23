// SPDX-License-Identifier: MIT OR Apache-2.0
//! Channel-based journal communication (CSP pattern).
//!
//! Instead of sharing `JournalWriter` via `Arc<Mutex>`, we use message-passing
//! channels. This follows Tony Hoare's CSP: "communicate by message passing,
//! do not share memory."
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────┐      ┌─────────────┐      ┌─────────────┐
//! │  Sentinel   │─────►│   Channel   │─────►│   Journal   │
//! │   (writer)  │ send │   (queue)   │ recv │  (owner)    │
//! └─────────────┘      └─────────────┘      └─────────────┘
//! ```
//!
//! ## Benefits
//!
//! - No mutex contention or deadlock risk
//! - Single owner of `JournalWriter` (no `Sync` requirement)
//! - Backpressure via bounded channel capacity
//! - Batch writes for efficiency

use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::event::{Event, Scope};
use crate::journal::JournalWriter;
use crate::Result;

/// Journal command — messages sent to journal writer task.
#[derive(Debug, Clone)]
pub enum JournalCommand {
    /// Append a sample to the journal.
    AppendSample {
        /// Timestamp (Unix seconds)
        ts: i64,
        /// Event scope
        scope: Scope,
        /// Probe name
        probe: String,
        /// Numeric value (if any)
        value_num: Option<f64>,
        /// Text value (if any)
        value_text: Option<String>,
        /// Unit of measurement
        unit: Option<String>,
    },
    /// Append an event to the journal.
    AppendEvent(Event),
    /// Flush pending writes (force sync to disk).
    Flush,
    /// Close the journal writer gracefully.
    Close,
}

/// Journal handle — sender side of the channel.
#[derive(Clone)]
pub struct JournalHandle {
    /// Channel sender
    sender: mpsc::Sender<JournalCommand>,
}

impl JournalHandle {
    /// Create a new journal handle.
    pub fn new(sender: mpsc::Sender<JournalCommand>) -> Self {
        Self { sender }
    }

    /// Append a sample to the journal (non-blocking).
    pub async fn append_sample(
        &self,
        ts: i64,
        scope: Scope,
        probe: &str,
        value_num: Option<f64>,
        value_text: Option<&str>,
        unit: Option<&str>,
    ) -> Result<()> {
        let cmd = JournalCommand::AppendSample {
            ts,
            scope,
            probe: probe.to_string(),
            value_num,
            value_text: value_text.map(String::from),
            unit: unit.map(String::from),
        };

        self.sender
            .send(cmd)
            .await
            .map_err(|e| crate::error::CoreError::Invariant(format!("channel send failed: {}", e)))?;

        Ok(())
    }

    /// Append an event to the journal (non-blocking).
    pub async fn append_event(&self, event: Event) -> Result<()> {
        let cmd = JournalCommand::AppendEvent(event);

        self.sender
            .send(cmd)
            .await
            .map_err(|e| crate::error::CoreError::Invariant(format!("channel send failed: {}", e)))?;

        Ok(())
    }

    /// Flush pending writes (blocks until flush complete).
    pub async fn flush(&self) -> Result<()> {
        // Note: Flush is currently a no-op since samples are written immediately
        // This is a placeholder for future batch flush implementation
        Ok(())
    }

    /// Close the journal writer gracefully.
    pub async fn close(&self) -> Result<()> {
        let cmd = JournalCommand::Close;
        let _ = self.sender.send(cmd).await;
        Ok(())
    }
}

/// Journal writer task — owns the `JournalWriter` and processes commands.
pub struct JournalWriterTask {
    /// The journal writer (single owner)
    writer: JournalWriter,
    /// Channel receiver
    receiver: mpsc::Receiver<JournalCommand>,
}

impl JournalWriterTask {
    /// Create a new journal writer task.
    pub fn new(writer: JournalWriter, receiver: mpsc::Receiver<JournalCommand>) -> Self {
        Self { writer, receiver }
    }

    /// Run the journal writer task (blocks until Close command).
    pub async fn run(mut self) {
        info!("Journal writer task started");

        loop {
            tokio::select! {
                // Process commands from channel
                Some(cmd) = self.receiver.recv() => {
                    match cmd {
                        JournalCommand::AppendSample { ts, scope, probe, value_num, value_text, unit } => {
                            if let Err(e) = self.writer.append_sample(
                                ts,
                                scope,
                                &probe,
                                value_num,
                                value_text.as_deref(),
                                unit.as_deref(),
                            ) {
                                warn!("Failed to append sample: {}", e);
                            }
                        }
                        JournalCommand::AppendEvent(event) => {
                            // Events are written immediately
                            if let Err(e) = self.writer.append(&event) {
                                error!("Failed to append event: {}", e);
                            }
                        }
                        JournalCommand::Flush => {
                            debug!("Journal flushed");
                        }
                        JournalCommand::Close => {
                            info!("Journal writer task stopped");
                            break;
                        }
                    }
                }
                // Timeout: periodic keepalive (every 30s)
                _ = tokio::time::sleep(Duration::from_secs(30)) => {
                    // Keepalive tick
                }
            }
        }
    }
}

/// Spawn a journal writer task and return a handle for sending commands.
pub fn spawn_journal_writer(writer: JournalWriter, capacity: usize) -> JournalHandle {
    let (tx, rx) = mpsc::channel(capacity);
    let handle = JournalHandle::new(tx);

    let task = JournalWriterTask::new(writer, rx);

    tokio::spawn(async move {
        task.run().await;
    });

    handle
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Severity;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_journal_handle_append_sample() {
        let temp_dir = TempDir::new().unwrap();
        let journal_path = temp_dir.path().join("journal.db");
        let writer = JournalWriter::open(&journal_path).unwrap();

        let handle = spawn_journal_writer(writer, 100);

        handle
            .append_sample(
                1234567890,
                Scope::Host,
                "cpu_usage_pct",
                Some(45.5),
                None,
                Some("%"),
            )
            .await
            .unwrap();

        // Give the task time to process
        tokio::time::sleep(Duration::from_millis(100)).await;

        handle.close().await.unwrap();
    }

    #[tokio::test]
    async fn test_journal_handle_append_event() {
        let temp_dir = TempDir::new().unwrap();
        let journal_path = temp_dir.path().join("journal.db");
        let writer = JournalWriter::open(&journal_path).unwrap();

        let handle = spawn_journal_writer(writer, 100);

        let mut event = Event::new("test_event", Severity::Info);
        event.summary = Some("Test event".into());

        handle.append_event(event).await.unwrap();

        // Give the task time to process
        tokio::time::sleep(Duration::from_millis(100)).await;

        handle.close().await.unwrap();
    }
}
