# Jack as Sovereignty Tool: Analysis & Recommendations

**Date:** 2026-05-22  
**Author:** Kilo  
**Status:** Draft for Review  

---

## Executive Summary

Jack (the Nurse persona in Russell) is designed to enable **user sovereignty** over Linux machines through collaborative AI assistance. However, the current implementation has critical gaps that undermine this goal:

1. **Jack cannot create skills autonomously** — The `skill-manager` skill referenced in chat sessions doesn't exist, leaving Jack unable to fulfill his core promise of extending his own capabilities.

2. **Security policies block effective assistance** — Jack's refusals (JR-3: "LLM never emits shell") are correctly implemented but create friction when users need simple sysadmin tasks done.

3. **Skill creation workflow is broken** — The `build_skill` function generated invalid manifests (wrong field names, missing required fields), causing silent failures.

This document analyzes the design and provides concrete recommendations to make Jack more effective at enabling user sovereignty.

---

## 1. Current State Analysis

### 1.1 What Jack Does Well

| Strength | Implementation |
|----------|----------------|
| **ACTION: protocol** | Clean separation between LLM intent and execution. Jack proposes registered skill IDs only. |
| **Consent gates** | Interventions require explicit operator approval (`/approve`, "ok", "yes"). |
| **Risk bands** | Skills declare `max_auto_risk` caps; Jack cannot exceed them. |
| **Proprioception** | Russell watches himself (5 self-vitals tracked). |
| **IDRS contract** | All mutations are Idempotent, Dry-run capable, Rollback-enabled, Structured-logged. |

### 1.2 Critical Gaps (User Sovereignty Blockers)

| Gap | Impact on Sovereignty |
|-----|----------------------|
| **No skill-manager skill** | Jack cannot extend his own capabilities. Users must manually create YAML files and scripts. |
| **Manifest validation errors** | `build_skill()` generated invalid manifests (`risk_band` vs `max_auto_risk`, missing `rollback` fields). |
| **No package management skill** | Jack cannot check/update software (as shown in chat log). User must trust external package managers blindly. |
| **Nested ACTION detection too strict** | Jack's multi-step proposals (e.g., "create manifest, then install") trigger false positive security violations. |
| **No skill discovery pipeline** | Users cannot easily find/install skills from remote registries without manual `fetch` commands. |

---

## 2. Design Review: Jack's Role in User Sovereignty

### 2.1 The Sovereignty Contract

User sovereignty means:
- **No forced trust** — Users understand what Jack does before it runs.
- **Auditability** — Every action is logged and reversible.
- **Extensibility** — Users can teach Jack new tricks without waiting for upstream updates.
- **Transparency** — Jack explains what he's doing and why.

### 2.2 Where the Design Succeeds

```
┌─────────────────────────────────────────────────────────────┐
│                    User Sovereignty Stack                    │
├─────────────────────────────────────────────────────────────┤
│  Layer 5: Policy (Human) ← User sets risk caps, consents   │
│  Layer 4: Intelligence (Jack + LLM) ← Proposes actions      │
│  Layer 3: Control (Nurse) ← Enforces IDRS, consent gates   │
│  Layer 2: Coordination (Skills) ← Registered playbooks      │
│  Layer 1: Ops (Sentinel) ← Observes, never mutates          │
└─────────────────────────────────────────────────────────────┘
```

Jack operates at **Layer 4** — he proposes, user consents, skills execute. This is correct.

### 2.3 Where the Design Fails

**Problem 1: Skill Creation Barrier**

To add a new capability, users must:
1. Know YAML manifest schema
2. Write shell scripts with proper error handling
3. Understand IDRS rollback requirements
4. Run `russell skill install` manually

This violates sovereignty — users are **forced to trust** the manual process without Jack's assistance.

**Problem 2: Jack's Hands Are Tied**

When Jack says "I'll create a skill for that" (as in the Ollama check example), he **cannot follow through**. The `skill-manager` skill he references doesn't exist. This is a **broken promise** that erodes trust.

**Problem 3: Security Theater vs. Security**

The nested ACTION detection (Task 3.4) is good — it prevents prompt injection. But it also blocks Jack from proposing **multi-step workflows** like:
```
ACTION: skill-manager/build package-checker
ACTION: skill-manager/install package-checker
ACTION: package-checker/check-version ollama
```

The fix isn't to allow nested ACTIONs — it's to give Jack a **sequencing primitive** (e.g., `ACTION: skill-manager/sequence` with a list of steps).

---

## 3. Recommendations

### 3.1 Immediate Fixes (Phase 1)

| Fix | Priority | Effort |
|-----|----------|--------|
| **Create `skill-manager` skill** | Critical | 2-3 hours |
| **Fix `build_skill()` manifest template** | Critical | 30 min (DONE) |
| **Add `package-checker` skill** | High | 1 hour |
| **Add `system-updater` skill** | High | 1 hour |

#### 3.1.1 Skill Manager Skill Design

```yaml
id: skill-manager
version: 0.1.0
symptoms: [capability_gap]

probes:
  - id: list-skills
    cmd: ["bash", "./scripts/list-skills.sh"]
    timeout: 5s
  - id: stats
    cmd: ["bash", "./scripts/skill-stats.sh"]
    timeout: 5s
  - id: check
    cmd: ["bash", "./scripts/skill-check.sh"]
    timeout: 10s

interventions:
  - id: build
    cmd: ["bash", "./scripts/build-skill.sh"]
    risk: low
    rollback: none_needed
    needs_sudo: false
  - id: install
    cmd: ["bash", "./scripts/install-skill.sh"]
    risk: low
    rollback: none_needed
    needs_sudo: false
  - id: prune
    cmd: ["bash", "./scripts/prune-skill.sh"]
    risk: low
    rollback: restore-from-registry
    needs_sudo: false
  - id: retire
    cmd: ["bash", "./scripts/retire-skill.sh"]
    risk: medium
    rollback: restore-from-archive
    needs_sudo: false
```

**Key design decisions:**
- All scripts are **relative paths** (`./scripts/*.sh`) — no bare commands.
- Rollback strategies are **named interventions** (e.g., `restore-from-registry`).
- `build` and `install` are low-risk (no sudo, reversible).
- `retire` is medium-risk (deletes files, requires archive first).

### 3.2 Medium-Term Improvements (Phase 2)

| Improvement | Priority | Effort |
|-------------|----------|--------|
| **Skill sequencing primitive** | Medium | 4-6 hours |
| **Remote skill registry** | Medium | 8-12 hours |
| **Skill evaluation framework** | High | 12-16 hours |
| **Natural-language skill builder** | High | 16-20 hours |

#### 3.2.1 Sequencing Primitive

Add a new ACTION type: `ACTION: <skill>/sequence` with inline steps:

```
ACTION: skill-manager/sequence
Steps:
  1. build package-checker
  2. install package-checker
  3. package-checker/check-version ollama
```

Jack proposes the sequence; user consents once; Russell executes step-by-step with rollback on failure.

#### 3.2.2 Remote Registry Integration

Enable `russell workshop search --remote` to:
1. Query a community registry (YAML index on IPFS or GitHub)
2. Download skill manifests to `~/.local/share/harness/skills-pending/`
3. Safety-scan before installation
4. Show telemetry (probe runs, failure rates) before install

### 3.3 Long-Term Vision (Phase 3)

| Vision | Description |
|--------|-------------|
| **Skill marketplace** | Community-contributed skills with reputation scores, safety audits, and telemetry. |
| **Jack learns from failures** | When a skill fails, Jack proposes fixes and updates the manifest autonomously (with consent). |
| **Cross-machine sync** | Export skill registry + manifests to backup/restore on new machines. |
| **Skill composition** | Jack chains multiple skills to solve complex problems (e.g., "My GPU hangs during training" → gpu-doctor + okapi-watcher + scenario-tester). |

---

## 4. Making Skill Management Fun

### 4.1 Gamification Ideas

| Idea | Implementation |
|------|----------------|
| **Skill badges** | "First skill built", "100 probe runs", "Zero failures this week" |
| **Coverage score** | Show % of symptoms covered by installed skills. Jack nudges: "You're at 67% coverage — want to add a network-watcher skill?" |
| **Skill evolution** | Skills that are used frequently get "leveled up" (auto-suggested improvements from Jack). |
| **Community sharing** | One-command `russell skill publish <id>` to share skills with the community. |

### 4.2 UX Improvements

| Improvement | Why It Matters |
|-------------|----------------|
| **Visual skill map** | `russell skill graph` — shows which skills address which symptoms, dependencies between skills. |
| **Skill health dashboard** | `russell skill dashboard` — shows probe success rates, intervention outcomes, recent failures. |
| **Natural-language search** | `russell workshop "I need to watch my GPU temperature"` → suggests gpu-doctor skill. |
| **Skill templates** | `russell skill new --template apt-watcher` — pre-filled manifests for common patterns. |

### 4.3 Jack's Voice in Skill Management

Jack should be **playful but competent** when helping with skills:

**Bad (too robotic):**
> "Skill 'package-checker' created. Run 'russell skill install package-checker' to activate."

**Good (Jack's voice):**
> "I've built the package-checker skill — it's like a little hound for your apt database. Want me to install it? Then I can sniff out outdated packages for you."

**Better (proactive):**
> "I noticed you asked about Ollama updates. I can build a skill to track package versions — want me to whip that up? Takes about 30 seconds, and then I'll never let a stale package sneak past."

---

## 5. Security Considerations

### 5.1 Threat Model

| Threat | Mitigation |
|--------|------------|
| **Malicious skill injection** | Safety scanner (7 rules) runs on all manifests before install. |
| **Prompt injection via LLM** | Nested ACTION detection, poka-yoke dispatcher (only registered IDs execute). |
| **Privilege escalation** | Skills declare `needs_sudo`; Russell prompts for password at consent time. |
| **Data exfiltration** | Skills declare `allowed_env_keys` and `needs_network`; blocked in air-gapped mode. |

### 5.2 Recommended Hardening

1. **Skill signing** — Community skills are GPG-signed; local skills are unsigned (user trusts themselves).
2. **Sandboxing** — Run skill scripts in Firejail or Bubblewrap for high-risk skills.
3. **Rate limiting** — Max 10 skill executions per minute to prevent runaway loops.
4. **Audit trail** — Every skill execution is journaled with full evidence bundles.

---

## 6. Implementation Roadmap

### Week 1: Critical Fixes
- [x] Fix `build_skill()` manifest template
- [ ] Create `skill-manager` skill (manifest + scripts)
- [ ] Create `package-checker` skill
- [ ] Create `system-updater` skill

### Week 2-3: Phase 2 Features
- [ ] Skill sequencing primitive
- [ ] Remote registry integration (GitHub-based MVP)
- [ ] Skill evaluation framework (telemetry collection)

### Month 2: Polish & Fun
- [ ] Visual skill map (`russell skill graph`)
- [ ] Dashboard (`russell skill dashboard`)
- [ ] Gamification (badges, coverage score)
- [ ] Natural-language skill search

---

## 7. Conclusion

Jack's design for user sovereignty is **sound in theory** but **broken in practice**. The core issue is that Jack promises capabilities he cannot deliver (skill creation, package management) because the underlying skills don't exist.

**Priority order:**
1. Build the `skill-manager` skill so Jack can actually manage skills.
2. Build essential sysadmin skills (package-checker, system-updater) so Jack can handle routine maintenance.
3. Add sequencing primitives so Jack can propose multi-step workflows.
4. Make skill discovery and installation fun (gamification, visual tools).

The goal is a system where:
- **Users trust Jack** because he's transparent and auditable.
- **Users empower themselves** because Jack makes skill creation accessible.
- **Users maintain sovereignty** because every action is consensual and reversible.

---

## Appendix A: Fixed Manifest Example

```yaml
# skills/package-checker/manifest.yaml
id: package-checker
version: 0.1.0
authored: 2026-05-22
min_harness_version: 0.20.0

kind: actionable

symptoms:
  - package_out_of_date

applies_when:
  - os_family: linux

probes:
  - id: match-package
    cmd: ["bash", "./scripts/match-package.sh"]
    timeout: 5s
  - id: check-version
    cmd: ["bash", "./scripts/check-version.sh"]
    timeout: 5s
  - id: list-installed
    cmd: ["bash", "./scripts/list-installed.sh"]
    timeout: 5s

interventions:
  - id: update-package
    cmd: ["bash", "./scripts/update-package.sh"]
    risk: low
    idempotent: true
    rollback: none_needed
    needs_sudo: true

safety:
  max_auto_risk: low
  require_human_for:
    - update-package
```

## Appendix B: Script Examples

```bash
#!/usr/bin/env bash
# skills/package-checker/scripts/check-version.sh
set -euo pipefail

PACKAGE="${1:-}"
if [[ -z "$PACKAGE" ]]; then
    echo "Usage: check-version.sh <package-name>" >&2
    exit 1
fi

if dpkg-query -W -f='${Status}' "$PACKAGE" 2>/dev/null | grep -q "install ok installed"; then
    VERSION=$(dpkg-query -W -f='${Version}' "$PACKAGE")
    echo "Installed: $PACKAGE=$VERSION"
    exit 0
else
    echo "Not installed: $PACKAGE"
    exit 0
fi
```

```bash
#!/usr/bin/env bash
# skills/package-checker/scripts/update-package.sh
set -euo pipefail

PACKAGE="${1:-}"
if [[ -z "$PACKAGE" ]]; then
    echo "Usage: update-package.sh <package-name>" >&2
    exit 1
fi

# Check if running as root (should be, since needs_sudo: true)
if [[ $EUID -ne 0 ]]; then
    echo "This script must run as root (via sudo)" >&2
    exit 1
fi

apt-get update -qq
apt-get install --only-upgrade -y "$PACKAGE"

echo "Updated: $PACKAGE"
```

---

**Next Steps:**
1. Review this document with the user.
2. Implement Week 1 critical fixes.
3. Test skill-manager skill in `russell chat` sessions.
4. Iterate based on user feedback.
