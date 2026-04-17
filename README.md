# Russell — Cybernetic Health Harness

> Named for the disposition, not the philosopher: a steady, watchful companion
> that tells you when something feels off and knows who to call when it does.

This folder is a **design project**, not yet an implementation. It re-casts the
loose collection of periodic shell scripts under `~/Clones/scripts/` as a
closed-loop, self-adaptive maintenance harness for a Framework 16 / AMD RDNA3 /
Ubuntu AI-dev workstation.

## Reading order

1. [`MACHINE_PROFILE.md`](MACHINE_PROFILE.md:1) — the patient's chart: what was
   actually observed on this machine, today.
2. [`cybernetic-health-harness.md`](cybernetic-health-harness.md:1) — **the
   design document.** Executive summary, metaphor, research findings, legacy
   recon + migration table, knowledge graphs (v1 and v2), architecture,
   scheduler, telemetry, four tier specifications, specialist Doctor,
   self-profiling bootstrap, logging/observability, safety contract, code
   blueprints, reflection against exemplars, philosophical grounding, and the
   implementation roadmap.

## One-line summary

> A `systemd`-driven core clock runs cadenced hygiene tasks; a continuous
> Sentinel watches for symptoms; a Doctor supervisor escalates with
> manifest-driven skill modules; a self-profiling bootstrap personalizes the
> regimen to *this* hardware; a SQLite journal turns every check into
> time-series evidence — all observable, idempotent, reversible, and refusing
> to "fix" things it doesn't understand.

## Status

- [x] Observed machine & legacy scripts recon
- [x] Knowledge graph v1 + v2
- [x] Plan drafted and revised against exemplars
- [ ] Implementation (Phases 0 → 6 — see §20 of the design document)

## Conventions

- Nothing here is executable yet. Code in the design document is illustrative
  blueprint, not production.
- Machine facts live in `MACHINE_PROFILE.md`. The design document references a
  *target class* of hardware; the bootstrap reconciles the real observed
  silicon at install time.
- Every proposed automation must answer: what does it sense, what does it
  change, how is it reversed, what does it log? If any of those four is
  missing, it is not ready.
