---
title: "ADR-0005: Privileged Operations via PolKit (Deferred)"
audience: [developers, architects]
last_updated: 2026-04-18
togaf_phase: "H"
version: "1.0.0"
status: "Deprecated"
---

<!-- TOGAF_DOMAIN: Change Management -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Accepted — Deferred -->
<!-- LAST_UPDATED: 2026-04-18 -->

> **Deferred.** This ADR's subject is outside the MVP boundary per
> [`../../specifications/MVP_SPEC.md`](../../specifications/MVP_SPEC.md) §5.
> It remains **Accepted** — when its phase opens, it ships this way.


<!--
audience: contributors touching root-requiring actions
last-reviewed: 2026-04-17
-->

# ADR-0005: Privileged operations — user-scope first, PolKit for narrow exceptions

- **Status:** Accepted
- **Date:** 2026-04-17
- **Deciders:** Project founders
- **Tags:** `safety`, `privilege`, `polkit`, `systemd`

## Context

Most of Russell's actions are read-only probes or user-scope
mutations (restarting `ollama.service --user`, pruning
`~/.cache/thumbnails`, writing into `~/.local/state/harness/`).
A small minority legitimately need root:

- `fwupdmgr update` — firmware.
- `apt update && apt upgrade` — package updates.
- `smartctl -t long /dev/nvme0n1` — disk tests.
- `journalctl --vacuum-size=` on the system journal.

Running the main `russell` binary as root would expose a much
larger attack surface. Running it with `sudo` would prompt
interactively, breaking unattended cadences.

## Decision

1. **Default: user-scope systemd.** `russell` and all its
   timers run under `systemctl --user`. The service units
   explicitly clear capabilities beyond `CAP_NET_BIND_SERVICE=0`
   and set `NoNewPrivileges=yes`.

2. **For the short list of root-requiring actions, use
   PolKit.** Each such action gets:
   - a dedicated `org.russell.<action>` PolKit action,
   - an install-time `.policy` file under
     `/usr/share/polkit-1/actions/` (managed by the OS
     packager; Russell ships it as a file),
   - a narrow helper binary (`russell-helper-<action>`)
     invoked via `pkexec`. The helper does exactly one
     thing, validates its arguments, and exits.

3. **No generic "elevate this command" helper.** Each helper
   is purpose-built. Adding a new privileged action requires
   a new helper and a new PolKit action.

4. **Interactive authentication prompts are acceptable for
   Tier III** (quarterly) actions, because the operator is
   typically present. Tier II's `weekly/apt-upgrade` uses a
   PolKit action configured for `auth_admin_keep` so the
   prompt authorizes a 5-minute window, not a permanent
   elevation.

5. **Never embed passwords or sudoers rules.** PolKit
   `auth_admin` prompts are the sole authentication path.

## Consequences

### Positive

- Smallest possible root surface. The main binary never
  runs as root.
- Each privileged action has its own audit trail in journald
  (pkexec logs by default).
- Removing privilege escalation is trivial: delete the
  `.policy` file and the helper binary; Russell's core
  continues to function without the affected feature.

### Negative / accepted costs

- More binaries to build and package (one helper per
  privileged action).
- Tier II timers that need privilege cannot be fully
  unattended; `apt upgrade` effectively requires the
  operator be logged in with an active PolKit agent, or
  `auth_admin_keep` grace from a recent interaction. We
  accept a one-per-session prompt.

### Neutral

- PolKit is already installed on Ubuntu 25.10 and similar
  distros; no new runtime dep.

## Alternatives considered

### sudo with NOPASSWD

Rejected. Writing into `/etc/sudoers.d/` from Russell is a
major safety regression. "Passwordless root for a user's
automation" is exactly the capability we do not want to
grant quietly.

### Set-UID helper

Rejected. setuid binaries are a historical footgun; PolKit
with pkexec provides argument validation and per-action
policy that setuid cannot.

### Run the main binary under a system unit as root

Rejected. Inverts the blast-radius argument: now every bug
in Russell is a root-privilege bug.

### Use `systemd-run --user --scope` to escalate

Rejected. `systemd-run` does not itself grant privilege; it
would still need sudo or polkit underneath. Adds a layer
without solving the authentication problem.

## Implementation notes

- Helper binary crate naming convention:
  `crates/russell-helper-<action>` (to be created with the
  first privileged action).
- PolKit `.policy` files live under
  `packaging/polkit/` in the repo; the packager installs
  them.
- Each helper binary asserts `geteuid() == 0` at startup and
  aborts otherwise.
- Each privileged action's risk band is `medium` or higher;
  all invocations route through the propose /
  `confirm_proposal` pattern.

## References

- PolKit docs: https://www.freedesktop.org/software/polkit/docs/latest/
- `systemd.exec(5)` — `NoNewPrivileges`, `CapabilityBoundingSet`.
- [`../../standards/safety.md`](../../standards/safety.md)
- [ADR-0003](0003-mcp-transport.md) — no network listener.
