---
title: "ADR Authoring Standard"
audience: [contributors, architects]
last_updated: 2026-04-18
togaf_phase: "Preliminary"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Preliminary -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-04-18 -->

<!--
audience: anyone filing a Russell ADR
last-reviewed: 2026-04-17
-->

# ADR standard

Russell uses Architecture Decision Records to freeze decisions that
would otherwise be re-litigated every time a new contributor joins.

## 1. When to file an ADR

File an ADR when you are:

- choosing between two or more options where the choice is not
  obvious in a month;
- introducing a new wire format, storage format, or binary
  contract;
- raising or lowering the risk ceiling for any class of action;
- changing the MCP surface (adding / removing / renaming a tool,
  changing a tool's risk band);
- adopting a dependency that constrains future choices (e.g.
  runtime, DB engine, IPC);
- overriding a previous ADR (supersede, don't rewrite).

If you are **not sure** whether you need one, the answer is almost
always yes. Short ADRs are fine; it is the record that matters.

## 2. Numbering

- Four-digit, zero-padded, monotonic: `0001`, `0002`, ...
  `0015`.
- Allocate your number by picking the next one higher than any
  merged ADR in `docs/adr/`.
- Never renumber a merged ADR. If two PRs race for the same
  number, the second renumbers.

## 3. Filenames

`docs/adr/NNNN-<short-slug>.md`

- `NNNN` is the number.
- `<short-slug>` is kebab-case, 2–6 words, imperative feel
  (`adopt-tokio-runtime`, `yaml-manifest-subprocess-skill-model`).

## 4. Format

Use [`docs/templates/adr-template.md`](../templates/adr-template.md).
Required sections:

- **Status** — one of Proposed / Accepted / Superseded / Deprecated.
- **Context** — what forced the decision.
- **Decision** — imperative, no hedging.
- **Consequences** — positive, negative, neutral.
- **Alternatives considered** — each with a one-paragraph
  rejection reason.

Optional sections: Implementation notes, References.

## 5. Status lifecycle

```
Proposed ──(merge)──> Accepted ──(later ADR)──> Superseded by ADR-MMMM
                                  │
                                  └─(rare)─> Deprecated (decision no longer applies)
```

- **Proposed** ADRs live in the PR that introduces them. They
  may be merged as Proposed if discussion is ongoing, but
  Accepted is the common case.
- An ADR moves to **Superseded** only by a new ADR that
  explicitly cites it in its own Supersedes field. Never edit
  the body of a superseded ADR; the replacement carries the
  current decision.
- **Deprecated** is for decisions whose subject has been
  removed entirely from Russell.

## 6. Supersession

When you supersede an ADR:

1. Give the new ADR the next free number.
2. In the new ADR's Status line, add `Supersedes ADR-XXXX`.
3. Edit the superseded ADR's Status line to
   `Superseded by ADR-YYYY`. This is the **only** permitted
   post-merge edit.
4. Do not delete content from the superseded ADR; future
   readers need to understand the history.

## 7. Cross-referencing

- ADRs cite the canonical design document by section number:
  "[`cybernetic-health-harness.md` §12.1](../../cybernetic-health-harness.md)".
- ADRs cite other ADRs by number and slug on first mention, by
  number thereafter.
- Architecture documents under `docs/architecture/` cite ADRs
  by number; they do not restate decisions.

## 8. Review

A Proposed ADR that is being actively discussed should include a
`## Open questions` section; that section is removed before the
ADR is Accepted.

## 9. Minimum ADRs that must exist before any code lands

Per [`AGENTS.md`](../../AGENTS.md), the following decisions must
be recorded before the corresponding code is written:

- Scope and charter (ADR-0001)
- Licensing (ADR-0002)
- MCP transport (ADR-0003)
- Persistence (ADR-0004)
- Privileged operations (ADR-0005)
- Profile abstraction (ADR-0006)
- Skill model (ADR-0007)
- LLM triage constraints (ADR-0008)
- Async runtime (ADR-0009)
- Observability stack (ADR-0010)
- Testing strategy (ADR-0011)
- Config formats (ADR-0012)
- Workspace layout (ADR-0013)
- Licensing of skill manifests (ADR-0014)
- Proprioception (ADR-0015)
