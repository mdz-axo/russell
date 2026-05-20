// SPDX-License-Identifier: MIT OR Apache-2.0
//! Process probe compositions.
//!
//! Each probe composes connectors (I/O) with tools (transforms).
//! The composition is thin glue — no logic beyond sequencing.
//!
//! All probes scan `/proc` to enumerate running processes. Each
//! probe function performs its own scan; the 5-minute cadence
//! makes the overhead negligible.

use super::connectors;
use super::tools;
use super::tools::ProcessStat;
use crate::impl_probe;

/// Collect `ProcessStat` for every visible PID.
///
/// Reads `/proc` directory listing, then reads `/proc/<pid>/stat`
/// for each PID. PIDs that exit between listing and reading are
/// silently skipped.
fn collect_stats() -> Vec<ProcessStat> {
    let Some(pids) = connectors::list_proc_pids() else {
        return Vec::new();
    };
    let mut stats = Vec::with_capacity(pids.len());
    for pid in &pids {
        if let Some(content) = connectors::read_proc_stat(*pid)
            && let Some(stat) = tools::parse_proc_stat(&content)
        {
            stats.push(stat);
        }
    }
    stats
}

/// Probe: total number of running processes (including kernel
/// threads, excluding processes that exited between listing and
/// reading).
pub fn proc_total_count() -> Option<f64> {
    let stats = collect_stats();
    Some(stats.len() as f64)
}

/// Probe: number of zombie (defunct) processes.
///
/// Zombies are processes that have exited but whose parent has
/// not yet called `wait()`. A growing count indicates orphaned
/// children.
pub fn proc_zombie_count() -> Option<f64> {
    let count = collect_stats().iter().filter(|s| s.state == 'Z').count();
    Some(count as f64)
}

/// Probe: number of processes in uninterruptible sleep (D state).
///
/// D-state processes are stuck waiting on I/O (typically storage).
/// A persistently high count may indicate a failing disk or
/// filesystem hang.
pub fn proc_stuck_count() -> Option<f64> {
    let count = collect_stats().iter().filter(|s| s.state == 'D').count();
    Some(count as f64)
}

/// Probe: number of processes in running state (R state).
///
/// This is the count of processes currently on the run queue.
/// When it exceeds the CPU count significantly, the system is
/// CPU-bound.
pub fn proc_running_count() -> Option<f64> {
    let count = collect_stats().iter().filter(|s| s.state == 'R').count();
    Some(count as f64)
}

/// Probe: name of the process with the highest cumulative CPU
/// time (utime + stime).
///
/// This is not a real-time CPU percentage, but a snapshot of
/// which process has consumed the most CPU over its lifetime.
/// Useful for identifying long-running resource hogs.
pub fn proc_top_cpu_name() -> Option<String> {
    collect_stats()
        .iter()
        .max_by_key(|s| tools::cpu_ticks(s))
        .map(|s| s.comm.clone())
}

/// Probe: name of the process with the highest RSS (resident
/// set size) in pages.
///
/// Combined with `proc_top_mem_pct`, this tells Jack which
/// application is consuming the most physical memory.
pub fn proc_top_mem_name() -> Option<String> {
    collect_stats()
        .iter()
        .max_by_key(|s| s.rss_pages)
        .map(|s| s.comm.clone())
}

/// Probe: RSS of the largest process expressed as a percentage
/// of total system memory (MemTotal from `/proc/meminfo`).
///
/// Uses 4 KiB page size (standard on x86_64 Linux).
pub fn proc_top_mem_pct() -> Option<f64> {
    let stats = collect_stats();
    let max_rss_pages = stats.iter().map(|s| s.rss_pages).max()?;
    let meminfo = connectors::read_file_to_string("/proc/meminfo")?;
    let total_kib = tools::parse_meminfo_kib(&meminfo, "MemTotal")?;
    if total_kib == 0 {
        return None;
    }
    let max_rss_kib = max_rss_pages * 4;
    Some((max_rss_kib as f64 / total_kib as f64) * 100.0)
}

/// Text-valued process samples (top CPU/memory names).
pub(crate) fn process_text_samples() -> Vec<super::Sample> {
    let stats = collect_stats();
    process_text_samples_from(&stats)
}

fn process_text_samples_from(stats: &[ProcessStat]) -> Vec<super::Sample> {
    let mut out = Vec::new();
    if let Some(s) = stats.iter().max_by_key(|s| tools::cpu_ticks(s)) {
        out.push(super::Sample {
            name: "proc_top_cpu_name".into(),
            value_num: None,
            value_text: Some(s.comm.clone()),
            unit: None,
        });
    }
    if let Some(s) = stats.iter().max_by_key(|s| s.rss_pages) {
        out.push(super::Sample {
            name: "proc_top_mem_name".into(),
            value_num: None,
            value_text: Some(s.comm.clone()),
            unit: None,
        });
    }
    out
}

// -- ProbeDescriptor impls (T13 split form) --

use super::descriptor::{ProbeCollector, ProbeMetadata};

/// Probe descriptor.
pub struct ProcTotalCount;
impl ProbeMetadata for ProcTotalCount {
    fn name(&self) -> &'static str { "proc_total_count" }
    fn unit(&self) -> Option<&'static str> { Some("count") }
}
impl ProbeCollector for ProcTotalCount {
    fn collect(&self) -> Option<f64> { proc_total_count() }
}

/// Probe descriptor.
pub struct ProcZombieCount;
impl ProbeMetadata for ProcZombieCount {
    fn name(&self) -> &'static str { "proc_zombie_count" }
    fn unit(&self) -> Option<&'static str> { Some("count") }
}
impl ProbeCollector for ProcZombieCount {
    fn collect(&self) -> Option<f64> { proc_zombie_count() }
}

/// Probe descriptor.
pub struct ProcStuckCount;
impl ProbeMetadata for ProcStuckCount {
    fn name(&self) -> &'static str { "proc_stuck_count" }
    fn unit(&self) -> Option<&'static str> { Some("count") }
}
impl ProbeCollector for ProcStuckCount {
    fn collect(&self) -> Option<f64> { proc_stuck_count() }
}

/// Probe descriptor.
pub struct ProcRunningCount;
impl ProbeMetadata for ProcRunningCount {
    fn name(&self) -> &'static str { "proc_running_count" }
    fn unit(&self) -> Option<&'static str> { Some("count") }
}
impl ProbeCollector for ProcRunningCount {
    fn collect(&self) -> Option<f64> { proc_running_count() }
}

/// Probe descriptor.
pub struct ProcTopMemPct;
impl ProbeMetadata for ProcTopMemPct {
    fn name(&self) -> &'static str { "proc_top_mem_pct" }
    fn unit(&self) -> Option<&'static str> { Some("%") }
}
impl ProbeCollector for ProcTopMemPct {
    fn collect(&self) -> Option<f64> { proc_top_mem_pct() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proc_total_count_returns_something_on_linux() {
        if !std::path::Path::new("/proc").exists() {
            return;
        }
        let count = proc_total_count();
        assert!(count.is_some());
        assert!(count.unwrap() > 0.0, "expected at least one process");
    }

    #[test]
    fn proc_zombie_count_is_non_negative() {
        if !std::path::Path::new("/proc").exists() {
            return;
        }
        let count = proc_zombie_count();
        assert!(count.is_some());
        assert!(count.unwrap() >= 0.0);
    }

    #[test]
    fn proc_stuck_count_is_non_negative() {
        if !std::path::Path::new("/proc").exists() {
            return;
        }
        let count = proc_stuck_count();
        assert!(count.is_some());
        assert!(count.unwrap() >= 0.0);
    }

    #[test]
    fn proc_top_cpu_name_returns_something_on_linux() {
        if !std::path::Path::new("/proc").exists() {
            return;
        }
        let name = proc_top_cpu_name();
        assert!(name.is_some());
        assert!(!name.unwrap().is_empty());
    }

    #[test]
    fn proc_top_mem_name_returns_something_on_linux() {
        if !std::path::Path::new("/proc").exists() {
            return;
        }
        let name = proc_top_mem_name();
        assert!(name.is_some());
        assert!(!name.unwrap().is_empty());
    }

    #[test]
    fn proc_top_mem_pct_is_percentage() {
        if !std::path::Path::new("/proc").exists() {
            return;
        }
        let pct = proc_top_mem_pct();
        assert!(pct.is_some());
        let v = pct.unwrap();
        assert!(
            (0.0..=100.0).contains(&v),
            "memory pct should be 0-100, got {v}"
        );
    }
}
