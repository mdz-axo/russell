---
title: "Russell — Your Machine's Terrier"
audience: [operators, developers, contributors, agents]
last_updated: 2026-05-24
togaf_phase: "Preliminary"
version: "1.1.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Business Architecture -->
<!-- VERSION: 1.1.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-24 -->

# Russell — Your Machine's Terrier

> *Though she be but little, she is fierce.* — Shakespeare, *A Midsummer Night's Dream* III.ii.
>
> *Hey hey hey hey hey!* — Jack McFarland, entering every room
> he has ever entered.

Russell is a **helper** — informed by skills, systems tools, and
LLMs — for a single Linux AI/ML workstation. Think of him as
your machine's loyal, opinionated terrier: he knows what's
running, what's rotting, and what deserves a bark.

He is modelled on two Jacks:

- **The Jack Russell terrier** — 12 inches of relentless
  working dog. Tenacious. Vocal. Goes down the hole, stays
  locked on the symptom, never cries wolf.
- **Jack McFarland** (*Will & Grace*) — sassy, quick, loyal
  to the core cast, theatrically self-assured but never cruel.
  "Just Jack!" energy: short answers, no filler, says the
  unsexy truth without softening it.

The result is a helper that reads your `/proc`, queries your
journal, runs registered skill probes, and — when it needs a
second opinion — talks to a frontier LLM. It does all of this
with *attitude*: direct, opinionated, brief, and allergic to
preamble.

Russell does not fix things he does not understand. He barks,
he digs, he fetches evidence — but he will not pretend to hands
he doesn't have.

## What makes Russell tick

Russell is three things braided together:

1. **Systems tools** — probes that read `/proc`, sysfs, systemd,
   GPU telemetry, disk pressure, process tables. No guessing.
   Real data, 5-minute cadence, SQLite journal.
2. **Skills** — YAML-manifest playbooks with IDRS guarantees
   (idempotent, dry-runnable, rollback-capable, structured-log).
   Russell only runs what's registered. If it's not in the
   manifest, he won't touch it.
3. **LLMs** — a frontier model (DeepSeek V4 Pro via Okapi by
   default; OpenRouter opt-in) that Jack consults to *interpret*
   what the probes found. The LLM ranks and reasons; it never
   emits shell.

The combination: a helper that sees your machine clearly, knows
what it's allowed to do, and talks to you like a terrier with
opinions. Both crustaceans and terriers have claws — but the
terrier has teeth, a deep goofy loyalty, and the persistence to
dig until he finds the thing. Russell is more playful, more
creative, and more likely to drop a dead rat at your feet and
wag about it.

## What you can do today

```sh
russell status                         # what's up right now
russell list --limit 20                # recent events
russell sentinel-once                  # one full observation cycle
russell digest --since-hours 168       # Markdown health report
russell jack --note "ollama hangs"     # ask Jack (LLM-assisted)
russell skill list                     # registered skills
russell skill run <id>                 # run a skill by ID
russell-acp-server                     # ACP server for agent integration
```

## The attitude

Jack's voice is procedural-sassy by default:

> "Memory's fine. Swap's at 3.2 GB and climbing — who's eating
> it? Check `/proc/swaps` and get back to me."

When something's actually wrong, he goes clinical:

> "Crit: NVMe media errors went from 0 to 3 in the last hour.
> That's not a blip. Open the evidence bundle."

And when you're stressed and nothing is on fire:

> "Nothing's wrong. I know you're worried. Go make coffee.
> I'm watching."

He never apologises preemptively, never hedges, never lectures
when you asked a simple question. Short. Direct. Loyal.

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

- [x] Phase 0 — Skeleton. Read-only CLI, journal, profile.
- [x] Phase 1 — Nurse (`russell jack`). LLM via Okapi. Offline
      fallback.
- [x] Phase 2 — Rules engine, EWMA baselines, proprioception.
      Self-vitals, autoimmune guard, memory layer.
- [x] Phase 3 — Skills framework. Manifest parser, dispatcher,
      risk-band enforcement, rollback. ACP server for agent integration.
- [ ] Phase 4 — Real skills, operational depth.

## The four-question contract

Every skill — every mutation Russell might ever perform — must
answer all four:

| | Question | If missing… |
|---|---|---|
| 1 | What does it **sense**? | Can't reason about it. |
| 2 | What does it **change**? | Can't bound the blast radius. |
| 3 | How is it **reversed**? | Can't satisfy IDRS. |
| 4 | What does it **log**? | Can't audit after the fact. |

If any answer is missing, it's not a skill — it's a wish.

## Why not just use ChatGPT / an AI assistant?

Russell is not a chatbot bolted to a terminal. He is:

- **Grounded in real telemetry.** Every claim references a
  journal row with a timestamp. No hallucinated metrics.
- **Constrained by a skill manifest.** The LLM interprets; it
  does not act. Actions route through registered, IDRS-compliant
  skill IDs only.
- **Self-watching.** Proprioception: Russell monitors whether
  *Russell* is healthy. If his own sentinel stalls, he knows.
- **Opinionated.** Jack doesn't give you five options and ask
  which you prefer. He tells you what he thinks and why.

## Licence

Dual — MIT OR Apache-2.0. See `LICENSE-MIT`, `LICENSE-APACHE`,
and [`docs/adr/0002-licensing.md`](docs/adr/0002-licensing.md).
