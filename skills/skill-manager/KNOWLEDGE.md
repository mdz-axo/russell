# Skill Manager — Usage Guide for Jack

## What this skill is

The skill-manager gives you (Jack) hands-on management of the Russell
skill lifecycle from within `russell chat`. Before this skill existed,
you could only run probes and interventions — not install, modify, or
retire them. Now you can.

## Probes (auto-execute, read-only)

### `list-skills`
Lists all loaded skills with their probes and interventions.
```
ACTION: skill-manager/list-skills
```

### `stats`
Shows performance telemetry: probe runs, failures, last run time,
average duration, intervention counts.
```
ACTION: skill-manager/stats
```

### `check`
Audits all skills for staleness, coverage gaps, and quality scores.
```
ACTION: skill-manager/check
```

## Interventions (require operator consent)

### `install <name>`
Installs or activates a skill. If the skill directory already exists
on disk but isn't active, this moves it to `installed` → `active`.
If it's discovered (from a remote search), it needs to be fetched first.
```
ACTION: skill-manager/install
Arguments --name <skill-name>
```

### `prune <name>`
Deprecates a skill (moves from active/stale → deprecated). Files
stay on disk. Can be undone with `restore`.
```
ACTION: skill-manager/prune
Arguments --name <skill-name>
```

### `restore <name>`
Restores a deprecated skill back to active.
```
ACTION: skill-manager/restore
Arguments --name <skill-name>
```

### `delete <name>`
Permanently retires a skill: removes it from the registry cache and
deletes the directory from disk. Cannot be undone (no restore).
Use only after pruning and confirming the skill is no longer needed.
```
ACTION: skill-manager/delete
Arguments --name <skill-name>
```

## When to use each verb

| Situation | Verb | Risk |
|---|---|---|
| Just installed a new skill to disk | `install <name>` | low |
| A skill has been stale/warning for 30+ days | `prune <name>` | low |
| Operator asked you to un-prune something | `restore <name>` | low |
| Skill is obsolete, superseded, or broken | `delete <name>` | medium |
| Want to see what's installed and their health | `list-skills` / `stats` / `check` | none |

## Guardrails

- **Never prune bundled skills** (okapi-watcher, web-search, sysadmin,
  skill-manager itself). These ship with Russell and are maintained
  by the Russell team.
- **Check stats before pruning.** If `stats` shows recent failures,
  investigate rather than prune. The skill might need fixing, not
  removing.
- **Ask before deleting.** `delete` is irreversible. Always confirm
  with the operator before proposing this action.

## Example workflow

```
operator → I want a skill to monitor swap pressure

Jack → Let me check what's already installed first.
       ACTION: skill-manager/list-skills

[probe result shows no swap-watcher]

Jack → No skill covers swap_pressure. I can build one.
     Want me to create a swap-watcher skill?

operator → yes, go ahead

Jack → ACTION: skill-manager/build
       Arguments --name swap-watcher

[intervention result: Created skills/swap-watcher/manifest.yaml]
[Operator approves]

Jack → The skeleton is ready. The manifest needs editing to add
     a swap usage probe. I'll adapt it now.
     ACTION: skill-manager/adapt
     Arguments --name swap-watcher
```

## Registry Telemetry

The skill-manager's `stats` probe shows live usage data that the
registry tracks automatically. Every time a probe or intervention
runs in `russell chat`, the registry updates:

| Field | What it means |
|---|---|
| `probe_runs` | Total probe executions |
| `recent_probe_failures` | Recent failures (counter) |
| `last_probe_run_at` | ISO 8601 timestamp of most recent run |
| `avg_probe_duration_ms` | EWMA of run duration |
| `intervention_runs` | Total intervention executions |
| `coverage_score` | Quality score 0.0–1.0 |

Use `stats` to see the numbers. Use `check` for the audit summary.