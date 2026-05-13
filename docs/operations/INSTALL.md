---
title: "Installing Russell"
audience: [operators]
last_updated: 2026-04-18
togaf_phase: "G"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Governance -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-04-18 -->

# Installing Russell

Russell runs as a user-scoped systemd service on Ubuntu 25.10 (and
any reasonably modern Linux with `systemd --user`). No root is
required. The binary lives in `~/.local/bin/russell`, the timers
in `~/.config/systemd/user/`, and Russell's state in
`~/.local/state/harness/`. See
[`../specifications/PERSISTENCE_CATALOG.md`](../specifications/PERSISTENCE_CATALOG.md)
for the full map.

## 1. Prerequisites

- A Linux system with `systemd --user` (Ubuntu 25.10 is the
  primary target; see [`../../MACHINE_PROFILE.md`](../../MACHINE_PROFILE.md)).
- `rustup` + stable Rust (see
  [`../../CONTRIBUTING.md`](../../CONTRIBUTING.md) §1).
- `sqlite3` in your `PATH` (only needed for the debug recipes below;
  Russell has SQLite statically-linked for its own use).
- Ollama installed and running (or auto-startable) with the
  `deepseek-v4-pro:cloud` model pulled. Russell's Doctor will attempt
  to start Ollama automatically, but does not manage model
  downloads — `ollama pull deepseek-v4-pro:cloud` is up to you.
- Optionally, an OpenRouter API key if you want Jack to consult
  a frontier cloud model instead. Russell works fine without
  one — Ollama is the default.

## 2. One-liner

From a freshly-cloned repo:

```sh
cd ~/Clones/russell
./packaging/bin/install.sh --release
```

`--release` builds the optimised binary. Drop it for a debug
build (faster to compile, slower to run — fine for development).

What the installer does, in order, all idempotent:

1. `cargo build` the `russell` binary.
2. Copies it to `~/.local/bin/russell`.
3. Installs the five systemd units to `~/.config/systemd/user/`.
4. Seeds `~/.config/harness/russell.env` from `.env.example` if
   no config exists yet (permissions `0600`).
5. Creates `~/.local/state/harness/` with the expected layout.
6. `systemctl --user daemon-reload`.
7. Enables + starts `russell-sentinel.timer` and
   `russell-digest.timer`.
8. Runs one `russell sentinel-once` to prove the wiring.
9. Prints `russell status`.

## 3. Configure Jack

Edit the config and add your API key:

```sh
${EDITOR:-nano} ~/.config/harness/russell.env
```

Fill in (only needed if using the openrouter backend):

```
# Optional — uncomment and fill in to use OpenRouter instead of local Ollama
# RUSSELL_DOCTOR_BACKEND=openrouter
# OPENROUTER_API_KEY=sk-or-v1-…
```

The other variables have sensible defaults; override only if you
know you need to.

### 3.1 Env discovery precedence

Russell looks for env in this order (first wins for the *file*;
process env *always* wins regardless):

1. `$XDG_CONFIG_HOME/harness/russell.env` — installer creates
   this. **Production location.**
2. `<repo-root>/.env` — convenience during development.
3. `./.env` — ad-hoc fallback.

## 4. Verify

```sh
systemctl --user list-timers 'russell-*'
journalctl --user -u russell-sentinel.service --since '10 min ago'
russell status
russell list --limit 5
russell jack --note "test"
```

`russell jack` should produce a response from the configured
LLM (local Ollama by default, or OpenRouter if you opted in),
or a Jack-voiced offline summary if Ollama is not running.
Either way, Jack speaks.

## 5. Regular Operation

Russell observes silently. You see him via:

| Command | When |
|---|---|
| `russell status` | Ad-hoc check on how things are |
| `russell list --limit 20` | Recent events |
| `russell digest --since-hours 168` | Weekly summary (also auto-rendered Sunday 09:00) |
| `russell jack --note "…"` | Ask Jack about something specific |

The Sentinel fires every 5 minutes. The weekly digest renders
every Sunday at 09:00 local (with up to 10 minutes jitter). If
either unit fails, the templated `russell-failure@.service`
captures the last 50 journal lines into
`~/.local/state/harness/runs/`.

## 6. Updating

To update Russell after pulling a new version:

```sh
cd ~/Clones/russell
git pull
./packaging/bin/install.sh --release
```

The installer is idempotent; running it again replaces the
binary + units and reloads systemd. Your config and state are
untouched.

## 7. Uninstalling

```sh
~/Clones/russell/packaging/bin/uninstall.sh
```

Removes the binary and units, stops and disables the timers.
**Your data survives** at `~/.local/state/harness/`.

To also remove the data:

```sh
~/Clones/russell/packaging/bin/uninstall.sh --purge
```

## 8. Troubleshooting

### Jack is always offline

Check whether Ollama is running:

```sh
curl -s http://localhost:11434/api/tags | head
# If this hangs or fails, Ollama is not running.
systemctl --user status ollama
# Russell auto-starts Ollama via systemctl --user start ollama.
# If that fails (no unit), install and enable Ollama first:
#   systemctl --user enable --now ollama
ollama list
# Check that deepseek-v4-pro:cloud is available:
#   ollama pull deepseek-v4-pro:cloud
```

### Sentinel hasn't run

```sh
systemctl --user status russell-sentinel.timer
systemctl --user list-timers russell-sentinel.timer
journalctl --user -u russell-sentinel.service -n 50
```

### Journal seems frozen

```sh
sqlite3 ~/.local/state/harness/journal.db \
  "SELECT MAX(ts) as last_sample, datetime(MAX(ts), 'unixepoch') FROM samples;"
```

If `last_sample` is older than 10 minutes and the timer says
it's been active, check `journalctl` for panic traces and email
the maintainer with the last 100 journal lines.

### The binary disappeared after an upgrade

Ubuntu sometimes cleans `~/.local/bin/` on distro major upgrade.
Re-run `./packaging/bin/install.sh --release` and you're back.

## 9. What gets installed, summary

| Path | Purpose | Removed by uninstall? |
|---|---|---|
| `~/.local/bin/russell` | The binary | yes |
| `~/.config/systemd/user/russell-sentinel.{timer,service}` | 5-minute observation cadence | yes |
| `~/.config/systemd/user/russell-digest.{timer,service}` | Weekly digest | yes |
| `~/.config/systemd/user/russell-failure@.service` | Failure capture | yes |
| `~/.config/harness/russell.env` | Your env | no (pass `--purge`) |
| `~/.local/state/harness/*` | Journal, evidence, runs, digest | no (pass `--purge`) |
| `~/.local/share/harness/skills/` | Reserved — empty in MVP | no (pass `--purge`) |

## 10. See also

- [`../specifications/MVP_SPEC.md`](../specifications/MVP_SPEC.md) — what Russell does and doesn't.
- [`../specifications/PERSISTENCE_CATALOG.md`](../specifications/PERSISTENCE_CATALOG.md) — every byte he writes.
- [`../standards/safety.md`](../standards/safety.md) — the IDRS contract.
- [`../architecture/THE_JACK.md`](../architecture/THE_JACK.md) — who Jack is.
