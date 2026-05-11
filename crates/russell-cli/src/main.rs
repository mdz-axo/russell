// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell` — command-line entry point.
//!
//! Phase 1 (`cybernetic-health-harness.md` §20,
//! `docs/specifications/MVP_SPEC.md` §2) ships the six read-only
//! verbs plus the Doctor's `help` cry-for-help channel.

#![deny(unsafe_code)]
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
        /// Output format: `stdout` (default) or `daily-log` (writes
        /// `memory/daily/YYYY-MM-DD.md`).
        #[arg(long, default_value = "stdout")]
        format: String,
    },
    /// Run the Sentinel once and append samples to the journal.
    SentinelOnce,
    /// Probe the Okapi inference engine and record health metrics.
    /// Use --auto-apply to automatically fix detected problems
    /// (model load, adapter unload). Intended for systemd timer.
    OkapiProbe {
        /// Automatically apply safe management actions.
        #[arg(long)]
        auto_apply: bool,
        /// Model to load if none is loaded (required with --auto-apply).
        #[arg(long, default_value = "")]
        default_model: String,
    },
    /// Ask Jack for help — composes a SOAP bundle, consults the
    /// LLM (or offline fallback), writes evidence, prints the
    /// response.
    #[command(name = "jack")]
    Jack {
        /// Free-text context to include as Subjective.
        #[arg(long)]
        note: Option<String>,
    },
    /// List or run skill probes.
    Skill {
        #[command(subcommand)]
        cmd: SkillCmd,
    },
    /// Start an interactive chat with Jack.
    /// Each turn sends the latest journal state to the LLM,
    /// maintaining conversation context across turns.
    Chat,
}

#[derive(Subcommand, Debug)]
enum SkillCmd {
    /// List all loaded skills and their probes/interventions.
    List,
    /// Run a probe or intervention from a loaded skill.
    Run {
        /// Skill reference in `<skill-id>/<probe-id>` format.
        id: String,
        /// Print what would run without executing.
        #[arg(long)]
        dry_run: bool,
    },
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    // Discover and load an env file before anything else. Existing
    // env always wins. See `russell_core::env::load_discovered` for
    // the precedence order.
    let paths_probe = std::env::var_os("RUSSELL_ROOT")
        .map(|r| russell_core::paths::Paths::rooted(std::path::PathBuf::from(r)))
        .or_else(|| russell_core::paths::Paths::from_env().ok())
        .unwrap_or_else(|| russell_core::paths::Paths::rooted("/tmp"));
    let loaded_env = russell_core::env::load_discovered(&paths_probe.config, None);
    if let Some(p) = &loaded_env {
        tracing::debug!(path = %p.display(), "env file loaded");
    }

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
        Command::Digest {
            since_hours,
            format,
        } => commands::digest::run(&paths, since_hours, &format),
        Command::SentinelOnce => commands::sentinel_once::run(&paths),
        Command::OkapiProbe {
            auto_apply,
            default_model,
        } => commands::okapi_probe::run(&paths, auto_apply, &default_model),
        Command::Jack { note } => commands::help::run(&paths, note.as_deref()).await,
        Command::Skill { cmd } => match cmd {
            SkillCmd::List => commands::skill::list(&paths),
            SkillCmd::Run { id, dry_run } => commands::skill::run(&paths, &id, dry_run).await,
        },
        Command::Chat => commands::chat::run(&paths).await,
    }
}
