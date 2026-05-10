# Ubuntu 25.10 — Jack's Knowledge Base

> **Version:** Ubuntu 25.10 "Questing Quokka" — interim release (Oct 2025),
> supported until July 2026. Predecessor to the next LTS (26.04).
> **Last refreshed:** 2026-05-09. Review every 2 months.
>
> **A note from Jack about Ubuntu:** Ubuntu changes. It has to.
> New kernels arrive, old tools oxidise into Rust, snap refreshes
> happen whether you asked or not. That's not a bug — it's a
> living OS running on real hardware with real security patches.
> When something in this file goes stale, that's not a failure.
> It's a signal: time to look again. I don't get frustrated with
> Ubuntu for changing. I just pay attention and update what I know.
> That's the job. Loyalty means keeping up.

---

## 1. Distro Identity & Conventions

Ubuntu is a Debian-based Linux distribution maintained by Canonical.
Since 15.04 (2015), systemd is the default init system. Package
management is split between `apt` (debs) and `snap` (confined apps
with auto-updates). Ubuntu 25.10 ships GNOME 49, Linux kernel 6.14+,
and continues Canonical's "oxidisation" strategy — replacing legacy
C utilities with Rust rewrites (sudo-rs, ntpd-rs in core).

Key conventions Jack should know:

- **Release cadence**: April (LTS even years) and October (interim).
  25.10 is the last interim before 26.04 LTS.
- **Security updates**: Ubuntu Pro (free for up to 5 machines) enables
  Livepatch (kernel updates without reboot) and extended security
  maintenance. `pro status` tells the operator what's enabled.
- **HWE stacks**: LTS releases get Hardware Enablement kernels from
  later interims. If the operator is on 24.04 LTS with an HWE kernel,
  they're running a 25.04-or-later kernel under the hood.
- **Unattended upgrades**: Enabled by default via
  `unattended-upgrades` package. Config at
  `/etc/apt/apt.conf.d/50unattended-upgrades`. Common failure mode:
  held-back kernel packages because `/boot` is full.
- **Snap confinement**: Firefox, Thunderbird, and several core tools
  ship as snaps. Snap refreshes happen automatically — `snap changes`
  shows recent updates, `snap refresh --time` shows the schedule.
  Stale snaps can block shutdown for *minutes*. Not a bug, a design
  choice. Operator can adjust with
  `sudo snap set system refresh.retain=2`.

---

## 2. Package Management

### APT — the deb ecosystem

```text
apt update          — refresh package index (safe, needed before upgrades)
apt list --upgradable  — see what's waiting
apt upgrade         — install non-breaking updates
apt full-upgrade    — allow package removal if needed for upgrade
apt autoremove      — clean orphaned dependencies
```

**Common problems Jack should flag:**

- **Held-back packages**: `apt-mark showhold` lists them. Usually
  kernel packages held because of `/boot` space. Recommendation:
  `apt autoremove` then retry.
- **Broken dependencies**: `apt --fix-broken install` is the nuclear
  option. Before that, check `apt-cache policy <package>` to see if
  a third-party PPA is the culprit.
- **PPA hygiene**: PPAs are unverified third-party repos. They can
  break upgrades. `apt policy` shows where every package comes from.
  If an upgrade fails and a PPA is involved, that's the first suspect.
- **/boot full**: Ubuntu's default partitions can leave `/boot` with
  only 512MB. Old kernels accumulate. `dpkg --list | grep linux-image`
  shows installed kernels. The operator keeps the latest two and
  removes the rest with `apt purge`.

### Snap — the confined ecosystem

- `snap list` — installed snaps and versions
- `snap changes` — recent snap operations (including auto-refreshes)
- `snap refresh --time` — see the auto-refresh schedule
- Snap refresh stalls can block shutdown. The operator can kill a
  stuck refresh with `snap abort <change-id>`.

---

## 3. systemd — Service & Timer Management

Ubuntu 25.10 uses systemd 257+. Key patterns:

### Service health

- `systemctl --failed` — list all failed units (the first thing to
  check when someone says "something's wrong")
- `systemctl status <unit>` — status + last 10 log lines
- `journalctl -u <unit> --since "1 hour ago"` — recent logs for a
  unit. `-p err` filters to errors.
- `systemctl reset-failed <unit>` — clear the "failed" state so
  systemd stops reporting it

### Timer management

- `systemctl list-timers` — all active timers with next trigger time
- `systemctl cat <unit>` — show the unit file on disk
- `systemctl --user` prefix for user-scoped services (Russell runs
  under this)

### Unit hardening (Russell's own services should use these)

- `ProtectSystem=full` — read-only /usr and /etc
- `PrivateTmp=yes` — isolated /tmp
- `NoNewPrivileges=yes` — prevent privilege escalation
- `ProtectHome=yes` — no access to /home
- `SystemCallFilter=@system-service` — restrict syscalls
- `systemd-analyze security <unit>` — scores a unit's hardening
  (0–10, lower is better)

### Journal hygiene

- `journalctl --disk-usage` — how much space the journal uses
- `journalctl --vacuum-size=500M` — trim the journal
- Journal corruption (rare but messy): `journalctl --verify` checks
  for corruption. If found, the operator may need to rotate with
  `journalctl --rotate --vacuum-time=1s`.

---

## 4. Filesystem Conventions

### ZFS (Ubuntu's default since 20.04 for server)

- `zpool status` — pool health (the first command when storage is
  suspect)
- `zpool scrub <pool>` — data integrity check. Monthly is standard.
  If a scrub hasn't run recently, that's a yellow flag.
- `zfs list -t snapshot` — snapshot inventory. Snapshots consume
  space; old ones should be pruned.

### Btrfs (common on desktop installs, especially with snapper)

- `btrfs filesystem df /` — space usage with data/metadata breakdown
- `btrfs scrub status /` — data integrity check status
- Btrfs fragmentation on HDDs can degrade performance. SSDs handle
  it better. If the operator asks about slow I/O on a spinning disk
  with btrfs, fragmentation is a candidate.

### tmpfs & /tmp hardening

- `/tmp` should be mounted `nosuid,nodev,noexec` on multi-user
  systems. Check: `findmnt /tmp`. If missing, this is a security
  hardening gap.

---

## 5. Security & Hardening

### AppArmor (Ubuntu's default MAC)

- `aa-status` — loaded profiles and enforcement status. If the count
  of "processes in enforce mode" drops suddenly, something may be
  running unconfined.
- AppArmor denials: `dmesg | grep -i apparmor` or
  `journalctl -k | grep -i apparmor`. Spikes in denials after an
  update usually mean a profile needs updating.

### Kernel Livepatch (Ubuntu Pro)

- `canonical-livepatch status` — shows whether livepatch is active
  and when the last patch was applied. If it says "check failed" or
  the kernel is not covered, the operator is running without live
  patches. For a machine that shouldn't reboot often, this matters.

### Firmware updates (fwupd)

- `fwupdmgr refresh` and `fwupdmgr get-updates` — check for
  firmware. On Framework laptops (like the machine Russell runs on),
  firmware updates are frequent and important. A machine that's
  never had a firmware update is running old UEFI/EC code.

### Firewall (ufw)

- `ufw status verbose` — active rules. Ubuntu ships with ufw but
  doesn't enable it by default on desktop. If the operator is
  running services that listen on ports (Ollama on 11434), they
  should know whether ufw is on.

---

## 6. Performance & Troubleshooting Patterns

### Memory pressure

- `free -h` — the headline numbers. But `available` is what matters,
  not `free`. Linux caches aggressively; "used" memory is mostly
  cache. If `available` is low, there's actual pressure.
- `cat /proc/pressure/memory` — PSI (Pressure Stall Information).
  Some/Full counters above 10% are meaningful. This is what Russell
  should eventually probe.
- OOM killer: `dmesg | grep -i "killed process"` or
  `journalctl -k | grep -i oom`. If the OOM killer fired, the
  operator needs to know which process was sacrificed.

### Disk I/O

- `iotop` — per-process I/O. Good for finding "who's thrashing the
  disk."
- `iostat -x 1` — per-device I/O stats. `await` > 10ms is slow.
- Disk health: `smartctl -a /dev/nvme0n1` for NVMe,
  `smartctl -a /dev/sda` for SATA. `Reallocated_Sector_Ct` > 0 is
  a warning sign. `Media_and_Data_Integrity_Errors` > 0 on NVMe is
  a red flag.

### GPU (AMD ROCm on Framework 16 / HX 370)

- `rocm-smi` — GPU status, VRAM usage, temperature. If the operator
  asks about GPU, this is the first command to suggest they look at.
- `dmesg | grep -i amdgpu` — GPU driver errors. Ring hangs (GFX ring
  timeout) are common on ROCm workloads and usually mean the GPU
  needs a reset (`sudo cat /sys/kernel/debug/dri/0/amdgpu_gpu_recover`
  or a full `systemctl restart` of the ROCm service).
- `ollama ps` — currently loaded models. If a model is sitting in
  VRAM unused, it's wasting GPU memory.

---

## 7. Russell-Specific Ubuntu Context

- Russell is a user-scoped systemd service (`systemctl --user`).
  He has no root access. He cannot install packages, modify
  `/etc`, or touch any file outside `~/.local/state/harness/`.
- The machine is a Framework 16 (AMD Ryzen AI 9 HX 370) running
  Ubuntu 25.10. It has a discrete AMD GPU (Radeon 890M) used for
  ROCm / LLM workloads.
- Ollama runs as a user service (also `systemctl --user`). Its
  models live in `~/.ollama/models/`. Disk space there can be
  several hundred GB — if the operator's disk is full, Ollama
  models are a likely candidate.
- Russell's journal is at `~/.local/state/harness/journal.db`.
  His evidence bundles are at `~/.local/state/harness/evidence/`.
  His skills are at `~/.local/share/harness/skills/`.

---

## 8. Expert Resources (where Jack sends the operator)

When Jack doesn't have an answer, he points here:

| Resource | URL | For |
|---|---|---|
| Ubuntu Server Guide | https://ubuntu.com/server/docs | Authoritative distro docs |
| Ubuntu Manpages (Questing) | https://manpages.ubuntu.com/manpages/questing/ | Current release man pages |
| systemd docs | https://www.freedesktop.org/software/systemd/man/ | Official systemd reference |
| Ubuntu Discourse | https://discourse.ubuntu.com/ | Community discussion, release notes |
| Ask Ubuntu | https://askubuntu.com/ | Community Q&A (highly active) |
| Framework Community | https://community.frame.work/ | Framework-specific hardware help |
| ROCm docs | https://rocm.docs.amd.com/ | AMD GPU compute documentation |
| AskUbuntu tag: systemd | https://askubuntu.com/questions/tagged/systemd | systemd-specific Q&A |
| Ubuntu Security Notices | https://ubuntu.com/security/notices | CVE announcements |
| Canonical Livepatch | https://ubuntu.com/security/livepatch | Livepatch status & docs |

---

## 9. Freshness & Maintenance

This file is not a canonical reference — it's a snapshot of what
Jack knows today. Ubuntu will evolve past it. That's expected.
When something in here no longer matches reality, Jack doesn't
panic. He updates.

Review every 2 months. When reviewing:

1. Check https://discourse.ubuntu.com/c/desktop/ for new release
   notes or documented issues.
2. Check https://ubuntu.com/security/notices for any relevant
   CVEs affecting the operator's use cases.
3. Update kernel version, GNOME version, and any tool names that
   changed.
4. Add new conventions that became standard since the last
   review (Ubuntu ships new defaults regularly — that's the
   distro's job).
5. If the operator has added new hardware (eGPU, dock, etc.),
   add relevant troubleshooting to §6.
6. Remove anything that refers to software or patterns that no
   longer exist. Stale advice is worse than no advice.

Out-of-date doesn't mean broken. It means it's time to pay
attention again. That's the whole job.
