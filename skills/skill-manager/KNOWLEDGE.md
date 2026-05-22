# skill-manager KNOWLEDGE.md
# Context for Jack when managing skills autonomously.

## Purpose

This skill enables Jack (the Nurse) to manage the skill lifecycle autonomously from within `russell chat`. Instead of requiring manual CLI commands, Jack can:

1. **Build** new skill skeletons when users describe capability gaps
2. **Install** skills that have been created on disk
3. **Prune** skills that haven't been evaluated in 30+ days
4. **Restore** deprecated skills to active status
4. **Retire** skills (archive and remove from disk)
5. **Restore from archive** retired skills

## Workflow Patterns

### Pattern 1: Capability Gap → Build → Install

```
User: "I need to check if Ollama is outdated"
Jack: "I'll build a package-checker skill for that. Want me to install it too?"
ACTION: skill-manager/build package-checker
[After build completes]
ACTION: skill-manager/install package-checker
[After install completes]
ACTION: package-checker/check-version ollama
```

### Pattern 2: Stale Skill → Prune

```
Jack: "The gpu-doctor skill hasn't been evaluated in 45 days. Want me to prune it?"
ACTION: skill-manager/prune gpu-doctor
```

### Pattern 3: Retire → Restore from Archive

```
Jack: "The old-logger skill is retired but archived. Want me to restore it?"
ACTION: skill-manager/restore-from-archive old-logger
```

## Safety Rules

1. **Build is low-risk** — Creates a skeleton manifest and directory, no execution.
2. **Install is low-risk** — Updates registry cache, skill runs on next load.
3. **Prune is low-risk** — Marks as deprecated, files remain on disk.
4. **Retire is medium-risk** — Removes files, requires archive first.
5. **Restore from archive is medium-risk** — Restores archived files, reversible.

## Script Arguments

All scripts accept the skill name as the first argument:

```bash
./scripts/build-skill.sh package-checker
./scripts/install-skill.sh package-checker
./scripts/prune-skill.sh package-checker
```

## Error Handling

Scripts exit with:
- `0` — Success
- `1` — Skill not found (for install/prune/restore)
- `2` — Already in target state (e.g., installing already-installed skill)
- `3` — Invalid skill name or path

## Registry Integration

All interventions update `~/.local/share/harness/registry/local-cache.yaml` and journal the transition via `RegistryCache::journal_transition`.

## Evaluation

After any intervention, the `verify-skill-exists` check runs to confirm the skill directory and manifest exist (for build/install) or are removed (for retire).

## Telemetry

The skill registry tracks:
- `probe_runs` — Count of probe executions
- `intervention_runs` — Count of intervention executions
- `recent_probe_failures` — Failed probes in last 10 runs
- `recent_intervention_failures` — Failed interventions in last 10 runs
- `avg_probe_duration_ms` — Average probe execution time
- `last_probe_run_at` — ISO timestamp of last probe run
- `coverage_score` — How many symptoms this skill covers

Jack uses this telemetry to recommend skill pruning, restoration, or creation.