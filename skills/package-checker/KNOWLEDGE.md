# package-checker KNOWLEDGE.md
# Context for Jack when managing system packages across multiple managers.

## Purpose

This skill enables Jack to:
1. **Detect** which package managers are available on the system
2. **Fuzzy-match** package names across ALL installed package managers
3. **Report versions** of installed packages from any manager
4. **List all installed** packages with their versions
5. **Check for available updates** across the system (apt)
6. **Update individual packages** via apt (requires sudo)
7. **Update all packages** via apt (requires sudo, higher risk)
8. **Install npm packages globally** (requires sudo, IDRS-compliant)
9. **Validate package names** before install attempts (catches invalid characters)

## Supported Package Managers

| Manager | Probes | Interventions | Notes |
|---------|--------|---------------|-------|
| apt/dpkg | match, check-version, list, check-updates | update-package, update-all | Primary system manager |
| npm | npm-check-version, npm-list-global | npm-install | Global packages only |
| snap | snap-check-version, snap-list | (none — use `snap install` manually) | Read-only for safety |
| pip | pip-check-version | (none — use `pip install --user` manually) | Read-only for safety |

## When Jack Should Use This

### Scenario 1: User asks about a specific package

```
User: "What version of cline is installed?"
Jack: "Let me check across all package managers."
ACTION: package-checker/check-version cline
```

If Jack knows the package is from npm specifically:
```
ACTION: package-checker/npm-check-version cline
```

### Scenario 2: User wants to install an npm package

```
User: "Install cline"
Jack: "Cline is an npm package. I can install it globally."
ACTION: package-checker/npm-install cline
  → Jack proposes: package-checker/npm-install (risk: Low [needs sudo])
  → Say 'ok' to approve
```

### Scenario 3: User gets an npm install error

```
User: "npm install -g cline~ gives 404!"
Jack: "The tilde (~) isn't allowed in npm package names. The real package is just 'cline'."
ACTION: package-checker/npm-check-version cline
[If not installed]
Jack: "Want me to install it?"
ACTION: package-checker/npm-install cline
```

### Scenario 4: User asks which package managers are available

```
User: "Can I install something with npm?"
Jack: "Let me check what's available."
ACTION: package-checker/detect-managers
```

### Scenario 5: User asks about a snap package

```
User: "What version of the snap node is installed?"
Jack: "Let me check."
ACTION: package-checker/snap-check-version node
```

### Scenario 6: System maintenance

```
Jack: "I notice your system hasn't been updated in 14 days. Want me to check for security updates?"
ACTION: package-checker/check-updates
```

## Common Package Name Errors

Jack should watch for these common mistakes and correct them proactively:

| Error | Cause | Fix |
|-------|-------|-----|
| `npm install cline~` | Tilde appended to name | Install `cline` (no tilde) |
| `npm install @scope/pkg~` | Tilde after scoped package | Install `@scope/pkg` |
| Package not found in one manager | Wrong manager assumed | Search across all managers with `match-package` |
| `npm` not found but `node` exists | Missing npm binary | Install npm via apt: `sudo apt install -y npm` |
| Ancient npm version (v6) | Snap-installed node (v10 era) | Remove snap node, install via apt |

## Probe Behavior

### detect-managers

Reports which package managers are installed and their versions:

```bash
ACTION: package-checker/detect-managers
```

Returns: List of available managers with versions and paths.

### match-package

Fuzzy-matches a pattern across ALL installed package managers:

```bash
ACTION: package-checker/match-package
Arguments: {"pattern": "ollama"}
```

Returns: Matches grouped by package manager.

### check-version

Reports the version of a package, searching all managers (or a specific one):

```bash
ACTION: package-checker/check-version
Arguments: {"package": "ollama"}

# Or specify a manager:
ACTION: package-checker/check-version
Arguments: {"package": "cline", "manager": "npm"}
```

Returns: `[manager] Installed: pkg=ver` or `Not installed: pkg (checked: apt, npm, snap, pip)`

### npm-check-version

Checks npm global packages specifically:

```bash
ACTION: package-checker/npm-check-version
Arguments: {"package": "cline"}
```

Returns: Installed version + registry availability check. Validates package name for invalid characters.

### npm-list-global

Lists all globally installed npm packages:

```bash
ACTION: package-checker/npm-list-global
```

### snap-check-version / snap-list

Snap-specific probes:

```bash
ACTION: package-checker/snap-check-version
Arguments: {"package": "node"}

ACTION: package-checker/snap-list
```

### pip-check-version

Pip-specific probe:

```bash
ACTION: package-checker/pip-check-version
Arguments: {"package": "requests"}
```

## Intervention Behavior

### npm-install

Installs an npm package globally (IDRS-compliant):

```bash
ACTION: package-checker/npm-install
Arguments: {"package": "cline"}
```

- **Idempotent:** Reinstalling = same end state
- **Dry-run:** `RUSSELL_DRY_RUN=1` shows what would happen
- **Rollback:** `npm-uninstall` reverses the installation
- **Structured log:** Emits EVENT record on success

Requires sudo. Validates package name before attempting install.

### update-package

Upgrades a single apt package:

```bash
ACTION: package-checker/update-package
Arguments: {"package": "ollama"}
```

Requires sudo. Runs `apt-get install --only-upgrade -y <package>`.

### update-all

Upgrades all apt packages:

```bash
ACTION: package-checker/update-all
```

Requires sudo. Runs `apt-get upgrade -y`. Higher risk because it updates everything.

## Safety Rules

1. **Probes are read-only** — No system state changes.
2. **npm-install is low-risk** — Single package, reversible via npm-uninstall.
3. **update-package is low-risk** — Single apt package, reversible.
4. **update-all is medium-risk** — Updates everything, requires explicit consent.
5. **No autoremove/remove** — This skill doesn't remove packages (that would be a separate skill).
6. **Package name validation** — npm install blocks invalid characters (~ ' ! ( ) *) before they reach npm.

## Error Handling

Scripts exit with:
- `0` — Success
- `1` — Package not found / usage error
- `2` — Already up to date
- `3` — Package manager error (lock held, network issue, etc.)

## Node/npm Compatibility Warning

On Ubuntu systems, Node.js and npm may be installed from multiple sources:
- **apt** (`nodejs` package): Modern but may not include npm separately
- **snap** (`node` snap): May bundle an ancient npm (v6) with node v10
- **nvm**: User-managed, not visible to system package managers

Jack should use `detect-managers` to understand the situation and warn about
version mismatches. The recommended setup is:
1. Install `nodejs` via apt for the runtime
2. Install `npm` via apt for the package manager
3. Remove the snap `node` to avoid PATH conflicts

## Telemetry

Jack tracks:
- How often each probe runs
- Update success/failure rates
- Time since last system update
- npm install success/failure rates

## Integration with System Updates

This skill is complementary to `system-updater` (which handles dist-upgrades, kernel updates, and reboots). Use `package-checker` for routine package maintenance and cross-manager queries.