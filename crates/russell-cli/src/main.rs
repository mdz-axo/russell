// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell` — command-line entry point.

#![deny(unsafe_code)]
#![deny(rust_2018_idioms)]

mod commands;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "russell", version, about = "Cybernetic health harness")]
struct Cli {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Show workspace summary.
    Status,
    /// List recent journal events.
    List { #[arg(long, default_value_t = 20)] limit: usize },
    /// Render Markdown digest.
    Digest { #[arg(long, default_value_t = 168)] since_hours: u32 },
    /// Run Sentinel once.
    SentinelOnce,
    /// Consult LLM for health assessment.
    Jack { #[arg(long)] note: Option<String> },
    /// List skills.
    SkillList,
    /// Run a skill.
    SkillRun { id: String },
    /// Install a skill.
    SkillInstall { name: String },
    /// Prune stale skills.
    SkillPrune,
    /// Run proprioception.
    Proprio,
    /// Show docs.
    Docs { #[arg(long)] strict: bool },
    /// Verify journal integrity.
    VerifyJournal,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let paths = russell_core::paths::Paths::from_env()?;

    match cli.cmd {
        Command::Status => commands::status::run(&paths),
        Command::List { limit } => commands::list::run(&paths, limit),
        Command::Digest { since_hours } => commands::digest::run(&paths, since_hours, "stdout"),
        Command::SentinelOnce => commands::sentinel_once::run(&paths),
        Command::Jack { note } => commands::help::run(&paths, note.as_deref()).await,
        Command::SkillList => commands::skill::list(&paths),
        Command::SkillRun { id } => commands::skill::run(&paths, &id, false).await,
        Command::SkillInstall { name } => commands::skill_lifecycle::install_skill(&paths, &name, true),
        Command::SkillPrune => commands::skill_lifecycle::prune_skill(&paths, "", true),
        Command::Proprio => commands::proprio::run(&paths).await,
        Command::Docs { strict } => commands::docs::run(&paths, strict),
        Command::VerifyJournal => commands::verify::run(&paths),
    }
}
