---
title: "Russell Magna Carta"
audience: [architects, developers, agents]
last_updated: 2026-05-25
version: "1.0.0"
status: "Active"
domain: "Cross-cutting"
ddmvss_categories: [trust, lifecycle]
---

# Russell Magna Carta

**Purpose:** Define the operator sovereignty contract and single-host constraint that govern Russell's existence.

**Axiom:** *The operator is sovereign. Russell is a tool, not a service.*

---

## Operator Sovereignty

### S-1: The Operator Controls Russell, Not Vice Versa

**Statement:** Russell exists to serve a single human operator. The operator can:
- Stop Russell at any time (`systemctl --user stop russell-*`)
- Delete Russell's state (`rm -rf ~/.local/state/harness/`)
- Modify Russell's configuration (`~/.config/harness/`)
- Uninstall Russell entirely (`./packaging/bin/uninstall.sh`)

**Rationale:** A health harness that cannot be stopped is malware. A tool that resists removal is not a tool.

**Consequence:** Cost: Russell cannot enforce uptime guarantees. Buy: the operator always has the kill switch.

---

### S-2: Russell Does Not Phone Home

**Statement:** Russell makes no network connections unless explicitly configured to do so. The default configuration:
- No telemetry
- No update checks
- No crash reports
- No analytics

**Rationale:** A single-host tool has no reason to contact external services. Network access is opt-in, not opt-out.

**Consequence:** Cost: Russell cannot auto-update or report crashes. Buy: the operator's machine stays private.

---

### S-3: Russell Does Not Escalate Privileges

**Statement:** Russell runs as the operator's user. It does not:
- Request sudo access
- Install system-wide packages
- Modify system configuration
- Create system services

**Rationale:** A health harness that requires root is a security risk. A tool that modifies the system it monitors is a conflict of interest.

**Consequence:** Cost: Russell cannot fix system-level issues. Buy: Russell cannot break system-level things.

---

## Single-Host Constraint

### H-1: One Machine, One Operator

**Statement:** Russell monitors exactly one machine for exactly one operator. There is no:
- Multi-tenant mode
- Fleet management
- Cross-machine correlation
- Central aggregator

**Rationale:** Russell is a terrier, not a sheepdog. He watches one house, not a flock.

**Consequence:** Cost: Russell cannot scale to multiple machines. Buy: Russell stays simple, focused, and auditable.

---

### H-2: Local-First, Local-Only

**Statement:** All Russell state lives on the host machine:
- Journal: `~/.local/state/harness/journal.db`
- Profile: `~/.local/state/harness/profile.json`
- Evidence: `~/.local/state/harness/evidence/`
- Skills: `~/.local/share/harness/skills/`

No state is synchronized to external services.

**Rationale:** A single-host tool has no need for distributed state. Synchronization adds complexity and failure modes.

**Consequence:** Cost: Russell state is not backed up automatically. Buy: Russell state is always available, even offline.

---

### H-3: The Operator is the Policy Layer

**Statement:** Russell has no role-based access control, no multi-tenant auth, no permission model beyond "the user who launched systemd --user."

**Rationale:** A single-operator tool has no need for access control. The operator is both user and admin.

**Consequence:** Cost: Russell cannot distinguish between multiple users. Buy: Russell has no auth complexity.

---

## Trust Boundaries

### T-1: Russell Does Not Mutate Without Consent

**Statement:** Russell's default posture is **observe > recommend > act**. Any mutation requires:
- Explicit operator consent (via `russell chat` or `russell skill run`)
- IDRS contract compliance (idempotent, dry-runnable, rollback-able, structured-logged)
- Risk band enforcement (high-risk interventions blocked without consent)

**Rationale:** A health harness that acts without permission is dangerous. Consent is the operator's shield.

**Consequence:** Cost: Russell cannot auto-remediate. Buy: Russell cannot auto-break.

---

### T-2: The LLM is a Consultant, Not an Executor

**Statement:** The LLM (Okapi or OpenRouter) may:
- Rank differentials
- Compose summaries
- Explain symptoms

The LLM may **not**:
- Generate shell commands
- Execute mutations
- Bypass the skill manifest

**Rationale:** A hallucinating LLM with shell access is a disaster. The manifest is the boundary.

**Consequence:** Cost: Russell cannot improvise fixes. Buy: Russell cannot hallucinate mutations.

---

### T-3: Skills are the Only Mutators

**Statement:** All mutations flow through the skill dispatcher. No code path bypasses the manifest. No ad-hoc shell execution. No "just run this command."

**Rationale:** A skill manifest is a contract. Ad-hoc execution is a liability.

**Consequence:** Cost: Russell cannot run arbitrary commands. Buy: Russell only runs audited, IDRS-compliant commands.

---

## Lifecycle Constraints

### L-1: Russell is Installable and Uninstallable

**Statement:** Russell can be:
- Installed via `./packaging/bin/install.sh`
- Uninstalled via `./packaging/bin/uninstall.sh`
- Updated via `git pull && ./packaging/bin/install.sh`

All operations are idempotent and reversible.

**Rationale:** A tool that cannot be removed is not a tool. Reversibility is the operator's escape hatch.

**Consequence:** Cost: Russell cannot enforce persistence. Buy: Russell can always be removed cleanly.

---

### L-2: Russell State is Resettable

**Statement:** `rm -rf ~/.local/state/harness/` cleanly resets Russell. No orphaned state, no hidden caches, no "temporary" files that become permanent.

**Rationale:** A tool whose state cannot be reset is a black box. Resetability is the operator's sanity check.

**Consequence:** Cost: Russell loses history on reset. Buy: Russell can always start fresh.

---

### L-3: Russell is Auditable

**Statement:** Every mutation is logged to the journal with:
- Timestamp
- Skill ID
- Risk band
- IDRS compliance
- Evidence bundle reference

**Rationale:** A tool that cannot be audited is a black box. Auditability is the operator's trust mechanism.

**Consequence:** Cost: Journal grows over time. Buy: Every action is traceable.

---

## Violations

If Russell violates any clause of this Magna Carta, it is a bug. File an issue with:
- The violated clause (e.g., "S-2: Russell phoned home")
- Reproduction steps
- Expected behavior (per this document)
- Actual behavior

---

## References

- ADR-0001 (scope and charter)
- ADR-0005 (privileged operations, deferred)
- ADR-0008 (LLM triage never emits shell)
- ADR-0023 (lift ADR-0007 deferral, Phase 3 skills)
