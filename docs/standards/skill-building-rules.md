---
title: "Skill Building Rules"
audience: [jack, agents, operators]
last_updated: 2026-05-21
togaf_phase: "Phase 3"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Governance -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-21 -->

# Skill Building Rules

> How to create valid skills that the dispatcher will accept.
> Read this before `ACTION: skill-manager/build` or manual edits.

## 1. Command Path Validation (CRITICAL)

Every skill command (`cmd:` array in probes/interventions) MUST pass path validation. The dispatcher rejects invalid paths at load time.

### Valid Command Patterns

| Pattern | Example | Status |
|---|---|---|
| **Interpreter + relative script** | `["bash", "./scripts/foo.sh"]` | ✓ Valid |
| **Interpreter + absolute script** | `["bash", "/home/user/skills/foo.sh"]` | ✓ Valid |
| **Absolute system command** | `["/usr/bin/systemctl", "--user", "restart", "okapi"]` | ✓ Valid |
| **Allowed interpreter** | `["sh", "-c", "echo hi"]` | ✓ Valid |
| **Bare command name** | `["russell", "skill", "list"]` | ✗ REJECTED |
| **PATH lookup** | `["python3", "script.py"]` | ✗ REJECTED |
| **Traversal attempt** | `["bash", "../escape.sh"]` | ✗ REJECTED |

### Allowed Interpreters

These command names are permitted without path prefix:

```
sh, bash, dash, python3, python, perl, ruby
```

### Why This Rule Exists

- **JR-2 (Observe > Recommend > Act):** Skills must be explicit about what they execute.
- **Security:** Prevents accidental PATH hijacking or ambiguous command resolution.
- **Auditability (JR-7):** The manifest declares exact execution targets.

### Fixing Bare Commands

If you see this error:

```
command path validation failed cmd=russell error=bare command name "russell" rejected
```

Change:
```yaml
cmd: ["russell", "skill", "list"]
```

To:
```yaml
cmd: ["bash", "./scripts/list-skills.sh"]
```

Then create `scripts/list-skills.sh`:
```bash
#!/usr/bin/env bash
russell skill list
```

**Note:** If `russell` is not in PATH (common during development), use the `RUSSELL_BIN` environment variable pattern:

```bash
#!/usr/bin/env bash
set -euo pipefail
RUSSELL_BIN="${RUSSELL_BIN:-/home/user/Clones/russell/target/debug/russell}"
"$RUSSELL_BIN" skill list
```

This allows runtime override via `RUSSELL_BIN=/custom/path/russell`.

## 2. Skill Directory Structure

```
~/.local/share/harness/skills/<id>/
  manifest.yaml              # Required: skill definition
  scripts/                   # Required: probe/intervention scripts
    probe-foo.sh
    intervention-bar.sh
  KNOWLEDGE.md               # Optional: context for Jack
```

## 3. Manifest Fields (Required)

```yaml
id: <kebab-case-id>          # e.g. journal-viewer, gpu-doctor
version: 0.1.0               # semver; bump on any manifest change
authored: YYYY-MM-DD
min_harness_version: 0.1.0

symptoms:
  - <symptom_class>          # e.g. high_process_count, llm_slow

applies_when:
  - os_family: linux         # Required precondition

probes:
  - id: <probe_id>
    cmd: ["bash", "./scripts/<id>.sh"]
    risk: none               # Probes MUST be risk: none
    timeout: 30s

interventions:               # Empty array [] if none
  - id: <intervention_id>
    cmd: ["bash", "./scripts/<id>.sh"]
    risk: low                # none | low | medium | high | critical
    idempotent: true
    rollback: none_needed    # OR rollback_id: <reverse_id>

safety:
  max_auto_risk: low         # Cap for auto-execution
```

## 4. Script Guidelines

### Probe Scripts (risk: none)

- Must NOT mutate system state
- Must exit with code 0 on success
- Output captured to stdout/stderr per manifest
- Example:

```bash
#!/usr/bin/env bash
# scripts/show-host-samples.sh
set -euo pipefail
cat <<'EOF'
Host samples table available in Objective section.
EOF
```

### Intervention Scripts (risk: low+)

- MUST satisfy IDRS:
  - **Idempotent:** Second run = first run's end state
  - **Dry-run:** Support `--dry-run` flag or `RUSSELL_DRY_RUN=1`
  - **Rollback:** Pre-state captured; reverse intervention exists
  - **Structured log:** Event written to journal
- Example:

```bash
#!/usr/bin/env bash
# scripts/prune-skill.sh
set -euo pipefail

SKILL_ID="${1:-}"
if [[ -z "$SKILL_ID" ]]; then
  echo "error: skill ID required"
  exit 1
fi

russell skill prune "$SKILL_ID"
```

## 5. Building Skills (Jack's Workflow)

### From Chat (`russell chat`)

1. **Discover gap:**
   ```
   Jack: The swap-pressure symptom has no installed skill.
   ```

2. **Propose build:**
   ```
   Jack: Want me to create a swap-watcher skill?
   operator → yes
   ```

3. **Build skeleton:**
   ```
   ACTION: skill-manager/build swap-watcher
   ```

4. **Add probe/intervention:**
   - Edit `~/.local/share/harness/skills/swap-watcher/manifest.yaml`
   - Add scripts to `scripts/`

5. **Install:**
   ```
   ACTION: skill-manager/install swap-watcher
   ```

### Manual Build

```bash
# Create directory structure
mkdir -p ~/.local/share/harness/skills/<id>/scripts

# Copy template
cp ../templates/skill-manifest.yaml \
   ~/.local/share/harness/skills/<id>/manifest.yaml

# Edit manifest (fill all required fields)
# Create scripts (use ./scripts/ paths in cmd:)

# Install
russell skill install <id>
```

## 6. Common Mistakes

| Mistake | Fix |
|---|---|
| Bare `russell` command in `cmd:` | Use `["bash", "./scripts/foo.sh"]` pattern |
| Missing `scripts/` directory | Create it; scripts MUST be inside |
| Probe with `risk: low` | Probes MUST be `risk: none` |
| No rollback on intervention | Add `rollback: none_needed` with justification OR `rollback_id` |
| Script not executable | `chmod +x scripts/*.sh` |
| Manifest missing required fields | `id`, `version`, `authored`, `symptoms`, `probes` |

## 7. Testing a Skill

```bash
# Verify manifest loads
russell skill list

# Run a probe
russell skill run <id>/<probe-id>

# Run an intervention (dry-run first)
russell skill run <id>/<intervention-id> --dry-run

# Check journal for events
russell journal tail
```

## 8. References

- [`safety.md`](safety.md) — IDRS contract
- [`../templates/skill-manifest.yaml`](../templates/skill-manifest.yaml) — starter template
- [`../architecture/skill-self-management-strategy.md`](../architecture/skill-self-management-strategy.md) — self-management design
- [`AGENTS.md`](../../AGENTS.md) — contributor orientation