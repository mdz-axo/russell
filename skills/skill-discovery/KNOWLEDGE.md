# Skill Discovery — Jack's Skill Lifecycle Manager

> **A note from Jack about finding skills:** Russell starts small (JR-1). But
> he has to grow — the machine gets new hardware, new workloads, new failure
> modes. When a problem appears that no existing skill can probe, that's a
> capability gap. The operator can write a skill. Or I can find one. The web
> search MCP bridge lets me search for skills, evaluate them, and guide the
> operator through installation. This file teaches me the full skill
> lifecycle: discover, validate, install, verify.
>
> **Source:** This knowledge file. Derived from ADR-0007 (skill manifest
> schema), the symptom catalog (`crates/russell-skills/src/symptom_catalog.rs`),
> and the existing skill corpus.
> **Requires:** `skills/web-search/KNOWLEDGE.md` (the MCP bridge).

---

## 1. The Skill Lifecycle

```
Gap detected → Search for skill → Evaluate candidate → Validate manifest
  → Check dependencies → Guide installation → Verify loaded → Ready
```

Each stage has rules. Most failures happen at validation or dependency
checking. I know those rules and enforce them before the operator copies
a single file.

---

## 2. Detecting Capability Gaps

When should I suggest finding or building a new skill?

| Trigger | Gap | Skill Category Needed |
|---|---|---|
| "I wish Russell watched X" | No probe exists for X | Monitoring / telemetry skill |
| "Can you restart X when it fails?" | No intervention exists for X | Sysadmin / recovery skill |
| "I don't know how X works on this machine" | Knowledge gap | Knowledge skill (KNOWLEDGE.md only) |
| A new service is deployed | No probe for that service | Service-specific skill |
| New hardware installed | No probe for that hardware | Hardware-specific skill |
| Operator asks about something outside the bundle | Unknown domain | Knowledge skill for the domain |
| A symptom fires with no skill to address it | Intervention gap | Remediation skill |

When I spot a gap, I say so explicitly: "Russell doesn't watch X. Want me
to search for a skill that does?"

---

## 3. Searching for Skills

### Where skills live

Skills for Russell are shared via:
- GitHub repositories (most common — `skills/` directories in Russell forks)
- Direct URLs (a manifest.yaml hosted anywhere accessible)
- The operator's local filesystem (they wrote one and want to install it)
- Kask's arsenal catalogue (if skills are registered there)

### Search queries

```
# General pattern
brave_web_search(query="russell skill monitor <domain> site:github.com", count=5)

# Specific examples
"russell skill nvidia GPU probe site:github.com"
"russell skill postgres health check site:github.com"
"russell skill disk S.M.A.R.T. monitor site:github.com"
"russell skill docker container watch site:github.com"
"russell skill kubernetes node health site:github.com"
"russell skill network latency probe site:github.com"
"russell skill apt security update check site:github.com"
```

Search tips:
- Add `site:github.com` to focus on repos (most skills live there)
- Try `file:manifest.yaml` if the search supports it
- Use `language:yaml` to find raw manifest files
- Try without `russell` to find general-purpose monitoring manifests that
  could be adapted

### What to search when results are thin

If no Russell-specific skill exists, search for the underlying capability:
1. "Linux procfs <topic> monitoring script" — probe scripts
2. "systemd service health check bash" — intervention scripts
3. "<tool> best practices 2026" — knowledge to build from

A general-purpose script can be wrapped in a Russell skill manifest. That's
JR-6 in action: reuse, don't depend.

---

## 4. Evaluating a Candidate Skill

When I find a skill, I evaluate it against these rules before suggesting
installation:

### 4.1 Manifest Validation (poka-yoke)

The manifest MUST pass all of these:

```
☐ File exists: skills/<id>/manifest.yaml
☐ `id` field matches the directory name
☐ `version` is valid semver
☐ `authored` is ISO 8601 date
☐ `min_harness_version` is compatible (≤ running Russell version)
☐ Every symptom in `symptoms:` is in the catalog (SYMPTOMS in symptom_catalog.rs)
☐ Probes have no `risk` field (risk is "none" by construction)
☐ Every intervention has `risk`, `rollback` strategy
☐ Rollback IDs resolve within the same manifest (if `rollback_id` used)
☐ Every `.sh`/`.py`/`.bash` in `scripts/` is referenced by a `cmd:` entry
☐ `safety.max_auto_risk` is ≤ the harness's configured cap
```

If any check fails, I report specifically which one and how to fix it.

### 4.2 Script Evaluation

```
☐ Scripts exist at the paths referenced in cmd:
☐ Scripts are executable (or will be after chmod +x)
☐ Scripts use #!/usr/bin/env bash (or python3) — portable shebangs
☐ Scripts don't require sudo unless needs_sudo: true in manifest
☐ Scripts don't write outside ~/.local/state/harness/ (Russell's sandbox)
☐ Scripts don't call package managers (apt install, pip install) — side effects
☐ Scripts don't modify system configs (/etc) — IDRS violation unless intervention
```

Red flags in scripts:
- `curl | bash` patterns — no pipeline execution
- Hardcoded paths (`/home/username/...`) — not portable
- `sudo ...` without consent gating
- Network calls in probes without documentation

### 4.3 Safety Evaluation

```
☐ max_auto_risk matches the worst intervention's risk
☐ require_human_for lists any interventions that always need consent
☐ No intervention claims rollback: none_needed when it modifies state
☐ No probe mutates anything (probes are always read-only)
```

---

## 5. Symptom Catalog Extension

If a candidate skill uses symptoms not in Russell's catalog, we have two
paths:

### Path A: Add to the catalog (for durable, reusable symptoms)

The symptom catalog is in `crates/russell-skills/src/symptom_catalog.rs`.
Adding a symptom requires:

1. The symptom name is kebab-case, descriptive, and not already in the list
2. It represents a real failure mode or condition Jack can reason about
3. It's added alphabetically within its category

Example addition:
```rust
// In symptom_catalog.rs, under the appropriate category:
"nvme_endurance_warning",
```

Per ADR-0007, extending the catalog "requires a short ADR." In practice,
this means documenting the new symptom in the skill's own documentation
and noting it in the PR.

### Path B: Map to existing symptoms (for overlap cases)

If the candidate's symptom is a synonym for an existing one, map it:
- Candidate says `gpu_overheating` → catalog has `gpu_temp_high`
- Candidate says `ram_exhausted` → catalog has `oom_killer_active`
- Candidate says `disk_full` → catalog has `resource_exhaustion`

The manifest must use catalog symptoms. The skill author should update
their manifest.

---

## 6. Skill Installation Guide

When the operator asks to install a skill, use the skill-manager meta-skill:

### Preferred: skill-manager verbs (from chat or jack)

```
# Activate a skill already on disk:
ACTION: skill-manager/install
Arguments --name <skill-id>

# Create a full manifest directly:
ACTION: skill-manager/create-manifest
---manifest
id: <skill-id>
...
---

# Build a skeleton first:
ACTION: skill-manager/build
Arguments --name <skill-id>
```

All skill-manager interventions require operator consent (low/medium risk).
Probes (`list-skills`, `stats`, `check`) auto-execute for telemetry.

### Manual fallback (shell commands for the operator)

### Quick install (knowledge-only skill)

```bash
# Copy the skill directory
cp -r /path/to/skill /home/$USER/.local/share/harness/skills/<skill-id>/

# Verify it loads
russell skill list
```

### Full install (actionable skill with scripts)

```bash
# 1. Copy the skill
SKILL_DIR=/home/$USER/.local/share/harness/skills/<skill-id>
mkdir -p "$SKILL_DIR"
cp /path/to/skill/manifest.yaml "$SKILL_DIR/"
cp -r /path/to/skill/scripts/ "$SKILL_DIR/scripts/"
cp /path/to/skill/KNOWLEDGE.md "$SKILL_DIR/"  # if present

# 2. Make scripts executable
chmod +x "$SKILL_DIR/scripts/"*

# 3. If new symptoms needed: edit symptom_catalog.rs, rebuild

# 4. Rebuild and reinstall
cd /path/to/russell
cargo build --release
./install.sh

# 5. Restart the sentinel timer
systemctl --user restart russell-sentinel.timer

# 6. Verify
russell skill list
russell skill run <skill-id>
```

### Install with symptom changes

If the skill needs new symptoms:
1. Add them to `crates/russell-skills/src/symptom_catalog.rs`
2. Run `cargo build --release` and `./install.sh`
3. Verify: `russell skill list` should show the skill without errors

---

## 7. Building a Skill from Scratch

When no skill exists and the operator wants me to help design one:

### Knowledge skill template

```yaml
# skills/<id>/manifest.yaml
id: <kebab-case-id>
version: 0.1.0
authored: YYYY-MM-DD
min_harness_version: 0.1.0

symptoms:
  - <from-catalog>

applies_when:
  - os_family: linux

probes: []
interventions: []

safety:
  max_auto_risk: none

references:
  - <sources>
```

### Actionable skill template (with probes)

```yaml
id: <kebab-case-id>
version: 0.1.0
authored: YYYY-MM-DD
min_harness_version: 0.1.0

symptoms:
  - <from-catalog>

applies_when:
  - os_family: linux

probes:
  - id: probe-<name>
    cmd: ["bash", "./scripts/probe-<name>.sh"]
    capture: stdout        # or stderr, or exit_code
    timeout: <duration>s
  - id: probe-<name2>
    cmd: ["bash", "./scripts/probe-<name2>.sh"]
    capture: stdout
    timeout: <duration>s

interventions:
  - id: <intervention-name>
    cmd: ["bash", "./scripts/<script>.sh"]
    risk: none|low|medium|high|critical
    idempotent: true|false
    rollback: none_needed|<rollback-id>|reboot
    timeout: <duration>s
    needs_sudo: false

safety:
  max_auto_risk: low
  require_human_for: []

evaluation:
  after_intervention:
    - id: verify
      cmd: ["bash", "-c", "<check command>"]
      expect_exit: 0
```

### Probe script template (bash)

```bash
#!/usr/bin/env bash
# probe-<name>.sh — <what this probes>
set -euo pipefail

# Collect the measurement
# Output exactly one value (number, boolean, or string) to stdout
echo "<value>"
```

### KNOWLEDGE.md template

```markdown
# <Title> — Jack's <Domain> Lens

> **A note from Jack about <topic>:** <One paragraph about what this
> teaches me and why it matters.>
>
> **Source:** <References and version info.>

## 1. <First section title>
...
```

---

## 8. Verifying a New Skill

After installation, I verify:

1. **Load test**: `russell skill list` — the skill appears without errors
2. **Probe test** (if actionable): `russell skill run <skill-id>/probe-<name>`
   — probe executes and returns output
3. **Jack awareness**: In the next `russell jack` or `russell chat` session,
   the skill's KNOWLEDGE.md content is in my context
4. **ACTION syntax**: I can propose `ACTION: <skill-id>/<probe-id>` and it
   resolves correctly

If any step fails, I identify the failure mode:
- Load fails → manifest validation error (see §4.1)
- Probe fails → script error or missing dependency
- Not in context → KNOWLEDGE.md missing or not loaded
- ACTION parse fails → skill ID or probe ID mismatch

---

## 9. Skill Hygiene

### When to update a skill

- New version of the monitored software changes its interface
- New symptoms discovered (add to manifest)
- Knowledge becomes stale (Ubuntu releases, kernel changes, API changes)
- Script efficiency improvements found

### When to retire a skill

- The monitored service is removed from the machine
- The hardware the skill targets is replaced
- A better skill supersedes it (keep the better one, retire the old)
- The skill's symptoms never fire (not watching anything useful)

Retirement: delete the directory from `~/.local/share/harness/skills/<id>/`
and it's gone. No uninstall ceremony. JR-1: austere.

### Sharing skills

Skills are self-contained directories. To share:
1. Push the `skills/<id>/` directory to a git repo
2. Or tar it: `tar czf <id>.tar.gz skills/<id>/`
3. Share the URL (github, gist, direct download)

---

## 10. Emergency Skill Bootstrap

If Russell is running and I need a skill RIGHT NOW and the operator is
asking for help:

```
Operator: "Jack, can you watch for OOM kills?"
Jack: "Not yet — no probe for it. But I can find one. Search for
'russell OOM killer probe site:github.com'. Or I can tell you how to
write one in 5 minutes. Which do you want?"
```

The fast-path template for a single probe skill:

```yaml
# emergency-oom-watcher/manifest.yaml
id: emergency-oom-watcher
version: 0.1.0
authored: "2026-05-13"
min_harness_version: 0.1.0
symptoms:
  - oom_killer_active
applies_when:
  - os_family: linux
probes:
  - id: check-recent-ooms
    cmd: ["bash", "-c", "dmesg -T | grep -i 'killed process' | tail -5 || echo 'none'"]
    capture: stdout
    timeout: 5s
interventions: []
safety:
  max_auto_risk: none
```

That's 14 lines. A `mkdir` and a `cp` and it's running. Jack can type the
manifest into chat and the operator pastes it. JR-1: when it needs to be
small, it's small.

---

**Version:** 1.0.0
**Last updated:** 2026-05-13
**Depends on:** web-search skill (MCP bridge)