---
title: "Russell CLI Reference"
audience: [operators, developers, contributors, agents]
last_updated: 2026-05-14
togaf_phase: "D"
version: "1.0.0"
status: "Active"
---

# Russell CLI Reference

<!-- TOGAF_DOMAIN: Technology -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-14 -->

Complete reference for every `russell` CLI command. Run `russell --help`
for the auto-generated usage summary; this document is the authoritative
manual.

## 1. Core Verbs

### `russell status`

Show the current machine snapshot from the journal's most recent
cycle. Format: summary table of numeric samples with severity markers.

```
russell status
```

Related: `russell-sentinel.service` (5-minute timer). This is a
read-only journal query; no side effects.

### `russell list`

List recent events from the journal.

```
russell list [--limit <N>]
```

Options:
- `--limit <N>` — number of events (default: 10)

### `russell profile [--init]`

Create or update the machine profile (`~/.local/share/harness/MACHINE_PROFILE.md`).

```
russell profile --init     # create a stub profile
russell profile             # refresh from current machine state
```

### `russell digest [--format <format>]`

Export the journal to readable formats.

```
russell digest --format daily-log   # daily Markdown log for memory/
russell digest                       # default summary
```

Formats: `daily-log`, `summary`.

### `russell sentinel-once`

Run one Sentinel cycle: collect host probes, evaluate against
`rules.d/*.toml`, journal results, run proprioception, and output a
summary. This is the unit of work scheduled by `russell-sentinel.timer`.

```
russell sentinel-once
```

Output: `sentinel: captured <N> samples, <M> threshold breaches in <ms> ms; proprio: age=<S>s stall=<S>s llm_p95=<M>ms drift=<S>s err_rate=<P>%`

Related files: `~/.config/harness/rules.d/agent-testing.toml`
(11 scenario-testing thresholds), built-in defaults at compile time.

### `russell jack`

Run a single Nurse session. Composes a SOAP bundle from the latest
journal data and sends it to the configured LLM backend (Okapi by
default). Prints Jack's response.

```
russell jack
```

### `russell chat`

Interactive multi-turn conversation with Jack. Each turn sends the
latest journal state to the LLM. Probes execute immediately;
interventions await consent (`/approve`, "yes", "ok", "go ahead").

```
russell chat
```

Commands during chat: `/help`, `/exit`, `/quit`, `/skills`, `/models`,
`/refresh`, `/reload`, `/history`, `/approve`, `/deny`.

Skill management during chat: `ACTION: skill-manager/list-skills`,
`ACTION: skill-manager/stats`, `ACTION: skill-manager/check`,
`ACTION: skill-manager/install`, `ACTION: skill-manager/prune`,
`ACTION: skill-manager/restore`, `ACTION: skill-manager/delete`.

### `russell okapi-probe`

Probe Okapi health: model list, GPU memory, adapter count.

```
russell okapi-probe
```

### `russell proprio`

Run Russell's self-observation cycle. Computes five self-vitals and
appends samples to the journal.

```
russell proprio
```

## 2. Skill Commands

### `russell skill list`

List all installed skills with their symptoms, probes, and
interventions. Validates against the symptom catalog (poka-yoke).

```
russell skill list
```

### `russell skill run <skill>/<step>`

Run a skill's probe or intervention. The step ID must match a
probe or intervention defined in the skill's manifest.

```
russell skill run okapi-watcher/probe-health
russell skill run scenario-tester/probe-scenario-run-okapi
russell skill run scenario-tester/probe-scenario-full
```

Options:
- `--dry-run` — print what would run without executing

The dispatcher respects the probe's `timeout:` field from the
manifest (e.g. `180s`, `5m`, `1h`). Default is 30s. Probe telemetry
(run counts, failures, duration EWMA) is recorded in the registry cache.

### `russell skill stats`

Show performance telemetry for all skills in the registry: probe runs,
failures, intervention counts, average duration (EWMA), and last run time.

```
russell skill stats
```

### `russell skill check`

Audit all installed skills for staleness (180-day threshold), coverage gaps
against the symptom catalog, and quality scores.

```
russell skill check
```

### `russell skill install <name>`

Install or activate a skill by name. Moves the skill to installed/active
status. Idempotent — safe to run on already-active skills.

```
russell skill install swap-watcher
```

### `russell skill prune <name>`

Deprecate a skill. Marks as deprecated; files remain on disk.
Reversible with `restore`.

```
russell skill prune swap-watcher
```

### `russell skill restore <name>`

Restore a deprecated skill back to active status.

```
russell skill restore swap-watcher
```

### `russell skill retire <name>`

Permanently retire a skill: removes from the registry cache and
deletes the skill directory from disk. Irreversible.

```
russell skill retire swap-watcher
```

### `russell workshop`

Interactive skill lifecycle REPL. Jack helps the operator discover,
evaluate, build, adapt, and maintain skills. See [Workshop Commands](#3-workshop-commands) below.

```
russell workshop
```

## 3. Workshop Commands

Built-in commands available inside the `russell workshop` REPL:

| Command | Description |
|---|---|
| `help` | Show the command guide |
| `/list` | List all skills with lifecycle status markers (✓ active, ⚠ stale, ✗ deprecated, • installed) |
| `/gaps` | Show symptom catalog entries with no installed skill |
| `/lookup <symptom>` | Which installed skills address this symptom? |
| `search <query>` | Search the local registry cache by name or symptom |
| `search --remote` | Search via Brave Search API (requires `BRAVE_API_KEY` env var). Falls back to local cache. |
| `fetch <url> <name>` | Download a skill manifest from URL, safety-scan it, and save to the skills directory |
| `evaluate <name>` | Show manifest, safety scan (manifest.yaml + KNOWLEDGE.md), and script listing |
| `build <name>` | Create a new skill skeleton (manifest.yaml) on disk at `~/.local/share/harness/skills/<name>/` |
| `adapt <name>` | Open the skill's manifest in `$EDITOR` (defaults to `vim`), re-scan on save |
| `check` | Audit all installed skills: staleness (>180 days, `valid_until` expiry), coverage gaps, score reporting |
| `prune <name>` | Deprecate a stale skill — marks as `deprecated`, files remain on disk |
| `restore <name>` | Move a deprecated skill back to `active` (alias: `unprune`) |
| `install <name>` | Move a discovered/evaluated skill to installed/active status |
| `/quit` | Exit the workshop (saves registry cache) |

Free-form text is routed to Jack (via Okapi) for interactive skill
design conversations.

### Lifecycle States

```
discovered → evaluated → installed → active → stale_warning → deprecated → retired
```
Skills in `deprecated` or `retired` state are not loaded by the
harness. Files remain on disk until manually deleted (JR-7:
persistence is auditable).

## 4. Skill Catalogue

### Actionable Skills (with probes/interventions)

| Skill | Probes | Interventions | Risk |
|---|---|---|---|
| `okapi-watcher` | 3 (health, models, gpu-libs) | 1 (restart-okapi) | Low |
| `sysadmin` | 8 (systemd-failed, degraded, clock, zombies, journal, coredumps, swap, stale-mounts) | 8 (reset-failed, force-clock-sync, reap-zombies, journal-vacuum, etc.) | Low–Medium |
| `scenario-tester` | 7 (run-okapi, run-chat, run-sentinel, evaluate, report, journal, full) | 0 | None |
| `oom-watcher` | 1 (check-oom-kills) | 0 | None |
| `skill-manager` | 3 (list-skills, stats, check) | 4 (install, prune, restore, delete) | Low |

### Knowledge Skills (data interpretation only, no probes)

`web-search`, `skill-discovery`, `skill-workshop`, `skill-maintenance`,
`pragmatic-cybernetics`, `pragmatic-semantics`, `ubuntu-jack`

## 5. Systemd Integration

Russell runs as a user systemd service:

```
russell-sentinel.timer  — every 5 minutes
russell-sentinel.service
russell-digest.timer    — Sunday 09:00
russell-digest.service
russell-failure@.service — templated failure capture
russell-okapi-probe.timer — once per minute
russell-okapi-probe.service
```

Manage with: `systemctl --user {enable,disable,start,stop} russell-*.{timer,service}`.

## 6. Environment Variables

| Variable | Default | Purpose |
|---|---|---|
| `RUSSELL_DOCTOR_BACKEND` | `okapi` | LLM backend: `okapi`, `mock`, `offline` |
| `RUSSELL_DOCTOR_OPENROUTER_KEY` | — | OpenRouter API key (opt-in) |
| `RUSSELL_DOCTOR_MODEL` | — | Override the default LLM model |
| `BRAVE_API_KEY` | — | Brave Search API key for `search --remote` |
| `EDITOR` | `vim` | Editor for `adapt <name>` |
| `RUSSELL_DRY_RUN` | — | Set to 1 for global dry-run mode |
| `RUST_LOG` | — | Tracing level: `info`, `debug`, `trace` |
| `SCENARIO_MODEL` | auto-detect | Model name for scenario-test probes |
| `SCENARIO_ITERATIONS` | `5` | Iterations for scenario-run-okapi |
| `WRITE_SAMPLES` | `0` | Set to 1 for scenario-evaluate to journal samples |

## 7. Data Layout

```
~/.config/harness/
  rules.d/              — operator threshold overrides (agent-testing.toml, etc.)
  russell.env           — environment configuration

~/.local/state/harness/
  journal.db            — SQLite journal (canonical data store)
  registry/
    local-cache.yaml    — skill registry cache (rebuildable)

~/.local/share/harness/
  skills/               — installed skills (manifests + scripts)
  MACHINE_PROFILE.md    — the patient's chart
  memory/               — derived Markdown layer (daily logs, reviews, test reports)
```