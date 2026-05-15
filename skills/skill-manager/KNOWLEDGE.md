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
Arguments <skill-name>
```

### `build <name>`
Creates a minimal skill skeleton on disk. Writes a bare manifest.yaml
with empty probes/interventions — a starting point for further editing.
After building, use `create-manifest` to write the full manifest, or
install it as-is and `adapt` it in the workshop.
```
ACTION: skill-manager/build
Arguments <skill-name>
```

### `create-manifest <name>`
Writes a full skill manifest from content Jack provides. Include the
manifest YAML in a `---manifest` block after the ACTION line:
```
ACTION: skill-manager/create-manifest
---manifest
id: my-skill
version: 0.1.0
authored: 2026-05-15
symptoms: [gpu_temp_high, gpu_freq_throttle]
probes:
  - id: check-gpu
    cmd: ["nvidia-smi", "--query-gpu=temperature.gpu", "--format=csv,noheader"]
    risk: none
    timeout: 10s
interventions: []
---
```
The YAML's `id` field must match the CLI argument. The content is
safety-scanned before writing. The skill is registered as Active
immediately — no separate `install` step needed.

### `prune <name>`
Deprecates a skill (moves from active/stale → deprecated). Files
stay on disk. Can be undone with `restore`.
```
ACTION: skill-manager/prune
Arguments <skill-name>
```

### `restore <name>`
Restores a deprecated skill back to active.
```
ACTION: skill-manager/restore
Arguments <skill-name>
```
### `build <name>`
Creates a minimal skill skeleton on disk. Writes a bare manifest.yaml
with empty probes/interventions — a starting point for further editing.
After building, use `create-manifest` to write the full manifest, or
install it as-is and `adapt` it in the workshop.
```
ACTION: skill-manager/build
Arguments <skill-name>
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

- **Never prune bundled skills** (okapi-watcher, web-search, skill-discovery,
  skill-workshop, skill-maintenance, skill-manager, scenario-tester,
  pragmatic-cybernetics, pragmatic-semantics, ubuntu-jack).
  These ship with Russell and are maintained by the Russell team.
- **Check stats before pruning.** If `stats` shows recent failures,
  investigate rather than prune. The skill might need fixing, not
  removing.
- **Ask before deleting.** `delete` is irreversible. Always confirm
  with the operator before proposing this action.
- **Use create-manifest to write complete skills.** You can now write full
  skill manifests directly from chat — no workshop needed. Include the
  YAML in a `---manifest` block after the ACTION line.

## Example workflow

```
operator → I want a skill to monitor swap pressure

Jack → Let me check what's already installed first.
       ACTION: skill-manager/list-skills

[probe result shows no swap-watcher]

Jack → No skill covers swap_pressure. The workshop can build one.
     Start 'russell workshop' and use 'build swap-watcher' to create it.
     Then I can install it from here with 'install swap-watcher'.

operator → switches to workshop, builds swap-watcher, returns to chat

Jack → ACTION: skill-manager/install
       Arguments swap-watcher

[intervention result: swap-watcher installed and active]

Jack → swap-watcher is now loaded. Want me to run a health check?
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