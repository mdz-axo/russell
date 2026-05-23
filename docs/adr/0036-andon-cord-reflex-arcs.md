# ADR-0036: Andon Cord for Reflex Arcs

- **Status:** Deferred (implementation removed; consent absorbed into ACP session interface)
- **Date:** 2026-05-19
- **Deciders:** Project founders
- **Tags:** `reflex-arcs`, `andon-cord`, `operator-consent`, `JR-2`

## Context

[`ADR-0021`](0021-proprioception-phase2-reflex-arcs.md) implemented reflex arcs that automatically propose interventions when probe thresholds are breached. However, interventions exceeding the auto-execution risk cap require explicit operator consent.

The Andon cord principle (from lean manufacturing) dictates that any operator can stop the line when they detect a problem. In Russell's context, the Andon cord is the operator's explicit approval/denial mechanism for reflex interventions.

## Decision

Implement `russell confirm` subcommand as the Andon cord for reflex arcs:

### Commands

```bash
russell confirm list                  # List pending reflex interventions
russell confirm <event-id>            # Approve a specific reflex intervention
russell confirm <event-id> --deny     # Deny a reflex intervention
```

### Implementation

- **Location:** Deferred. The `confirm.rs` CLI module was removed during the 2026-05-23 adversarial review. Consent functionality is planned to be absorbed into the ACP session interface (`russell-acp-server`), where hKask agents can surface `PendingAction` and receive consent responses.
- **Journal API:** `get_event(id: i64)` available on `JournalReadPort` trait
- **Persistence:** Approval/denial recorded as `reflex_confirmed`/`reflex_denied` events

### Security Model

- Only `reflex_proposed` events can be confirmed (validated by `action` field)
- Original event metadata preserved in confirmation record
- All confirmations/denials journaled with `tier: operator` for audit

### Workflow

1. Sentinel detects threshold breach → fires reflex arc → writes `reflex_proposed` event
2. Operator runs `russell confirm list` to see pending interventions
3. Operator runs `russell confirm <ID>` to approve or `--deny` to refuse
4. Confirmation/denial recorded in journal
5. For approved interventions, operator manually executes via `russell skill run <skill>/<action>`

## Consequences

### Positive

- Explicit operator consent for high-risk interventions (JR-2 compliance)
- Audit trail of all reflex arc decisions
- Separation of concerns: approval ≠ execution (operator retains final control)
- Non-urgent: interventions persist for 24h window

### Negative

- Manual execution step required (operator must run `russell skill run`)
- No automatic escalation if operator doesn't respond
- Event IDs are opaque (operator must read probe/intervention names carefully)

### Deferred

- Automatic intervention execution after confirmation (requires capability tokens)
- Escalation policy (e.g., auto-deny after 24h, SMS alert)
- Batch confirmation (approve/deny all at once)

## Compliance

- **JR-2:** Observe > Recommend > Act — consent gate enforced
- **JR-3:** LLM never emits shell — operator executes interventions manually
- **IDRS:** Structured log — all confirmations/denials journaled

## References

- [`AGENTS.md`](../../AGENTS.md) §5 — vocabulary (Andon cord definition)
- [`ADR-0021`](0021-proprioception-phase2-reflex-arcs.md) — reflex arc foundation
