---
title: "SOAP Bundle Template"
audience: [developers]
last_updated: 2026-04-18
togaf_phase: "C"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Information Systems -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-04-18 -->

<!--
audience: Doctor (machine-generated) and downstream readers
format: harness.soap.v1
reference_model: Weed, L. (1968). "Medical Records that Guide and Teach."
    New England Journal of Medicine 278(11):593-600.
    SOAP (Subjective, Objective, Assessment, Plan) is the standard
    clinical documentation format. Russell adapts it for machine
    health: Subjective = operator note, Objective = probe data,
    Assessment = LLM interpretation, Plan = proposed interventions.
-->

# SOAP — <evidence_id>

- **Evidence ID:** `<YYYYMMDDTHHMMSSZ-<symptom>-<shorthash>>`
- **Triggered by:** `sentinel | cli | tier-escalation`
- **Symptom class:** `<symptom>`
- **Skill used:** `<skill-id>` v`<skill-version>`
- **Scope:** `host | self` (self = proprioception / meta-Doctor run)
- **Duration:** <start> → <end> (<elapsed>)
- **Final state:** `resolved | pending_confirm | failed | no_skill`

## Subjective

Free-text context from the human operator (via `--note`) or the
triggering tier. If none, record `(none)`. Do not fabricate.

## Objective

### Samples

Last 60 minutes of probes relevant to this symptom, rendered as a
compact table. Full JSON at `samples.json` in the bundle.

| ts | probe | value | unit |
|---|---|---|---|

### Probe outputs

For each probe the dispatcher executed, link to its per-probe record:

- `<probe-id>` → `./<probe-id>.json` (rc=`<n>`, duration=`<ms>ms`)

### System snapshots

- `dmesg.log` — kernel ring buffer tail
- `systemctl-status.txt` — unit status for named services
- `lsblk.json`, `rocm-smi.json`, `sensors.json` — as applicable

## Assessment

LLM-assisted differential, ranked. Each hypothesis references
probe IDs that support or refute it. The LLM selected from the
loaded manifest's probe and intervention IDs; it did **not** emit
shell. Full transcript at `llm-transcript.jsonl`.

1. **<hypothesis A>** — supported by `<probe-id>`, `<probe-id>`.
   Confidence (self-reported): 0.xx.
2. **<hypothesis B>** — ...

## Plan

### Auto-applied (`risk <= max_auto_risk`, no confirmation required)

| step | id | risk | rc | notes |
|---|---|---|---|---|

### Deferred for human confirmation

| id | risk | reason | confirm command |
|---|---|---|---|
| <id> | medium | requires_confirmation | `russell confirm <evidence_id> --step <id>` |

### Evaluation

Post-intervention checks and their results. If any failed, the
rollback chain was executed and is recorded here.

| id | rc | expect | pass |
|---|---|---|---|

## Footer

- Bundle on disk: `~/.local/state/harness/evidence/<evidence_id>/`
- Journal event IDs: `<id1>, <id2>, ...`
- Related ADRs: ADR-0007, ADR-0008
