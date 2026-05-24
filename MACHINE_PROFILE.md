---
title: "Machine Profile — The Patient"
audience: [operators, developers, agents]
last_updated: 2026-05-14
togaf_phase: "C"
version: "2.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Data Architecture -->
<!-- VERSION: 2.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-14 -->

# Machine Profile — "The Patient"

Observed 2026-05-11 on `mdz-axolotl-Laptop-16`.
Updated: 2026-05-11 (GPU probe coverage, DRM mappings, VRAM sizes).
This is the source-of-truth for every recommendation in
[`docs/status/CONSOLIDATED-STATUS.md`](docs/status/CONSOLIDATED-STATUS.md).
Re-run the probes in §7 when anything material changes.

---

## 1. Chassis & Power

| Field | Value |
|---|---|
| Vendor / model | **Framework Laptop 16 (AMD Ryzen AI 300 Series)** |
| BIOS/UEFI | (capture via `sudo dmidecode -t bios`) |
| Battery (BAT1) | NVT FRANDBA, **156 charge cycles**, 78.3 / 85.0 Wh (≈92 % of design capacity) |
| AC/DC at probe | Discharging, 30.7 W draw |
| Sleep mode | `mem_sleep_default=deep` (kernel cmdline) — good |

Framework 16 specifics that matter for maintenance:
- Firmware is distributed via **LVFS / `fwupdmgr`** — no
  Windows trip required.
- The AMD variant has a **dGPU expansion module** (see §3);
  expansion modules can be hot-seated, so topology can
  legitimately change between boots.
- Known upstream concerns: s2idle battery drain and
  post-suspend throttling on early firmware revisions. Check
  BIOS notes before opening a ticket.

## 2. CPU & Memory

| Field | Value |
|---|---|
| CPU | **AMD Ryzen AI 9 HX 370 w/ Radeon 890M** (Zen 5, family 26, model 36) |
| Topology | 1 socket · 12 cores · 24 threads |
| L2 / L3 | 12 MiB / 24 MiB |
| Boost | 5.16 GHz max · 605 MHz min |
| ISA notables | AVX-512 (F/DQ/BW/VL/VNNI/BF16/VBMI/VBMI2/BITALG/VPOPCNTDQ/IFMA/VP2INTERSECT), SHA-NI, AMX-free |
| RAM | **91 GiB** usable (≈96 GiB physical) |
| Swap | 8.0 GiB (3.2 GiB in use at probe) |

An NPU (AMD XDNA / Ryzen AI) is part of this SoC family;
probe with `lspci | grep -i "Signal Processing Controller"`
or `xrt-smi examine` (if XRT is installed). Treat XDNA as
optional until software catches up.

## 3. Graphics

Two AMD GPUs present simultaneously, both accessed by Russell's
GPU probes at `/sys/class/drm/card*/device/`.

| Slot | DRM | PCI ID | Device | VRAM | Role |
|---|---|---|---|---|---|
| `c4:00.0` | `card1` | `0x1002:0x7480` Navi 33 | **Radeon RX 7700S** (dGPU) — `gfx1102` | ~8 GiB | Heavy inference / compute |
| `c5:00.0` | `card2` | `0x1002:0x150e` Strix | **Radeon 890M** (iGPU) | ~512 MiB | Display, light compute |

- **Russell targets `card1`** (the dGPU) for GPU probes. The
  path is hardcoded (`/sys/class/drm/card1/device/`). On
  machines with a different DRM topology, GPU probes return
  `None`.
- ROCm installed: **7.2.0** under `/opt/rocm-7.2.0` + `/opt/rocm`
- `rocm-smi` is **not currently installed** on this machine;
  GPU probes use sysfs directly (no subprocess).
- **Known hazard:** ROCm 7.2.0 has an open regression that
  causes VRAM over-allocation / OOM with Ollama on Navi 3x /
   Strix iGPUs (upstream issue [ROCm #5902]). The `okapi-watcher`
   skill monitors GPU health as part of the sentinel cycle.

## 4. Storage

| Device | Size | Purpose | FS |
|---|---|---|---|
| `nvme0n1p2` | 3.6 TB | **root `/`** | ext4 |
| `nvme0n1p1` | 1 GB | `/boot/efi` | vfat |
| `nvme1n1` | 1.8 TB | unmounted spare | — |
| `sda` / `sdb` | 2 × 931 GB | removable, exFAT, `/media/mdz/*` | exfat |

Room for growth is generous. No RAID. No BTRFS/ZFS
snapshots — snapshots in this design are handled by
**Timeshift + rsync to the spare NVMe** (see §8 of the
plan). `fstrim.timer` is already running weekly.

## 5. Operating System

| Field | Value |
|---|---|
| Distro | **Ubuntu 25.10** (*Questing Quokka*, non-LTS) |
| Kernel | `6.17.0-20-generic` |
| Init | systemd with timers already driving: `fwupd-refresh`, `apt-daily{,-upgrade}`, `fstrim`, `man-db`, `logrotate`, `systemd-tmpfiles-clean`, `sysstat-*`, `e2scrub_all`, etc. |
| Snaps | **38** installed (includes Codium, browsers, Rust toolchains, core20–24) |
| Flatpaks | **12** installed |

Because 25.10 is non-LTS, **end of standard support is July
2026**. A deliberate upgrade-planning checkpoint belongs on
the annual cadence — don't let the OS silently fall out of
support. (Track OS lifecycle in
[`docs/status/CONSOLIDATED-STATUS.md`](docs/status/CONSOLIDATED-STATUS.md).)

## 6. Toolchain & Workloads

### Rust
- `rustup` 1.28.2; **five toolchains installed**: stable,
  nightly, 1.75, 1.90, 1.94.1 (candidate for pruning)
- `~/.rustup` and `~/.cargo` are material on disk (measured
  by the existing health script)
- Cargo-installed binaries: `cargo-machete`, `chrysalis`,
  `rust-mcp-server`, `sccache`, `tokei`, `udql`

### Editors
- **Zed Preview 0.233.1** at `~/.local/bin/zed`, config at
  `~/.config/zed/` (keymap, settings, themes)
- **VSCodium 1.105.17075** via Snap (`/snap/bin/codium`)
- Extensions loaded in Codium skew heavily to AI coding:
  `saoudrizwan.claude-dev` (Cline), `rooveterinaryinc.roo-cline`,
  `kilocode.kilo-code`, `danielsanmedium.dscodegpt`,
  `factory.factory-vscode-extension`, `kombai.kombai`
- Browser stack (from Snap list): Brave, Firefox, Chromium

### AI / runtime
- **Ollama 0.20.4** — `ollama.service` enabled & active,
  override drop-in present (backend toggled — see
  `~/Clones/scripts/ollama/`)
- Node 24.14 (via nvm) · npm 11.11
- Podman 5.4.2 (no Docker daemon)

### Scratch code
- `~/Clones/` holds ~20 working trees (`peripheral`,
  `arsenal`, `UDQL`, `dbhub`, `fal`, `fermi`, `slate`,
  `ubuntu_mcp_server`, `zed-rules`, `scripts`, `russell`,
  loose notes, etc.). Repo hygiene is a first-class concern.

## 7. Fingerprinting commands (re-run to refresh)

```bash
# chassis + firmware
cat /sys/devices/virtual/dmi/id/{sys_vendor,product_name,product_version,bios_version,bios_date}
fwupdmgr get-devices --no-unreported-check

# cpu / mem / topology
lscpu ; numactl -H 2>/dev/null ; free -h ; cat /proc/cmdline

# gpus
lspci -nnk | grep -EA3 'VGA|3D|Display'
rocminfo 2>/dev/null | grep -E 'Name|Marketing|gfx'
cat /sys/class/drm/card*/device/vendor 2>/dev/null

# storage + health
lsblk -o NAME,SIZE,MODEL,SERIAL,TYPE,FSTYPE,MOUNTPOINT
sudo smartctl -H /dev/nvme0n1 ; sudo smartctl -A /dev/nvme0n1

# battery
upower -i $(upower -e | grep BAT)

# runtime state
systemctl --failed ; systemctl --user --failed
systemctl list-timers --all --no-pager

# installed toolchain
rustup show ; ls ~/.cargo/bin
zed --version ; codium --version ; codium --list-extensions
node -v ; npm -v ; ollama --version
snap list ; flatpak list ; apt list --upgradable 2>/dev/null | head
```

All of the above are read-only. They are the first thing the
**Intake** module of Russell runs. Output is stored as the
"patient chart" referenced by every subsequent hygiene task.

## 8. Russell Probe Coverage

As of 2026-05-11, Russell's Sentinel collects the following
probes from this machine every 5 minutes:

| Probe | Typical value | Notes |
|---|---|---|
| `mem_available_mib` | ~77,000 MiB | 91 GiB installed |
| `mem_pressure_some_pct` | 0% | Near-zero on this machine |
| `mem_pressure_full_pct` | 0% | — |
| `swap_used_mib` | ~3,500 MiB | 8 GiB swap configured |
| `loadavg_1m` | 0.4–2.0 | 12-core × 2-thread |
| `proc_total_count` | ~600 | Above 500 warn threshold |
| `proc_zombie_count` | 5 | Persistent Snap orphans |
| `proc_stuck_count` | 0 | D-state would indicate I/O hang |
| `proc_running_count` | 1–8 | — |
| `proc_top_cpu_name` | varies | Text probe |
| `proc_top_mem_name` | varies | Text probe |
| `proc_top_mem_pct` | 1–14% | — |
| `gpu_vram_used_pct` | 55–86% | RX 7700S VRAM varies with LLM load |
| `gpu_temp_c` | 28–34°C | — |
| `gpu_util_pct` | 0–90% | Spikey |
| `disk_root_used_pct` | 26% | 885G / 3.6T root fs |
| `disk_io_pressure_some_pct` | 0–1% | Low on NVMe |
| `disk_io_pressure_full_pct` | 0% | — |
| `net_tcp_connections` | ~39 | IPv4 sockets in use |
| `net_tcp6_connections` | ~2 | IPv6 sockets in use |
| `systemd_degraded` | 0 | Bool: 1 = degraded |
| `systemd_user_failed_count` | 5 | Persistent Snap scope failures |
| `systemd_system_failed_count` | 0 | — |

Okapi health is monitored via the `okapi-watcher` skill, which
queries tokens, requests, errors, GPU memory, and adapter counts
during sentinel cycles.
