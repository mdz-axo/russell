// SPDX-License-Identifier: MIT OR Apache-2.0
//! `russell profile` — show or initialise `profile.json`.

use anyhow::{Context, Result};
use russell_core::Profile;
use russell_core::paths::Paths;

pub fn run(paths: &Paths, init: bool) -> Result<()> {
    let path = paths.profile();
    if init && !path.exists() {
        let p = Profile::stub();
        p.save(&path)
            .with_context(|| format!("writing {}", path.display()))?;
        println!("wrote stub profile to {}", path.display());
    }
    if !path.exists() {
        println!("no profile at {}", path.display());
        println!("run `russell profile --init` to create a stub.");
        return Ok(());
    }
    let p = Profile::load(&path).with_context(|| format!("loading {}", path.display()))?;
    // Pretty-print as JSON; the machine chart is authored-by-JSON per ADR-0006.
    let out = serde_json::to_string_pretty(&p)?;
    println!("{out}");
    Ok(())
}
