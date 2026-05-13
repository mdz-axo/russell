# SysAdmin — Jack's Host Maintenance Knowledge Base

> **Version:** 0.1.0 — initial sysadmin toolkit for Russell
> **Last refreshed:** 2026-05-12

This is a **mutation skill** — Russell's first general-purpose host
maintenance toolkit. It gives Jack the ability to *propose* concrete
cleanup actions when the host shows symptoms that need more than
just watching.

## What this skill covers

| Domain | Probes | Interventions |
|---|---|---|
| Systemd units | Failed unit count, degraded state | `reset-failed` (user + system) |
| Clock (NTP) | chronyc offset, stratum, sync status | `force-clock-sync` |
| Zombie processes | Zombie count, parent identification | `reap-zombies` (SIGCHLD parents) |
| Journal | Disk usage (user + system) | `journal-vacuum` (500M cap) |
| Coredumps | File count, total size, age | `remove-coredumps` |
| Swap | Usage, pressure, top consumers | `flush-swap` (swapoff/swapon) |
| Mounts | Stale/unreachable mountpoints | (detection only — no auto-unmount) |

## When Jack should propose each intervention

### systemd-reset-failed
- **Symptom:** `systemd_service_degraded`, `systemd_timer_misfire`
- **Trigger:** Sentinel reports failed user unit count > 0
- **What it does:** Runs `systemctl --user reset-failed`, clearing the
  failed flag on all failed user units. Does NOT restart anything.
- **When to skip:** If the same unit fails repeatedly — the unit has
  a real problem. Jack should mention the failing unit name and suggest
  investigating logs rather than resetting.
- **Risk:** Low. Purely cosmetic — clearing the flag doesn't affect
  running services.

### systemd-reset-failed-system
- Same as above but targets system-level units. Requires sudo.
- Only propose if `probe-systemd-degraded` shows degraded state AND
  the operator has NOPASSWD sudo configured for systemctl.

### force-clock-sync
- **Symptom:** `clock_skew`
- **Trigger:** `probe-clock-offset` shows offset > 5 seconds or
  status=desynced. Also when Jack detects `timer_drift_s` > 300
  (the proprioception probe that watches systemd timer drift).
- **What it does:** Runs `chronyc -a makestep` (immediate step, not
  slow slew). Falls back to cycling `timedatectl set-ntp`.
- **When to skip:** If the machine is in a VM that gets clock from
  the hypervisor — chronyc may have nothing to sync against.
- **Risk:** Medium. A clock jump can confuse log ordering, database
  transactions, and TLS certificate validation. But a clock that's
  5 minutes off is already confusing everything — the jump is the
  lesser evil.
- **Post-intervention:** Jack should note whether the offset returned
  to < 1 second.

### reap-zombies
- **Symptom:** `zombie_accumulation`, `process_table_bloat`
- **Trigger:** Sentinel reports `proc_zombie_count` > 5 (warn) or
  > 20 (alert)
- **What it does:** Finds all zombie processes, groups them by parent
  PID, and sends SIGCHLD to each parent. This prompts parents to call
  `waitpid()` and clean up. Zombies reparented to init (PID 1) are
  left alone — init reaps automatically.
- **Safety:** Uses SIGCHLD, not SIGKILL. SIGCHLD is harmless to a
  running process — it just says "one of your children changed state,
  maybe check on them."
- **When to skip:** If the zombie count is growing rapidly (rate >
  10/min) — the parent process has a bug and signalling won't fix it.
  Jack should name the parent process so the operator can investigate.
- **Risk:** Medium. The signalling itself is safe, but the operator
  should know which process is spawning zombies.

### journal-vacuum
- **Symptom:** `journal_bloat`
- **Trigger:** `probe-journal-size` shows user_journal_mb > 1000
  (1 GB) or system_journal_mb > 2000 (2 GB)
- **What it does:** Runs `journalctl --user --vacuum-size=500M` or
  `journalctl --vacuum-size=500M` (system, needs sudo). Trims the
  oldest entries until the journal fits within the cap.
- **Risk:** Low. Vacuum only removes old archived entries — recent
  logs are preserved. Default Ubuntu retention is generous.
- **Post-intervention:** Jack should note the new journal size.

### remove-coredumps
- **Symptom:** `coredump_accumulation`
- **Trigger:** `probe-coredumps` shows total_mb > 500 or count > 10
- **What it does:** Removes all `core.*` files from
  `/var/lib/systemd/coredump/`.
- **When to skip:** If a specific application is crashing repeatedly
  — the coredumps are evidence. Jack should name the crashing binary
  rather than suggesting removal. Only propose removal when the dumps
  are old (oldest > 7 days) and space is the concern.
- **Risk:** Low. Coredumps are forensic artifacts, not system state.

### flush-swap
- **Symptom:** `swap_pressure`, `swap_retention`
- **Trigger:** `probe-swap-detail` shows swap_used_kb > 1 GB AND
  avail_memory > swap_used (sufficient RAM to page everything back)
- **What it does:** Drops page cache, then runs `swapoff -a` followed
  by `swapon -a`. This forces all swapped pages back into RAM and
  starts swap fresh.
- **When to skip:** If available memory is less than swap used — the
  swapoff will fail or hang. If the same workload immediately
  re-swaps, the machine is genuinely under memory pressure and needs
  more RAM or fewer processes, not swap flushing.
- **Risk:** Medium. swapoff can be slow (seconds to minutes).
  Requires sudo. The cache drop momentarily reduces I/O performance.
- **Post-intervention:** Jack should note the new swap usage and
  whether it starts climbing again.

## Working with the consent gate

The sysadmin skill declares `safety.max_auto_risk: medium`. This
means the consent gate will allow interventions up to Medium risk
without additional password prompts (the operator still has to
`/approve`).

`flush-swap` and `force-clock-sync` are additionally listed in
`require_human_for` — the dispatcher gates on this field to require
explicit human review even within the auto-risk cap. (This field is
loaded from the manifest but the enforcement code is pending.)

## Sudo requirements

Several interventions require root privileges:

| Intervention | Needs sudo | Command |
|---|---|---|
| systemd-reset-failed-system | Yes | `systemctl reset-failed` |
| force-clock-sync | Yes | `chronyc -a makestep` |
| journal-vacuum-system | Yes | `journalctl --vacuum-size=500M` |
| remove-coredumps | Yes | `rm /var/lib/systemd/coredump/core.*` |
| flush-swap | Yes | `swapoff -a && swapon -a` |

The operator must configure NOPASSWD sudo in
`/etc/sudoers.d/russell` for these commands. Example:

```
<user> ALL=(ALL) NOPASSWD: /usr/bin/systemctl reset-failed
<user> ALL=(ALL) NOPASSWD: /usr/bin/chronyc -a makestep
<user> ALL=(ALL) NOPASSWD: /usr/bin/journalctl --vacuum-size=*
<user> ALL=(ALL) NOPASSWD: /usr/sbin/swapoff -a
<user> ALL=(ALL) NOPASSWD: /usr/sbin/swapon -a
```

## What this skill does NOT do (yet)

- **Targeted unit restart:** The current ACTION syntax
  (`skill-id/intervention-id`) doesn't support dynamic arguments.
  Cannot do `restart-unit <name>` without extending the protocol.
  Workaround: Jack names the failing unit; the operator runs
  `systemctl --user restart <unit>` manually.
- **apt autoremove:** Deferred to Phase 4 per `MVP_SPEC.md`.
- **Snap cleanup:** Deferred — snap refreshes and retention policies
  are in the ubuntu-jack knowledge base but have no skill backing.
- **Disk space cleanup (general):** Deferred. The probes provide
  visibility; interventions for package cache, Docker images, etc.
  are future work.
- **Network interface reset:** Out of scope — Russell is a single-host
  harness, not a network manager.

## Freshness

This skill targets Ubuntu 25.10 with systemd 257+, chrony (ntpd-rs),
and standard Linux /proc and /sys interfaces. It should remain
compatible with future Ubuntu releases as these are stable kernel
and systemd interfaces.

Review every 2 months alongside ubuntu-jack's KNOWLEDGE.md.
