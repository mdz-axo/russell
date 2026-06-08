---
title: "ADR-0050: Shell Commands Through the Consent Gate"
audience: [developers, architects, agents]
last_updated: 2026-06-05
ddmvss_context: "jack"
ddmvss_artifact: "adr"
version: "1.0.0"
status: "Accepted"
supersedes: "ADR-0008"
---

# ADR-0050: Shell Commands Through the Consent Gate

- **Status:** Accepted
- **Date:** 2026-06-05
- **Deciders:** Project founders
- **Tags:** `jack`, `shell`, `safety`, `consent`, `JR-3`
- **Supersedes:** ADR-0008 (LLM Never Emits Shell)

## Context

ADR-0008 established that "the LLM never emits shell." Every action
had to go through a registered skill manifest. This was intended to
prevent hallucinated commands from becoming mutations.

In practice, ADR-0008 made Jack unable to help with the most common
operator requests: installing packages, checking versions, reading
logs, diagnosing network issues. The LLM already understands these
tasks; the skill-gate architecture silenced that understanding. When
no skill covered a task, Jack could only say "That's not in my skill
bundle" — a menu-driven dead end that was worse than no system at
all.

The operator's experience: ask Jack to install a package, get told
he can't. The underlying LLM knows `npm install -g cline`; Jack's
architecture forbids him from saying it. 20,000 lines of code
subtracting value from the LLM's core competency.

The root design error: the system treated all LLM output as
executable code and constrained it accordingly. But LLM output
directed at a human operator is *speech*, not *code*. The trust
boundary should be at the **dispatcher** (what gets executed
automatically), not at the **LLM's mouth** (what gets said).

## Decision

Jack may propose shell commands using a new `SHELL:` prefix syntax:

```
SHELL: sudo npm install -g cline
```

Every `SHELL:` command goes through a safety classifier before
reaching the operator, and every command requires operator consent
before execution.

### The consent gate

1. **The LLM proposes** a command via `SHELL: <command>`.
2. **The safety classifier** assigns a risk band and checks for
   destructive patterns. Blocked commands are rejected with an
   error message explaining why.
3. **The operator sees** the command, its risk band, and whether
   it needs sudo. They say "ok" to approve or "no" to refuse.
4. **The dispatcher executes** the approved command via `bash -c`,
   captures stdout/stderr, journals the event, and returns the
   output to the LLM for interpretation.

### Safety classifier

| Category | Risk Band | Examples | Consent Required |
|----------|-----------|---------|-----------------|
| Read-only | none | `ls`, `cat`, `which`, `npm view`, `apt list`, `systemctl status` | Yes (always) |
| Low-risk mutation | low | `apt install`, `npm install -g`, `systemctl start` | Yes |
| High-risk mutation | medium | `apt remove`, `rm -rf`, `kill -9`, `systemctl stop` | Yes |
| Destructive | blocked | `rm -rf /`, `mkfs`, `shutdown`, `reboot`, fork bombs | Never |

The classifier is heuristic, not formal. It errs on the side of
caution: unknown commands default to low risk with consent required.

### Relationship to skills

`SHELL:` and `ACTION:` coexist:

- **ACTION:** for registered skills — IDRS guarantees (idempotent,
  dry-run, rollback, structured log), pre-approved command paths.
- **SHELL:** for ad-hoc commands — no IDRS guarantees, but the
  operator reviews every command before execution.

Prefer `ACTION:` when a skill exists. Use `SHELL:` when no skill
covers the task and the operator needs immediate help.

### Updated JR-3

**Old:** "The LLM never emits shell. It ranks IDs; it does not
compose commands."

**New:** "Shell commands go through the consent gate. Destructive
commands are blocked. The LLM proposes; the operator consents; the
dispatcher executes."

## Consequences

### Positive

- Jack can help with any task the operator would do at a shell.
- The LLM's core competency — understanding problems and
  articulating solutions — is no longer suppressed.
- Skills remain valuable for their IDRS guarantees, but are no
  longer the *only* execution path.
- Destructive commands remain blocked. The consent gate
  prevents accidental mutations.

### Negative / accepted costs

- The safety classifier is heuristic and may misclassify edge
  cases. A rare command could be under-classified (too low risk)
  or over-classified (too high risk). The operator's review
  is the backstop.
- No IDRS guarantees for shell commands. If `npm install -g cline`
  fails halfway, there's no automatic rollback. The operator
  must clean up manually.
- Shell commands are not pre-approved by manifest authors. The
  trust model shifts: the operator is trusted with the command
  (they review it), not a manifest author.

### Neutral

- `SHELL:` and `ACTION:` share the same consent gate UI. The
  operator experience is consistent.

## Alternatives considered

### Keep ADR-0008, add more skills

Rejected. This was the approach that failed: adding 6 scripts to
package-checker for npm/snap/pip support, when the LLM already
knew how to install an npm package. More scripts = more code to
maintain for less capability than the LLM provides for free.

### Allow shell commands only as "recommendations" (text, not execution)

Rejected. The operator would still have to leave the chat, type
the command, and come back. This breaks the conversational flow
and defeats the purpose. Jack should be able to *execute* the
command with consent, not just suggest it.

### Sandboxed execution (bubblewrap, seccomp)

Valuable as defense-in-depth, but insufficient alone. A sandboxed
`rm -rf ~/.cache` still deletes the cache. The consent gate is
the primary safety mechanism; sandboxing is a future enhancement.

### Function-calling with a generic `run_command(cmd: str)` tool

Rejected in ADR-0008 for the right reason: an unconstrained
`run_command` tool exposes too much surface. The `SHELL:` syntax
with the safety classifier is a constrained version of this: the
classifier blocks destructive patterns, and the consent gate
requires human review.

## Implementation notes

- `russell-meta::action` gains a `ShellCommand` variant in
  `ResolvedAction`, a `ShellBlocked` variant in `ActionError`,
  and a `classify_shell_command` function implementing the safety
  classifier.
- The `SHELL:` prefix is parsed before `ACTION:` in
  `resolve_action`.
- The chat handler routes `ShellCommand` through the consent gate.
- Shell execution uses `bash -c` (or `sudo -S -- bash -c` for
  sudo commands) with timeout and output capture.
- All shell executions are journaled as help-session events.
- **Consent sovereignty:** After operator consent ("ok", "yes",
  `/approve`), the dispatcher's risk cap is set to `Critical`,
  ensuring consented actions always execute. The `max_auto_risk`
  cap in skill manifests controls *whether consent is needed*, not
  *whether a consented action may proceed*. The operator's consent
  is sovereign. (Bug fix: 2026-06-07 — previously, `ShellCommand`
  set `max_auto_risk = None`, blocking all shell commands after
  consent; `Intervention` propagated the manifest cap, blocking
  consented actions above the skill's auto threshold.)

## References

- [ADR-0008](0008-llm-triage-never-emits-shell.md) — superseded
- [JR-3](../architecture/PRINCIPLES.md) — updated principle
- [AGENTS.md](../../AGENTS.md) — updated vocabulary and principles