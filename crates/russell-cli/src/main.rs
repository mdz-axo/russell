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
    /// Show pod status.
    PodStatus,
    /// Activate Russell pod.
    PodActivate,
    /// Deactivate Russell pod.
    PodDeactivate,
    /// Show agent persona.
    PodPersonaShow,
    /// List memory artifacts.
    PodArtifactsList {
        #[arg(long, default_value = "all")]
        r#type: String,
    },
    /// Export memory artifacts.
    PodArtifactsExport {
        #[arg(long)]
        output: String,
        #[arg(long, default_value = "public")]
        visibility: String,
    },
    /// List recent journal events.
    List {
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Render Markdown digest.
    Digest {
        #[arg(long, default_value_t = 168)]
        since_hours: u32,
    },
    /// Run Sentinel once.
    SentinelOnce,
    /// Interactive chat with Jack.
    Chat,
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
    /// Run self-triage (Russell diagnoses own health).
    SelfTriage,
    /// Show docs.
    Docs {
        #[arg(long)]
        strict: bool,
    },
    /// Verify journal integrity.
    VerifyJournal,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let paths = russell_core::paths::Paths::from_env()?;

    match cli.cmd {
        Command::Status => commands::status::run(&paths),
        Command::PodStatus => commands::pod::status(&paths),
        Command::PodActivate => commands::pod::activate(&paths).await,
        Command::PodDeactivate => commands::pod::deactivate(&paths).await,
        Command::PodPersonaShow => commands::pod::persona_show(&paths),
        Command::PodArtifactsList { r#type } => commands::pod::artifacts_list(&paths, &r#type),
        Command::PodArtifactsExport { output, visibility } => {
            commands::pod::artifacts_export(&paths, &output, &visibility)
        }
        Command::List { limit } => commands::list::run(&paths, limit),
        Command::Digest { since_hours } => commands::digest::run(&paths, since_hours, "stdout"),
        Command::SentinelOnce => commands::sentinel_once::run(&paths),
        Command::Chat => commands::chat::run(&paths).await,
        Command::SkillList => commands::skill::list(&paths),
        Command::SkillRun { id } => commands::skill::run(&paths, &id, false).await,
        Command::SkillInstall { name } => {
            commands::skill_lifecycle::install_skill(&paths, &name, true)
        }
        Command::SkillPrune => commands::skill_lifecycle::prune_skill(&paths, "", true),
        Command::Proprio => commands::proprio::run(&paths).await,
        Command::SelfTriage => commands::self_triage::run(&paths).await,
        Command::Docs { strict } => commands::docs::run(&paths, strict),
        Command::VerifyJournal => commands::verify::run(&paths),
    }
}
