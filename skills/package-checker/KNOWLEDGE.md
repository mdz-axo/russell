# package-checker KNOWLEDGE.md
# Context for Jack when managing system packages.

## Purpose

This skill enables Jack to:
1. **Fuzzy-match** package names against installed Debian packages
2. **Report versions** of installed packages
3. **List all installed** packages with their versions
4. **Check for available updates** across the system
5. **Update individual packages** (requires sudo)
6. **Update all packages** (requires sudo, higher risk)

## When Jack Should Use This

### Scenario 1: User asks about specific package version

```
User: "What version of Ollama is installed?"
Jack: "Let me check."
ACTION: package-checker/check-version ollama
```

### Scenario 2: User wants to update a package

```
User: "Is there a newer version of Ollama?"
Jack: "I can check for updates and upgrade it if available. Want me to?"
ACTION: package-checker/check-updates
[If update available]
ACTION: package-checker/update-package ollama
```

### Scenario 3: System maintenance

```
Jack: "I notice your system hasn't been updated in 14 days. Want me to check for security updates?"
ACTION: package-checker/check-updates
```

## Probe Behavior

### match-package

Fuzzy-matches a pattern against installed packages:

```bash
ACTION: package-checker/match-package
Arguments: {"pattern": "ollama"}
```

Returns all packages containing the pattern (case-insensitive).

### check-version

Reports the exact version of a specific package:

```bash
ACTION: package-checker/check-version
Arguments: {"package": "ollama"}
```

Returns: `Installed: ollama=1.2.3` or `Not installed: ollama`

### list-installed

Lists all installed packages with versions:

```bash
ACTION: package-checker/list-installed
```

Returns: Full list of `package=version` pairs.

### check-updates

Checks for available updates:

```bash
ACTION: package-checker/check-updates
```

Returns: List of packages with available updates, or "All packages up to date".

## Intervention Behavior

### update-package

Upgrades a single package:

```bash
ACTION: package-checker/update-package
Arguments: {"package": "ollama"}
```

Requires sudo. Runs `apt-get install --only-upgrade -y <package>`.

### update-all

Upgrades all packages:

```bash
ACTION: package-checker/update-all
```

Requires sudo. Runs `apt-get upgrade -y`. Higher risk because it updates everything.

## Safety Rules

1. **Probes are read-only** — No system state changes.
2. **update-package is low-risk** — Single package, reversible.
3. **update-all is medium-risk** — Updates everything, requires explicit consent.
4. **No autoremove/remove** — This skill doesn't remove packages (that would be a separate skill).

## Error Handling

Scripts exit with:
- `0` — Success
- `1` — Package not found
- `2` — Already up to date
- `3` — apt/dpkg error (lock held, network issue, etc.)

## Telemetry

Jack tracks:
- How often each probe runs
- Update success/failure rates
- Time since last system update

## Integration with System Updates

This skill is complementary to `system-updater` (which handles dist-upgrades, kernel updates, and reboots). Use `package-checker` for routine package maintenance.