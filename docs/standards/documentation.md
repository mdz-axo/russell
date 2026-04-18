---
title: "Documentation Voice & Register (Legacy)"
audience: [contributors]
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
audience: anyone touching Markdown, Mermaid, or rustdoc in Russell
last-reviewed: 2026-04-17
-->

# Documentation standard

Russell's documentation is part of the product. Agents read these
files to understand what they are allowed to do. Keep that audience in
mind.

## 1. Doc tiers

| Tier | Location | Purpose | Style |
|---|---|---|---|
| **Orientation** | `README.md`, `AGENTS.md`, `CONTRIBUTING.md` | First-touch docs for humans and agents | Terse, link-heavy, imperative |
| **Design** | `cybernetic-health-harness.md`, `MACHINE_PROFILE.md` | Canonical background; vocabulary source | Discursive, research-grade |
| **Architecture** | `docs/architecture/` | How the locked decisions fit together today | Diagrams + prose; one topic per file |
| **Standards** | `docs/standards/` | Rules that PR reviews enforce | Checklists, numbered clauses |
| **ADRs** | `docs/adr/NNNN-slug.md` | Immutable records of locked decisions | ADR template format only |
| **Templates** | `docs/templates/` | Starting points for new artifacts | Comment-heavy skeletons |
| **rustdoc** | In the source | API reference | Follow Rust API Guidelines |

## 2. Voice and tone

- **Second person for the reader, first person plural for the
  project.** "You must …"; "We chose …".
- **Imperative mood for standards.** "Use `tracing-journald`.",
  not "We prefer `tracing-journald`."
- **No apologies, no hedging in standards.** A rule either exists
  or it does not.
- **Cite sources.** When a decision rests on upstream
  documentation, link the exact page; when it rests on an ADR,
  link the ADR.

## 3. Markdown conventions

- One H1 per file; it matches the file's topic.
- ATX-style headers (`#`), not setext.
- Fence code blocks with the language tag:
  ````
  ```rust
  fn hello() {}
  ```
  ````
- Lines wrap at ~72 columns for prose, looser for tables. Never
  break a URL across lines.
- Relative links inside the repo; absolute links for upstream.
- Every file begins with an HTML comment naming its audience and
  last-reviewed date:
  ```
  <!--
  audience: ...
  last-reviewed: YYYY-MM-DD
  -->
  ```
- The `last-reviewed` stamp is bumped whenever content materially
  changes. Typo fixes do not require a bump.

## 4. Terminology

- Use the medical metaphor vocabulary exactly as defined in
  [`AGENTS.md`](../../AGENTS.md) §3. Do not invent synonyms.
- When introducing a new term, add it to the AGENTS.md table in
  the same PR.
- Abbreviations: expand on first use per file, then use the short
  form. `SOAP`, `IDRS`, `EWMA`, `VSM`, `MCP` are exempt once the
  reader has reached `AGENTS.md` §3.

## 5. Mermaid discipline

- All diagrams are Mermaid (no binary images for architecture).
- Prefer `flowchart TB` for architecture, `sequenceDiagram` for
  protocols, `stateDiagram-v2` for state machines, `gantt` for
  schedules.
- Node names use the medical vocabulary: `Sentinel`, `Doctor`,
  `Skill Registry`.
- Keep diagrams under ~15 nodes. If you need more, split into
  sub-diagrams.
- Always include a one-paragraph prose summary next to every
  diagram; a reader using a terminal renderer may not see it.

## 6. Cross-linking

- From orientation → standards: link to the whole standard, not
  a clause, unless the clause is stable.
- From standards → ADRs: link by ADR number, not slug, so
  renames do not break references (path still matters, but the
  number is the canonical handle).
- From ADRs → cybernetic-health-harness.md: cite the section
  number (§X.Y).

## 7. Tables

- Use Markdown tables only when comparing ≥3 attributes across
  ≥3 rows. Otherwise a bulleted list is clearer.
- Left-align text; right-align numerics; decimal-align is not
  available in plain Markdown (don't fake it).
- Header row gets the pipe pattern `|---|`, not `|:---:|`, unless
  alignment is semantically meaningful.

## 8. Diagrams and file paths

- Any path mentioned in running prose is wrapped in backticks:
  `~/.local/state/harness/journal.db`.
- Paths that exist in the repo are hyperlinks on first mention
  per section.

## 9. Commit message for doc-only changes

- Type: `docs`. Scope: the doc area you touched (`docs`,
  `readme`, `agents`, etc.). See
  [`commits.md`](commits.md).

## 10. Review checklist for documentation PRs

- [ ] `last-reviewed` stamp bumped on every materially-changed
      file.
- [ ] New terminology added to `AGENTS.md` §3 if introduced.
- [ ] All internal links resolve.
- [ ] Mermaid renders without error (paste into
      https://mermaid.live if unsure).
- [ ] Code fences have language tags.
- [ ] No long-form advocacy in standards; move it to an ADR.
