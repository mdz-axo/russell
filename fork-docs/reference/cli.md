---
title: "Russell CLI Reference"
audience: [operators, developers, contributors, agents]
last_updated: 2026-05-24
togaf_phase: "D"
version: "1.1.0"
status: "Active"
---

# Russell CLI Reference

<!-- TOGAF_DOMAIN: Technology -->
<!-- VERSION: 1.1.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-24 -->

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

## 3. Skill Lifecycle

Skills follow a defined lifecycle:

```
discovered → evaluated → installed → active → stale_warning → deprecated → retired
```

Skills in `deprecated` or `retired` state are not loaded by the
harness. Files remain on disk until manually deleted (JR-7:
persistence is auditable). Lifecycle management is available via
`russell skill` subcommands and the ACP session interface.

## 4. ACP Server

The ACP (Agent Client Protocol) server is Russell's primary interface
for agent integration. It runs as a separate binary:

```
russell-acp-server
```

The server implements JSON-RPC 2.0 over stdio with macaroon OCAP
authentication. Agents create sessions, query Russell's health
data, dispatch skills (filtered by visibility), and provide consent
for interventions. See [`../deployment/acp-integration.md`](../deployment/acp-integration.md).

## 5. Skill Catalogue

### Actionable Skills (with probes/interventions)

| Skill | Probes | Interventions | Risk |
|---|---|---|---|
| `okapi-watcher` | 3 (health, models, gpu-libs) | 1 (restart-okapi) | Low |
| `sysadmin` | 8 (systemd-failed, degraded, clock, zombies, journal, coredumps, swap, stale-mounts) | 8 (reset-failed, force-clock-sync, reap-zombies, journal-vacuum, etc.) | Low–Medium |
| `scenario-tester` | 9 (run-okapi, run-acp, run-sentinel, evaluate, report, journal, full, test-capability-attenuation, test-prompt-sanitization) | 0 | None |
| `oom-watcher` | 1 (check-oom-kills) | 0 | None |
| `skill-manager` | 3 (list-skills, stats, check) | 4 (install, prune, restore, delete) | Low |

### Knowledge Skills (data interpretation only, no probes)

`web-search`, `skill-discovery`, `skill-maintenance`,
`pragmatic-cybernetics`, `pragmatic-semantics`, `ubuntu-jack`,
`package-checker`, `journal-compactor`

## 5. Systemd Integration

Russell runs as a user systemd service:

```
russell-sentinel.timer  — every 5 minutes
russell-sentinel.service
russell-digest.timer    — Sunday 09:00
russell-digest.service
russell-failure@.service — templated failure capture
russell-acp-server.service — ACP server for agent integration
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