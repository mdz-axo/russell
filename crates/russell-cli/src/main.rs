// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell` — command-line entry point.
//!
//! Phase 0 (`cybernetic-health-harness.md` §20) ships read-only
//! subcommands: `list`, `status`, `profile`, `digest`, plus the
//! `sentinel-once` helper used to populate the journal in
//! development and to provide the 7-day samples that Phase 0
//! defines as its success criterion.

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]
#![warn(missing_docs)]

mod commands;

use anyhow::Result;
use clap::{Parser, Subcommand};

/// Russell — cybernetic health harness.
#[derive(Parser, Debug)]
#[command(
    name = "russell",
    version,
    about = "Cybernetic health harness for a Linux AI/ML workstation",
    long_about = None,
)]
struct Cli {
    /// Anchor all on-disk paths under this directory instead of $HOME.
    /// Primarily useful for testing; production defaults to XDG paths.
    #[arg(long, global = true)]
    root: Option<std::path::PathBuf>,

    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Show workspace / journal / profile summary.
    Status,
    /// List recent journal events.
    List {
        /// Max rows to print.
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Show or initialise the machine profile.
    Profile {
        /// Initialise a stub profile.json if one does not exist.
        #[arg(long)]
        init: bool,
    },
    /// Render a Markdown digest of recent activity.
    Digest {
        /// Window in hours back from now.
        #[arg(long, default_value_t = 168)]
        since_hours: u32,
    },
    /// Run the Sentinel once and append samples to the journal.
    /// Phase-0 helper; the full timer-driven Sentinel lands in
    /// Phase 1.
    SentinelOnce,
}

fn main() -> Result<()> {
    russell_core::telemetry::init();
    let cli = Cli::parse();

    let paths = match cli.root {
        Some(ref r) => russell_core::paths::Paths::rooted(r),
        None => russell_core::paths::Paths::from_env()?,
    };
    paths.ensure_dirs()?;

    match cli.cmd {
        Command::Status => commands::status::run(&paths),
        Command::List { limit } => commands::list::run(&paths, limit),
        Command::Profile { init } => commands::profile::run(&paths, init),
        Command::Digest { since_hours } => commands::digest::run(&paths, since_hours),
        Command::SentinelOnce => commands::sentinel_once::run(&paths),
    }
}
