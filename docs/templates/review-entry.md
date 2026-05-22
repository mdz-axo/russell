---
title: "REVIEW.md Entry Template"
audience: [operators, developers, agents]
last_updated: 2026-05-09
togaf_phase: "C"
version: "1.0.0"
status: "Active"
---

# REVIEW.md Entry Template

<!-- TOGAF_DOMAIN: Data Architecture -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-09 -->

Copy this template when adding a review entry to
`~/.local/state/harness/memory/REVIEW.md`.

## Purpose

REVIEW.md is the **human-in-the-loop gate** for Russell's durable
memory. Observations from daily logs that show a persistent pattern
are promoted here for operator review. Only entries marked
`Status: accepted` graduate to Russell's durable knowledge.

Russell writes the `Observation` and `Rationale` sections. The
operator fills in `Review Notes` and sets the `Status`.

## Template

```markdown
## [YYYY-MM-DD] Observation Title

**Type:** W | B | O
**Confidence:** 0.0–1.0
**Evidence:** session-id-1, session-id-2, probe-cycle-N
**Status:** pending-review | accepted | rejected

### Observation
(What Russell observed or inferred. One paragraph. Self-contained.)

### Rationale
(Why Russell thinks this is durable. Cite the evidence —
corroborating sessions, probe cycles, consistency of the pattern.)

### Review Notes
(To be filled by the operator. Accept, reject, or ask for more evidence.)
```

## Type Reference

| Type | Meaning | Promotion threshold |
|---|---|---|
| `W` | **World fact.** Observable host state. | 2+ corroborating observations over 24h+. |
| `B` | **Biographical.** Operator preference or communication style. | Confirmed explicitly by operator in a session. |
| `O` | **Opinion.** Russell's inference about a pattern. | 3+ corroborating sessions or probe cycles. |

## Status Lifecycle

```
pending-review ──→ accepted  (operator approves)
                ──→ rejected  (operator disagrees or asks for more evidence)
                ──→ (stale)   (no operator action for 30 days; auto-rejected)
```

Accepted entries become part of Russell's durable context and are
included in future SOAP bundles (as a `## Durable Memory` section).

Rejected entries stay in REVIEW.md for audit trail. Russell does
not re-propose the same observation unless new evidence emerges.

## Rules

1. **One observation per entry.** Do not bundle multiple observations.
2. **Evidence must be cited.** Session IDs or probe timestamps.
3. **Confidence is Russell's self-assessment.** It reflects
   corroboration count × recency × signal clarity. It is not a
   statistical measure.
4. **Status defaults to `pending-review`.** Russell never sets
   `accepted` or `rejected` — only the operator does.
5. **Mutate nothing.** REVIEW.md entries never cause Russell to
   take action. They are observations, not interventions. (JR-2.)

## Example

```markdown
## [2026-05-09] Ollama memory leak pattern under sustained ROCm load

**Type:** O
**Confidence:** 0.85
**Evidence:** 01JQ26KXYZ-help-a1b2, 01JQ27KXYZ-help-c3d4, 01JQ28KXYZ-help-e5f6
**Status:** pending-review

### Observation
Swap usage climbs steadily (2.1 → 3.2 → 4.8 GB) over 6-hour
sessions when ollama is running a 70B model under ROCm. Memory
available stays stable at ~90 GiB — the swap growth is not from
memory pressure. When ollama is restarted, swap returns to
baseline within 30 minutes.

### Rationale
Three sessions over three days show the same pattern. The
correlation between ollama uptime and swap growth is consistent
across all three. No other process shows significant swap usage
during these windows. This is likely a CUDA/ROCm memory allocator
pattern rather than a leak — pinned memory that gets swapped but
never freed until process restart.

### Review Notes
(Operator: "Confirmed. Known ollama behaviour with 70B+ models
on ROCm. Not a leak — it's ROCm's caching allocator. Accepted
as a durable fact for future sessions.")
```

## See Also

- [ADR-0022](../adr/0022-markdown-memory-layer.md) — design decision
- [`PERSISTENCE_CATALOG.md`](../specifications/PERSISTENCE_CATALOG.md) §2.9 — persistence registration
- [`daily-log.md`](daily-log.md) — daily log template (where candidate observations originate)
