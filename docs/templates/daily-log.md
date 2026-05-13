---
title: "Daily Log Template"
audience: [operators, developers, agents]
last_updated: 2026-05-09
togaf_phase: "C"
version: "1.0.0"
status: "Active"
---

# Daily Log Template

<!-- TOGAF_DOMAIN: Data Architecture -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-09 -->

Copy this template when creating a new daily log at
`~/.local/state/harness/memory/daily/YYYY-MM-DD.md`.

## Purpose

Daily logs are **derived exports** from Russell's SQLite journal.
They provide a human-readable narrative of each day's activity.
They are rebuildable — if deleted, `russell digest --format markdown`
regenerates them from the journal.

## Template

```markdown
# Russell Daily Log — YYYY-MM-DD

## Summary
- Sessions: N help calls
- Alerts: N | Warnings: N | Info: N
- Self-health: [healthy | degraded — reason]

## Session Notes
<!-- One line per `russell jack` session today -->
- [session-id] — one-line summary of what was discussed

## Retain
<!-- 2–5 durable observations. Tagged: W=world fact, B=biographical, O=opinion -->
<!-- Entries here survive journal compaction. Write them as if someone will -->
<!-- read them six months from now with no other context. -->

- [W] Fact about the host — what changed, what stayed the same.
- [B] Operator preference noted during a session.
- [O](c=0.85) Russell's inference — what pattern seems durable.
```

## Tag Reference

| Prefix | Meaning | Example |
|---|---|---|
| `[W]` | **World fact.** Observable host state. | `[W] NVMe `nvme0n1` SMART media errors went from 0 to 3 on 2026-05-09.` |
| `[B]` | **Biographical.** Operator preference or communication style. | `[B] Operator prefers `crit` alerts via terminal, not email.` |
| `[O](c=N)` | **Opinion.** Russell's inference with confidence 0.0–1.0. | `[O](c=0.85) Swap growth pattern matches ollama memory leak — 3 corroborating sessions.` |

## Confidence Score Guide

The `c=N` on `[O]` entries is Russell's self-assessed confidence:

| Score | Meaning |
|---|---|
| 0.9–1.0 | Corroborated across 5+ sessions or multiple probe cycles. High signal. |
| 0.7–0.9 | Corroborated across 2–4 sessions. Pattern is emerging. |
| 0.5–0.7 | Single observation or weak signal. Worth tracking but not actionable. |
| < 0.5 | Do not write to Retain. Below the durability threshold. |

## Rules

1. **2–5 entries only.** If you have more, pick the most durable.
2. **Self-contained.** Each entry must make sense read alone, six months later.
3. **No shell commands.** Describe what to look at, not what to run. (JR-3.)
4. **Evidence-linked.** Where possible, cite a session ID or probe timestamp.
5. **Rebuildable.** If this file is deleted, `russell digest` regenerates it from the journal. Nothing here is canonical — the journal is.

## See Also

- [ADR-0022](../../adr/0022-markdown-memory-layer.md) — design decision
- [`PERSISTENCE_CATALOG.md`](../../specifications/PERSISTENCE_CATALOG.md) §2.9 — persistence registration
- [`review-entry.md`](review-entry.md) — REVIEW.md entry template
