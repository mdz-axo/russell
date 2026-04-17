<!--
audience: Russell contributors authoring commits and PRs
last-reviewed: 2026-04-17
-->

# Commit and branch conventions

Russell follows [Conventional Commits](https://www.conventionalcommits.org/)
with a small set of project-specific types and scopes. The goal is
machine-readable history so that `russell changelog` can render a
weekly digest of what shipped without human curation.

## 1. Commit message format

```
<type>(<scope>): <imperative subject, no period>

<optional body wrapped at ~72 cols>

<optional footer>
```

- **Imperative mood:** "add gpu-doctor skeleton", not "added".
- **No trailing period** on the subject line.
- Body paragraphs separated by blank lines; bullet lists with
  `- `.
- Footer lines each on their own line. Examples:
  - `Refs: ADR-0007`
  - `Fixes: #42`
  - `BREAKING CHANGE: removes the `snapshot_probe` MCP tool`

## 2. Types

| Type | Use when |
|---|---|
| `feat` | New user-visible capability (CLI subcommand, MCP tool, tier module). |
| `fix` | Bug fix. Must name the behaviour before and after. |
| `docs` | Documentation only. |
| `refactor` | Code change with no behaviour change and no new API surface. |
| `test` | Tests only. |
| `chore` | Build system, dev tooling, CI, dep bumps. |
| `adr` | Adds, amends, or supersedes an ADR. |
| `skill` | Adds or updates a skill manifest / scripts. |
| `proprio` | Proprioception / self-health work (meta-Sentinel, self-triage, reflex arcs). |

Anything else — style tweaks, reverts — uses one of the above; do
not invent a new type.

## 3. Scopes

| Scope | Covers |
|---|---|
| `core` | `russell-core` crate. |
| `mcp` | `russell-mcp` crate or the MCP surface document. |
| `skills` | `russell-skills` crate or the `skills/` tree. |
| `doctor` | `russell-doctor` crate. |
| `sentinel` | `russell-sentinel` crate. |
| `proprio` | `russell-proprio` crate and proprioception docs. |
| `journal` | Journal schema, migrations, event format. |
| `profile` | Profile bootstrap / `profile.json`. |
| `cli` | `russell-cli` crate. |

Cross-cutting scopes exist when the change genuinely cuts across
several crates:

| Scope | Covers |
|---|---|
| `docs` | Cross-cutting documentation changes. |
| `build` | Cargo workspace configuration, CI. |
| `deps` | Dependency updates. |

If a commit touches two scopes, split the commit. If that is
impractical, pick the dominant scope.

## 4. Subject conventions

- Lowercase, except acronyms and proper nouns.
- Say what the commit **does**, not what it touches:
  - ✅ `feat(mcp): add confirm_proposal tool`
  - ❌ `feat(mcp): changes to server.rs`
- Reference an ADR when the commit executes a locked decision:
  `feat(journal): migrate to SQLite WAL (Refs: ADR-0004)`.

## 5. Body

A body is mandatory when the subject alone cannot answer:

- **Why** — what problem does this solve?
- **What changed** — the shape of the change, not the diff.
- **How tested** — which tests or manual checks gate the claim.

For trivial commits (`chore(deps): bump serde to 1.0.200`) a
body is optional.

## 6. Footers

- `Refs: ADR-NNNN[, ADR-MMMM]` — cite locked decisions this
  commit implements.
- `Fixes: #N` — closes issue #N on merge.
- `Co-authored-by:` — for pair / sub-agent work, one per line.
- `BREAKING CHANGE: ...` — any change that alters the MCP wire
  format, the journal schema, a CLI flag's semantics, or the
  skill manifest schema.

## 7. Branch naming

- `feat/<scope>-<slug>` — new capability.
- `fix/<scope>-<slug>` — bug fix.
- `adr/NNNN-<slug>` — ADR-only branch.
- `skill/<skill-id>` — skill addition / edit.
- `proprio/<slug>` — self-health work.
- `docs/<slug>` — doc-only work.

Keep branches short-lived. Rebase on `main`; do not merge `main`
back into the feature branch.

## 8. PR hygiene

- PR title mirrors the first commit's Conventional Commit
  subject.
- PR description includes:
  - A one-paragraph summary.
  - A **Testing** section listing the commands you ran.
  - The ADR number(s) the change implements, if any.
  - A **Rollback** paragraph for any change that alters runtime
    behaviour (matches the R in IDRS — rollback thinking
    applies to the development process too).

## 9. When in doubt

The commit message is the message your future self reads when
`git bisect` lands on this commit at 02:00. Write it for them.
