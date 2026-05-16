---
title: "ADR-0022: Markdown Memory Layer — Derived Exports for Human Legibility"
audience: [developers, architects]
last_updated: 2026-05-09
togaf_phase: "C"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Data Architecture -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-09 -->

# ADR-0022: Markdown Memory Layer — Derived Exports for Human Legibility

- **Status:** Accepted
- **Date:** 2026-05-09
- **Deciders:** Project operator
- **Tags:** `memory`, `markdown`, `jr-5`, `jr-7`, `doctor`

## Context

Russell's SQLite journal is the canonical store for all structured
state per C-2 and JR-7. It captures telemetry (samples, events,
help_sessions) with machine-queryable precision, but it has a
human-readability gap:

1. **No cross-session narrative.** Each `russell jack` session
   produces an isolated evidence bundle. The journal has all the
   data, but reconstructing "what happened on this host last
   Tuesday" requires SQL queries.
2. **No durable memory for the Doctor.** Jack starts every session
   from zero context beyond the last 24h of journal data. He
   cannot accumulate knowledge about the operator's preferences
   or the machine's personality across sessions.
3. **No operator-legible review surface.** The `help_sessions`
   table and evidence bundles are functional but require tooling
   to read. An operator who wants to quickly understand "what
   does Russell know?" has no easy answer.

OpenClaw's Markdown memory architecture demonstrates that a
companion human-readable layer makes an agent feel like a real
assistant rather than a black-box diagnostic tool. The question
is how to adapt this to Russell without violating JR-7 and C-2.

### Constraints

- **C-2:** Persistence is SQLite. One database, one writer, one
  place to look.
- **JR-7:** Every byte Russell writes is registered in
  `PERSISTENCE_CATALOG.md`. No hidden caches.
- **JR-1:** Austere by default. Every new file must earn its
  place.
- **ADR-0020:** The Doctor already skips LLM calls below threshold;
  adding context persistence makes threshold-skip sessions more
  useful (they contribute to durable memory without consuming
  API credits).

## Decision

Add a **Markdown memory layer** as **derived exports** from the
journal. The journal remains the sole canonical store. Markdown
files are rebuildable from the journal at any time and can be
safely deleted.

### Memory files (Russell-owned, under `~/.local/state/harness/memory/`)

| File | Purpose | Writer |
|---|---|---|
| `daily/YYYY-MM-DD.md` | Daily log with `## Retain` section | `russell digest` or future `reflect` verb |
| `REVIEW.md` | Russell's self-assessment review surface | Future `reflect` verb; human-reviewed |

### Identity files (operator-owned, under `~/.config/harness/`)

| File | Purpose | Russell writes? |
|---|---|---|
| `PERSONA.md` | Runtime persona customisation for Jack | Never |
| `USER.md` | Operator profile (timezone, prefs, urgency bar) | Never |

These are operator-owned config files (like `russell.env`). Russell
reads them at startup if they exist; Russell never writes to them.
This respects the config/state boundary established in the XDG
layout.

### Daily log convention

Each `daily/YYYY-MM-DD.md` follows this structure:

```markdown
# Russell Daily Log — YYYY-MM-DD

## Summary
- Sessions: N help calls
- Alerts: N | Warnings: N | Info: N
- Self-health: [healthy | degraded — reason]

## Session Notes
- [session-id] — one-line summary

## Retain
<!-- 2–5 durable observations. Tagged: W=world fact, B=biographical, O=opinion -->
- [W] Fact about the host...
- [B] Operator preference noted...
- [O](c=0.85) Russell's inference with confidence...
```

### `## Retain` section conventions

Each retain entry is tagged with a type prefix:

| Prefix | Meaning | Example |
|---|---|---|
| `[W]` | **World fact.** Observable host state. | `[W] NVMe `nvme0n1` SMART media errors went from 0 to 3 on 2026-05-09.` |
| `[B]` | **Biographical.** Operator preference or communication style. | `[B] Operator prefers `crit` alerts via terminal, not email.` |
| `[O](c=N)` | **Opinion.** Russell's inference with confidence 0.0–1.0. | `[O](c=0.85) Swap growth pattern matches ollama memory leak — 3 corroborating sessions.` |

The confidence score is a Russell self-assessment, not a
statistical measure. It reflects: number of corroborating
observations × recency × signal clarity.

### REVIEW.md structure

REVIEW.md is the human-in-the-loop gate for promoting observations
from daily logs into durable memory:

```markdown
## [YYYY-MM-DD] Observation Title

**Type:** W | B | O
**Confidence:** 0.0–1.0
**Evidence:** session-ids or probe references
**Status:** pending-review | accepted | rejected

### Observation
(What Russell observed or inferred.)

### Rationale
(Why Russell thinks this is durable.)

### Review Notes
(To be filled by the operator.)
```

### Rebuildability guarantee

All Markdown files in `memory/` can be regenerated from the
journal. The `russell digest --format markdown` verb (or a future
`russell reflect` verb) reads the journal and produces the
Markdown. This means:

- `rm -rf ~/.local/state/harness/memory/` is safe — nothing is
  lost that the journal does not already have.
- If the Markdown format evolves, old logs can be regenerated
  from the same journal.
- The journal + Markdown layer together give operators both
  machine-queryable telemetry AND human-readable narrative.

### Pre-compaction memory flush (deferred)

When Phase 2 journal compaction prunes old rows, a `reflect` pass
SHALL write retained facts to the daily Markdown log before
compaction proceeds. This ensures narrative continuity across
compaction boundaries. The hook point is documented here but
implementation is deferred until the compaction skill lands.

### Doctor integration

The Doctor (`russell jack`) reads `PERSONA.md` and `USER.md`
at session startup if they exist. Their content is appended to
the compiled-in `JACK_PERSONA` system prompt, giving Jack
session-to-session continuity without violating JR-3 (the LLM
still never emits shell; it just knows more about the operator).

After each session, the Doctor appends a one-line session note
to the current day's `daily/YYYY-MM-DD.md` if it exists. If the
file does not exist, no write occurs — the daily log is lazily
created by `russell digest` or `russell reflect`.

## Consequences

### Positive

- Operators can read Russell's memory in any text editor. No
  SQL required.
- Jack gains session-to-session context from `USER.md` and
  `PERSONA.md`, making him feel more like an assistant and less
  like a one-shot tool.
- `REVIEW.md` creates a structured human-in-the-loop gate —
  Russell never silently promotes low-quality observations into
  durable memory. This satisfies JR-5 (proprioception visibility).
- The rebuildability guarantee means Russell's memory survives
  any Markdown format evolution. The journal is always the
  source of truth.

### Negative / accepted costs

- 4 new files in the persistence surface (though 2 are
  operator-owned config, 1 is a directory of derived exports).
- The rebuildability guarantee means the `russell digest` or
  `reflect` verb must be able to produce Markdown from the
  journal. This is additional code (deferred to when the verb
  lands).
- Daily log appends from the Doctor add a small write per
  `russell jack` call (mitigated: append-only, ~200 bytes).

### Neutral

- The `memory/` directory is Russell-owned state but does not
  introduce a new canonical store. `rm -rf memory/` loses
  nothing that can't be rebuilt.
- `PERSONA.md` and `USER.md` are operator-owned config files,
  same tier as `russell.env`. Their absence changes nothing;
  their presence enriches Jack's context.

## Alternatives Considered

### Make Markdown the canonical store for Doctor knowledge

Rejected. Violates C-2 ("Persistence is SQLite") and JR-7
("every byte is registered"). The journal must remain the
single source of truth. Markdown is a derived view.

### Skip Markdown entirely; add a `russell memory export` command later

Rejected. The point is not just export — it's legibility. An
export command still requires the operator to run a tool. The
Markdown files are always there, always readable, always
grep-able. This is the property that makes OpenClaw's agent feel
like a real assistant.

### Store persona/user context in the journal (new table)

Considered. Would keep everything in SQLite per C-2, but loses
the "edit in any text editor" property. Operator-owned identity
files belong in `~/.config/`, not in a SQLite database Russell
owns.

### Use JSON instead of Markdown for daily logs

Rejected. The entire point is human readability. Markdown is
the universal language of developers' notes, commit messages,
and documentation. JSON in a text editor is not legible in the
same way.

## Implementation Notes

1. `Paths::memory_dir()`, `Paths::memory_daily_dir()`,
   `Paths::user_md()`, `Paths::persona_md()` added to
   `russell-core/src/paths.rs`.
2. Memory directories added to `ensure_dirs()`.
3. New entries in `PERSISTENCE_CATALOG.md` §2.9–2.11.
4. Doctor reads `USER.md` and `PERSONA.md` at session startup
   (future implementation — this ADR authorises the paths;
   the read logic lands in a follow-up PR).
5. Doctor appends session note to `daily/YYYY-MM-DD.md` after
   each session (future implementation).
6. `russell digest --format markdown` generates daily logs
   (future implementation).
7. Pre-compaction memory flush hook is documented but deferred
   until Phase 2 compaction lands.

## References

- [C-2](../architecture/PRINCIPLES_CATALOG.md) — Persistence is SQLite
- [JR-5](../architecture/PRINCIPLES_CATALOG.md) — Proprioception: Jack watches Jack
- [JR-7](../architecture/PRINCIPLES_CATALOG.md) — Persistence is auditable
- [`PERSISTENCE_CATALOG.md`](../specifications/PERSISTENCE_CATALOG.md) — full persistence register
- [`THE_JACK.md`](../architecture/THE_JACK.md) — Jack's persona design
- [ADR-0020](0020-threshold-gated-llm-escalation.md) — threshold-gated LLM (related: threshold-skip sessions still contribute to memory)
- OpenClaw architecture (external) — inspiration for Markdown memory pattern
