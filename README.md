# Russell — Cybernetic Health Harness

> *Though she be but little, she is fierce.* — Shakespeare, *A Midsummer Night's Dream* III.ii.
>
> Named for the disposition of the Jack Russell terrier and for
> *Will & Grace*'s "Just Jack" — small but mighty, quick but
> loyal, alert but never cruel.

Russell is a **single-host** cybernetic health harness for a
Linux AI/ML workstation. He observes one machine, remembers what
he saw, reports it honestly, watches himself, and — when asked —
consults a frontier LLM with zero data retention to help the
operator think about what he is seeing.

He does not fix things he does not understand.

## What Russell is today

Phase 0 (read-only skeleton) is complete. Phase 1 (the MVP
Doctor — `russell jack`) is the current target.

**Six verbs.** All read-only. See
[`docs/specifications/MVP_SPEC.md`](docs/specifications/MVP_SPEC.md) §2.

```sh
russell status                         # summary
russell list --limit 20                # recent events
russell profile --init                 # one-shot stub
russell digest --since-hours 168       # Markdown report
russell sentinel-once                  # one observation cycle
russell jack --note "ollama hangs"     # ask Jack    [Phase 1]
```

## Reading order (do not skip)

1. [`AGENTS.md`](AGENTS.md) — the binding rules.
2. [`docs/README.md`](docs/README.md) — portal + critical set.
3. [`docs/architecture/PRINCIPLES_CATALOG.md`](docs/architecture/PRINCIPLES_CATALOG.md)
   — JR-1 through JR-7.
4. [`docs/specifications/MVP_SPEC.md`](docs/specifications/MVP_SPEC.md)
   — the pinned boundary.
5. [`docs/architecture/THE_JACK.md`](docs/architecture/THE_JACK.md)
   — who Jack is.
6. [`MACHINE_PROFILE.md`](MACHINE_PROFILE.md) — the patient.
7. [`cybernetic-health-harness.md`](cybernetic-health-harness.md)
   — the full design, the aspirational target.

## Status

See [`docs/status/CONSOLIDATED-STATUS.md`](docs/status/CONSOLIDATED-STATUS.md).

- [x] Phase 0 — Skeleton. Read-only CLI verbs. Journal. Profile.
- [x] Phase 1 — Doctor (`russell jack`). Kimi K2 via OpenRouter
      with `zdr: true`. Offline fallback.
- [x] Phase 1b — systemd unit files.
- [x] Phase 1c — 20-day soak (closed per ADR-0018).
- [ ] **Phase 2 — Rules engine, EWMA baselines, Tier I modules.
      Self-vital active. Kask integration live.**
- [ ] Phase 3+ — Skills, dispatch, MCP surface, full proprio.
      See `cybernetic-health-harness.md` §20.

## The four-question contract

Every proposed automation answers:

- What does it **sense**?
- What does it **change**?
- How is it **reversed**?
- What does it **log**?

If any of those is missing, it is not ready.

## Licence

Dual — MIT OR Apache-2.0. See `LICENSE-MIT`, `LICENSE-APACHE`,
and [`docs/adr/0002-licensing.md`](docs/adr/0002-licensing.md).
