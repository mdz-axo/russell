---
title: "Safety Standard — IDRS, Risk Bands, Kill Switches"
audience: [developers, operators, agents]
last_updated: 2026-04-18
togaf_phase: "G"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Governance -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-04-18 -->

<!--
audience: every contributor proposing a mutating action
last-reviewed: 2026-04-17
-->

# Safety standard — IDRS, risk bands, honeymoon, andon cord

> "First, do no harm."
>
> Russell's default posture is **observe > recommend > act**.
> If you are adding code that mutates host state, this document
> is the contract you are agreeing to.

## 1. The IDRS contract

Every mutating action — a skill intervention, a CLI subcommand, an
MCP tool, a proprioception reflex — MUST satisfy all four:

### I — Idempotent

- Running the action twice converges to the same end state.
- The dispatcher provides `russell run --module <id>
  --verify-idempotent`, which executes the action, snapshots
  relevant state, executes it again, and diffs.
- Interventions whose natural form is "create X if absent"
  pass trivially. Interventions that toggle (e.g. "restart
  ollama") must record that a second invocation is a no-op
  in the structured log.

### D — Dry-run

- A `--dry-run` flag on the CLI, a `dry_run: true` key on an
  MCP mutating tool, and `RUSSELL_DRY_RUN=1` in the
  environment all route through the same code path: the
  dispatcher consults a `DryRun` guard before every
  subprocess spawn, file write, systemctl call, etc.
- In dry-run mode the action emits the `harness.event.v1`
  record it *would* have emitted, with `action: "would_<verb>"`
  and `dry_run: true`.
- New subsystems that spawn processes or write files MUST
  integrate with the `DryRun` guard before their first
  merge.

### R — Rollback

- Every mutating step captures pre-state **before** acting.
- Each intervention in a skill manifest declares exactly one of:
  - `rollback_id: <reverse intervention id>` — normal case;
  - `rollback: none_needed` — with a code comment explaining
    why (usually: action is a restart / refresh that has no
    asymmetric effect);
  - `rollback: reboot` — for cases where no cheap reverse
    exists; requires human confirmation at apply time.
- Config edits always keep a `.bak` copy with the manifest
  timestamp.
- Systemd drop-ins always get a templated revert unit
  (`systemd-run --unit=revert-<id>`).
- APT / dpkg mutations rely on `/var/log/dpkg.log` and
  `apt-history.log`; record the exact timestamp and package
  list in the evidence bundle.

### S — Structured log

- Every action emits a JSON event conforming to
  `harness.event.v1` (see [`journal` schema]
  (../architecture/overview.md#journal)).
- Human-readable output derives from the event; never the
  other way around.
- Events include `dry_run`, `risk`, `module`,
  `intervention_id`, and an `evidence_ref` that points at
  the bundle on disk (or `null` for non-SOAP events).

### What "not a skill" means

If any of I/D/R/S cannot be satisfied, the action is not an
intervention — it is a probe. Declare `risk: none`, put it in
the `probes:` section of the manifest, and do not mutate.

## 2. Risk bands

| Band | Meaning | Auto-executable? |
|---|---|---|
| `none` | Observational. Read-only probe. | yes |
| `low` | Reversible with no side effects the user would mind. Service restart, cache refresh, trash empty. | yes (by default) |
| `medium` | Reversible, but with a rollback ID that must exist. Config file edit, systemd drop-in, backend switch. | only if `max_auto_risk >= medium` AND not in honeymoon AND not `requires_confirmation` |
| `high` | May require reboot, session loss, or network interruption. Module reload, kernel parameter change. | no (default); requires explicit andon-cord confirmation |
| `critical` | Data-loss-possible. FS conversion, disk wipe, firmware flash with no LVFS rollback. | no (default); in practice, never auto. |

### Default caps

- Globally: `max_auto_risk: low`.
- Per-skill: declared in the manifest's `safety:` block,
  **never higher** than the global cap.
- The Doctor honors the minimum of global, per-skill, and
  per-session caps.

## 3. Honeymoon window

- The 30 days following a fresh `russell bootstrap` are the
  honeymoon window.
- During the window, any intervention with `risk >= high` is
  forced to *propose*, regardless of manifest or config.
- The window is tracked in `profile.json` under
  `bootstrap_completed_at`; tampering with this timestamp is
  out of scope for Russell (the user is the Policy layer).

## 4. Andon cord

The andon cord is the human stop-the-line signal. Two surfaces:

- `russell confirm <evidence_id>` — CLI.
- `confirm_proposal` MCP tool — programmatic.

Either surface:

1. Validates that the `evidence_id` exists and has a pending
   proposal.
2. Validates that no override would violate the current risk
   cap.
3. Records a `harness.event.v1` event with
   `action: "confirmed"`, the confirming actor (CLI user,
   MCP client principal), and a timestamp.
4. Invokes the deferred interventions in order, respecting
   all preconditions.

Confirmations do **not** linger: if a proposal is not confirmed
within 24 hours, it expires and its evidence bundle is marked
`expired`.

## 5. Kill switches

Three layers, from coarsest to finest:

1. **Global disable:** an empty file at
   `~/.config/harness/disable` turns every Russell timer into
   a no-op on its next trigger. The Sentinel continues
   collecting samples (observation is always safe), but tiers
   and the Doctor halt.
2. **Per-module pause:** `russell pause <module> --until
   <rfc3339>` records a cooldown in the journal. The
   dispatcher consults this before running anything.
3. **Per-evidence quarantine:** an evidence bundle marked
   `quarantined` by the user will never be auto-confirmed.

Kill-switch state is surfaced by `russell status` and the
`system_status` MCP tool, so an agent cannot mistakenly
announce that Russell is "running normally" while disabled.

## 6. Preconditions on interventions

Manifests may declare preconditions:

- `on_ac_power` — check `upower` / `/sys/class/power_supply`.
- `no_active_graphical_session` — check `loginctl` for
  active seats.
- `screen_locked` — check via `dbus` to `org.gnome.ScreenSaver`
  (or equivalent on other session managers).
- `baseline_samples_present: >= N` — refuse if fewer than N
  Sentinel samples for the relevant probe exist in the last
  24 hours.
- `journal_healthy` — refuse if the meta-Sentinel reports the
  journal is degraded (see
  [`../archive/proprioception.md`](../archive/proprioception.md)).

A precondition failure is not an error; it is a normal
`deferred` outcome, journaled as such.

## 7. What a review asks

When reviewing a PR that adds or alters a mutating action,
confirm:

- [ ] I — `--verify-idempotent` test exists and passes.
- [ ] D — Dry-run path is exercised by a test that asserts
      **zero** external side effects (use a fake
      subprocess runner).
- [ ] R — Rollback strategy is declared; if `none_needed` or
      `reboot`, the justification is present in a code
      comment or manifest comment.
- [ ] S — Emits a `harness.event.v1` record that
      round-trips through the journal.
- [ ] Risk band is the **lowest** honestly applicable.
- [ ] `requires_confirmation` is set wherever a reasonable
      operator would want a last look.
- [ ] Any new precondition is implemented in
      `russell-core::preconditions` and covered by a test.
- [ ] An ADR cites this change if it introduces a new
      convention (e.g. new risk band, new precondition, new
      kill-switch surface).

## 8. LLM and safety

The LLM is a classifier over manifest IDs, not a command
author. See [ADR-0008](../adr/0008-llm-triage-never-emits-shell.md).

- LLM output feeding into the dispatcher is always validated
  against the loaded manifest's ID set (poka-yoke). An
  unknown ID is rejected without execution and logged as a
  `doctor.llm.rejected_id` event.
- LLM self-reported confidence below 0.6 forces human
  handoff.
- Two consecutive intervention failures force human handoff,
  regardless of risk band.

## 9. Runtime Security Features

These runtime defenses complement the install-time safety
scanner (see [`../adr/0024-skill-registry-workshop-lifecycle.md`](../adr/0024-skill-registry-workshop-lifecycle.md)).

### 9.1 Prompt Sanitization Pipeline

All LLM input/output is sanitized to prevent injection and
exfiltration. See [ADR-0030](../adr/0030-prompt-sanitization-pipeline.md).

**Input sanitization** (operator notes, skill knowledge):
- Redacts `RUSSELL_*` environment variable references
- Strips shell metacharacters (`;|&$()\``)
- Detects prompt injection phrases ("ignore previous", etc.)
- Enforces 4KB max length per input

**Output sanitization** (LLM responses):
- Redacts secret patterns (API keys, tokens, passwords)
- Validates `ACTION: skill/action` syntax against registered skills
- Replaces invalid ACTION directives with warnings
- Strips shell metacharacters from output

**What operators see:**
```
[LLM response was sanitized: redacted RUSHELL_* references]
```

This is normal — Russell is protecting your environment.

### 9.2 Capability Attenuation

Skills only receive declared environment variables. See
[ADR-0031](../adr/0031-capability-attenuation.md).

**Manifest declaration:**
```yaml
safety:
  allowed_env_keys: ["HOME", "LANG", "PATH"]
  needs_network: false
```

**Runtime enforcement:**
- `env_clear()` called before every skill subprocess
- Only `allowed_env_keys` + default allowlist propagated
- Default PATH restricted to `/usr/local/bin:/usr/bin:/bin`
- Skills with `needs_network: false` may be blocked in air-gapped environments

**What operators see:**
- Skills cannot access undeclared env vars (API keys, tokens)
- Safety scanner flags skills with network patterns but `needs_network: false`

### 9.3 Consent Expiry

All intervention approvals expire after **5 minutes (300 seconds)**.

**Enforcement:**
- `PendingAction::is_expired()` checked before dispatch
- Expiry prevents stale approvals executing after context shifts
- Error message: "Approval expired; please re-confirm"

**Configuration:**
```bash
# Strict mode (only /approve accepted)
RUSSELL_CONSENT_MODE=strict

# Conversational mode (default — "ok", "yes", "go ahead" accepted)
RUSSELL_CONSENT_MODE=conversational
```

## 10. Relationship to proprioception

Self-triage (see
[`../archive/proprioception.md`](../archive/proprioception.md))
uses the same IDRS contract. A reflex arc may fire without
waiting for the next cadence, but it still emits
`harness.event.v1`, still respects the kill switches, and
still triggers the autoimmune check to prevent self-triage
from invoking itself in a loop.

## 10. Not-yet-automated safety

Things on the roadmap that are deliberately **not** automated
yet and will require a dedicated ADR before they are:

- Firmware updates via `fwupdmgr update`.
- Kernel boot parameter edits (`/etc/default/grub`).
- Partition-layer operations.
- Any skill that touches `/etc` outside of narrowly-scoped
  systemd drop-ins covered by a revert unit.

Until those ADRs land, these remain **proposal-only**.
