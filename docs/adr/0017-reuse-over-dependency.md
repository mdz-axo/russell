---
title: "ADR-0017: Reuse over Dependency — Copy-with-Provenance"
audience: [developers, architects, contributors]
last_updated: 2026-04-18
togaf_phase: "H — Change Management"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Change Management -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-04-18 -->

# ADR-0017: Reuse over Dependency — Copy-with-Provenance

- **Status:** Accepted
- **Date:** 2026-04-18
- **Deciders:** Project founders
- **Tags:** `dependencies`, `workspace`, `jr-6`, `sync`

## Context

Russell's LLM client (ADR-0016), the SQLite patterns in
`russell-core::journal`, and eventually a few more subsystems
would benefit from code that already exists in sibling
workspaces: `~/Clones/slate/stack/crates/stack-llm/`,
`~/Clones/peripheral/crates/peripheral-store/`, and others. Two
natural approaches exist:

1. **Depend** on those crates via `path = "../../slate/..."` or
   git dependency.
2. **Copy** the functions we need into Russell with a citation.

The first is what `peripheral` does today (depends on
`stack-llm`). The second is what JR-6 prescribes for Russell.

## Decision

Russell **copies** code from upstream workspaces rather than
depending on them. Every copy:

1. Carries a **file-header comment** naming the upstream source
   path, the upstream commit SHA at copy time, Russell's local
   changes, and the sync policy.
2. Is **registered** in
   [`../operations/REUSE_MANIFEST.md`](../operations/REUSE_MANIFEST.md)
   with a row in §3.
3. Is **granular** — we copy the functions we need, not whole
   crates. If a function we need imports types from a crate-
   local module, we either copy those types too (adding a row
   per file) or rewrite to Russell's own types.

### Rule: no runtime dependency on sibling workspaces

`Cargo.toml` in any Russell crate MUST NOT contain a
`path = "../../<sibling-workspace>/..."` dependency. The only
exception is temporary local development of an upstream change
that the author intends to port upstream — and even then, the
path-dep must be removed before merge.

### Rule: only proven code is copied

We do not speculatively copy "in case we need it later." Copy
when a concrete Russell feature needs the code. Extract the
minimum that works.

### Rule: license compatibility

Every copied file's upstream must be compatible with Russell's
dual MIT / Apache-2.0 (ADR-0002). In practice both `stack-*` and
`peripheral-*` are MIT/Apache; if a future copy target is GPL
we stop and re-open the question under ADR-0002.

### Rule: sync is a deliberate action

The REUSE_MANIFEST has a Sync Log (§6). When upstream fixes a
bug in a function we copied, we:

1. Review the upstream diff.
2. Decide: port, adapt (apply similar fix in Russell-shaped
   way), or freeze (the fix doesn't apply to our usage).
3. Record the decision as a dated row in the Sync Log.
4. If porting: update the upstream-commit field in the copy's
   file header.

No automatic sync. No "pull upstream main." Every sync is a PR.

## Consequences

### Positive

- Russell builds offline, in isolation.
- A breakage in `slate/stack` or `peripheral` does not propagate
  to Russell.
- Code review of a copy is trivial: the file-header tells the
  reviewer where it came from and what was changed.
- Russell's footprint stays small because we copy surgically.
- Upstream improvements are deliberate, not surprise.

### Negative / accepted costs

- When a CVE-class bug is fixed upstream, Russell must port it.
  The window is defined by the REUSE_MANIFEST sync cadence
  (§5: 7 days for CVE-class, 90 days for general review).
- Duplication across sibling workspaces. This is the explicit
  trade-off JR-6 makes.
- If Russell's needs diverge from upstream, "rebasing" a copy
  becomes harder over time. Mitigation: keep copies small and
  functional, not framework-shaped.

### Neutral

- The `REUSE_MANIFEST.md` becomes a load-bearing document.
  That's fine; it's small and is updated only when we copy.

## Alternatives Considered

### A. Depend on `stack-*` via path

Rejected per JR-6 rationale. Peripheral pays this cost; Russell
does not need to.

### B. Depend via git revision pin

Rejected. Still requires network to build; still exposes Russell
to unrelated upstream churn through the transitive dependency
graph.

### C. Fork the upstream crate into Russell's workspace

Rejected. A fork is a crate; the file-header + manifest pattern
is lighter and preserves the "one concrete thing, one row"
property of the REUSE_MANIFEST.

### D. Publish Russell's own stack-llm analogue as a crate

Rejected for MVP. Would require publishing to crates.io or
similar; premature for a single-host tool.

## Implementation Notes

- Every copied file begins with:

  ```rust
  // SPDX-License-Identifier: MIT OR Apache-2.0
  //
  // Copied from: slate/stack/crates/stack-llm/src/wire.rs
  // Upstream commit: <sha>
  // Russell changes: <one-line summary>
  // Sync policy: review on upstream bug fix; pull fixes, not features.
  // See docs/operations/REUSE_MANIFEST.md row <N>.
  ```

- The `<sha>` is the output of
  `git -C ~/Clones/slate log -1 --format=%H`
  at copy time. If upstream is not a git checkout, record
  `"not-versioned (path copy)"` with a dated comment.

- File-header discipline is enforced by review. A missing header
  on a file that is clearly copied upstream is a review blocker.

## References

- [`../architecture/PRINCIPLES_CATALOG.md`](../architecture/PRINCIPLES_CATALOG.md)
  §3 JR-6 — the principle this ADR mechanises.
- [`../operations/REUSE_MANIFEST.md`](../operations/REUSE_MANIFEST.md)
  — the register.
- [ADR-0013](0013-rust-workspace-layout.md) — the workspace
  topology that makes "path dependency outside this workspace"
  a visible line in `Cargo.toml`.
