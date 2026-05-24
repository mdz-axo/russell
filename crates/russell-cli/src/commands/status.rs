// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell status` — read-only workspace summary.

use anyhow::Result;
use russell_core::paths::Paths;

pub fn run(paths: &Paths) -> Result<()> {
    println!("Russell — status");
    println!("  config:       {}", paths.config.display());
    println!("  state:        {}", paths.state.display());
    println!("  data:         {}", paths.data.display());

    let kill = paths.kill_switch();
    println!(
        "  kill switch:  {} ({})",
        kill.display(),
        if kill.exists() { "ENGAGED" } else { "clear" }
    );

    let profile_path = paths.profile();
    if profile_path.exists() {
        match russell_core::Profile::load(&profile_path) {
            Ok(p) => println!("  profile:      {} (id={})", p.schema, p.profile_id),
            Err(e) => println!(
                "  profile:      {} (load error: {})",
                profile_path.display(),
                e
            ),
        }
    } else {
        println!("  profile:      <absent — see docs/deployment/QUICKSTART.md>");
    }

    let journal_path = paths.journal();
    if journal_path.exists() {
        // Best-effort read of counts.
        let reader = russell_core::journal::JournalReader::new(&journal_path);
        let counts = reader.severity_counts(0, i64::MAX).unwrap_or_default();
        println!(
            "  journal:      {} (info={} warn={} alert={} crit={})",
            journal_path.display(),
            counts.info,
            counts.warn,
            counts.alert,
            counts.crit
        );
    } else {
        println!("  journal:      <absent — run `russell sentinel-once`>");
    }
    Ok(())
}
