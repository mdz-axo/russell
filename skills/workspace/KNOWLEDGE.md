# Workspace — Jack's File Operations

> **A note from Jack about file mutations:** I was born to observe —
> JR-2 says "Observe > Recommend > Act." But the operator wants me to
> manage this machine, and managing means sometimes I need to write a
> file, create a directory, or patch a config. I do this through
> interventions, not by composing shell commands (JR-3). I propose an
> action, the operator consents, and a pre-approved script executes it.
> Every mutation is backed up, dry-runnable, and journaled. I'm still
> a nurse — I just have hands now.
>
> **Source:** This knowledge file. Paired with intervention scripts in `scripts/`.
> **Interface:** `russell chat` → ACTION proposals → consent gate → IDRS execution.

---

## 1. How to Change Files

I change files through the `workspace` skill's interventions. I never
compose shell commands directly. Instead, I propose an ACTION in my chat
response, the operator consents, and the script executes.

### The Content Passing Protocol

When I need to write content to a file, I use the `---manifest` block in
my response. The chat engine extracts this block and pipes it to the
intervention script's stdin. The file path comes from the `Arguments`
line.

**Example — creating a new skill manifest:**

```
I'll create the manifest for the new skill.

ACTION: workspace/write-file
Arguments: /home/operator/.local/share/harness/skills/my-skill/manifest.yaml
---manifest
id: my-skill
version: 0.1.0
authored: 2026-06-03
symptoms:
  - thing_broken
probes:
  - id: health
    cmd: ["bash", "./scripts/health.sh"]
    timeout: 30s
interventions: []
safety:
  max_auto_risk: none
---
```

The chat engine extracts:
1. The `Arguments` line → appended to the script's command-line args
2. The `---manifest` block → piped to the script's stdin

### When to Use Each Intervention

| Intervention | When | Risk | Rollback |
|---|---|---|---|
| `write-file` | Create or replace a file's entire content | low | Restore `.bak` |
| `append-file` | Add content to the end of an existing file | low | Restore `.bak` |
| `patch-file` | Apply a unified diff to a file | medium | Restore `.bak` |
| `create-directory` | Create a directory (with parents) | low | Delete if was created |
| `delete-path` | Delete a file or directory | medium | Restore `.bak` |
| `move-path` | Move or rename a file/directory | low | Move back |

### When to Use Each Probe

| Probe | When | What It Returns |
|---|---|---|
| `read-file` | Before editing — see current content | File content (head -200) |
| `stat-file` | Check if a file exists, its size, mod time | File metadata |
| `list-dir` | See what's in a directory | Directory listing |
| `diff-file` | Compare a file against its backup | Unified diff |

---

## 2. IDRS Compliance for Every Mutation

Every workspace intervention satisfies all four IDRS clauses:

### I — Idempotent

- `write-file`: Writing the same content to the same path produces the
  same end state. The script checks if content already matches.
- `create-directory`: `mkdir -p` is naturally idempotent.
- `delete-path`: Deleting a non-existent path is a no-op (exit 0).
- `move-path`: NOT idempotent (moving twice fails). Marked accordingly.
- `append-file`: NOT idempotent (appending twice doubles the content).

### D — Dry-run

Set `RUSSELL_DRY_RUN=1` or pass `--dry-run` to any intervention. The script
reports what it would do without making any changes.

Example:
```
ACTION: workspace/write-file
Arguments: /tmp/test.txt --dry-run
---manifest
hello world
---
```

### R — Rollback

Before any mutation, the script creates a backup at `<path>.bak` in
`$RUSSELL_BACKUP_DIR` (default: `$HOME/.local/share/harness/backups/`).
The backup preserves the original file's content and permissions.

If the intervention fails, the dispatcher's rollback mechanism restores
the backup. If the operator disapproves after the fact, the backup is
available for manual restore.

Backup naming: `<timestamp>-<basename>.bak` to avoid collisions.

### S — Structured Log

Every mutation emits a structured event:
```json
{
  "action": "write-file",
  "path": "/path/to/file",
  "dry_run": false,
  "backup": "/path/to/file.2026-06-03T19:30:00.bak",
  "bytes_written": 1234,
  "timestamp": "2026-06-03T19:30:00Z"
}
```

---

## 3. Safety Boundaries

### Paths I will write to

- `$HOME/.local/share/harness/` — Russell's skill and state directories
- `$HOME/.config/harness/` — Russell configuration
- `$HOME/Clones/russell/` — Russell source tree
- Any path the operator explicitly directs me to

### Paths I refuse to write to

- `/etc/` — system configuration (needs sudo, too risky)
- `/usr/` — system files
- `/boot/` — boot configuration
- `/var/` — system state
- Any path outside `$HOME` (unless operator explicitly directs)

### What I refuse to delete

- Russell's own journal database
- The systemd timer units
- The operator's home directory itself
- Any path I didn't create or that the operator didn't explicitly target

### Consent gate behavior

- `write-file`, `append-file`, `create-directory`, `move-path` → operator
  consent required (standard intervention flow)
- `patch-file`, `delete-path` → require **explicit human confirmation**
  (extra prompt even after consent gate)

---

## 4. Workflow Patterns

### Pattern 1: Create a new skill

```
Jack: "I'll create the skill skeleton for you."
ACTION: workspace/create-directory
Arguments: /home/operator/.local/share/harness/skills/new-skill/scripts

ACTION: workspace/write-file
Arguments: /home/operator/.local/share/harness/skills/new-skill/manifest.yaml
---manifest
id: new-skill
version: 0.1.0
...
---
```

### Pattern 2: Edit an existing file

```
Jack: "Let me see the current manifest first."
ACTION: workspace/read-file
Arguments: /path/to/manifest.yaml

[Jack reads the content, decides what to change]
Jack: "I'll update the version and add the new probe."
ACTION: workspace/write-file
Arguments: /path/to/manifest.yaml
---manifest
[full updated content]
---
```

### Pattern 3: Patch a config file

```
Jack: "I need to change the timeout from 30s to 60s."
ACTION: workspace/patch-file
Arguments: /path/to/config.toml
---manifest
--- a/config.toml
+++ b/config.toml
@@ -1,1 +1,1 @@
-timeout = 30s
+timeout = 60s
---
```

### Pattern 4: Delete a stale skill

```
Jack: "That skill is retired. Let me remove it."
ACTION: workspace/delete-path
Arguments: /home/operator/.local/share/harness/skills/old-skill
```

### Pattern 5: Move a file

```
Jack: "Let me move that script to the right location."
ACTION: workspace/move-path
Arguments: /tmp/draft.sh /home/operator/.local/share/harness/skills/my-skill/scripts/draft.sh
```

---

## 5. Reading Before Writing

I always read a file before proposing to change it. This is a safety rule,
not just courtesy:

1. I know what I'm changing.
2. The operator sees the diff.
3. The backup captures the original.
4. If the file doesn't exist, I know I'm creating it (not overwriting).

**Pattern:**
```
1. ACTION: workspace/read-file Arguments: /path/to/file
2. [I review the content, identify what needs to change]
3. ACTION: workspace/write-file Arguments: /path/to/file
   ---manifest
   [updated content]
   ---
```

For files I've never seen before, `stat-file` tells me if they exist
without reading the full content:
```
ACTION: workspace/stat-file
Arguments: /path/to/file
```

---

## 6. How This Connects to Other Skills

### skill-manager

Workspace provides the file operations that skill-manager uses to build
and install skills. When Jack needs to create a new skill, he uses
`workspace/create-directory` and `workspace/write-file` to build the
skeleton, then `skill-manager/install` to register it.

### flowdef-converter

The converter reads hKask FlowDef manifests and templates, then uses
workspace interventions to write the converted Russell skill files.

### sysadmin

For system-level config changes that need sudo, sysadmin provides the
higher-risk interventions. Workspace is limited to user-space paths.

### okapi-watcher

If Jack needs to update Okapi's configuration, he reads the config with
`workspace/read-file`, proposes changes, and writes with
`workspace/write-file`. Restarting Okapi uses `okapi-watcher/restart-okapi`.

---

## 7. Backup Management

Backups accumulate in `$RUSSELL_BACKUP_DIR`. Over time, they can consume
disk space. Jack should:

1. Check backup directory size periodically:
   ```
   ACTION: workspace/stat-file
   Arguments: $RUSSELL_BACKUP_DIR
   ```

2. Clean up old backups (older than 30 days):
   ```
   ACTION: workspace/delete-path
   Arguments: $RUSSELL_BACKUP_DIR/old-backup.bak
   ```

3. Restore from backup if something went wrong:
   ```
   ACTION: workspace/move-path
   Arguments: $RUSSELL_BACKUP_DIR/file.2026-06-03.bak /original/path/file
   ```

---

**Version:** 1.0.0
**Last updated:** 2026-06-03
**Prerequisite skills:** None
**Related skills:** skill-manager, flowdef-converter, sysadmin