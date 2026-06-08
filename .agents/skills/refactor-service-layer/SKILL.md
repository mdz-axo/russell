---
name: refactor-service-layer
visibility: public
description: >
  Extract a shared service layer from duplicated surface logic (CLI, API, ACP servers)
  using the strangler fig pattern, deep-module discipline, and vertical tracer-bullet TDD.
  Activate when the user says "refactor service layer", "extract shared logic",
  "deduplicate surfaces", or when architectural analysis reveals business logic
  embedded in presentation layers.
---

# Refactor Service Layer

Extract duplicated business logic from surface layers into a deep service crate,
using Martin Fowler's strangler fig pattern for safe incremental migration,
Ousterhout's deep-module discipline for interface design, Pocock's deletion test
for validating each extraction, and Karpathy's surgical-change principle for
controlling blast radius per commit.

Composes skills: `improve-codebase-architecture` (explore & deepen),
`tdd` (vertical tracer bullets), `coding-guidelines` (surgical changes),
`constraint-forces` (classify decisions), `zoom-out` (module map),
`pragmatic-semantics` (epistemic discipline).

## When to Activate

- User says "refactor service layer", "extract shared logic", "deduplicate surfaces"
- Architectural analysis reveals business logic in `russell-cli/commands/` that
  duplicates logic in `russell-api-server/routes/` or ACP server
- A domain operation exists in two or more surfaces with divergent return types,
  error handling, or state construction

## Do NOT Activate For

- Adding new features (use `tdd` skill instead)
- One-off bug fixes (use `diagnose` skill instead)
- Pure presentation changes (terminal formatting, HTTP serialization)
- Refactoring that doesn't extract business logic to a shared crate

## Source Methodologies

This skill synthesizes four proven approaches:

### 1. Strangler Fig (Fowler, 2004)

Introduce the new service crate **alongside** the existing code. Migrate one domain
at a time. Both surfaces delegate to the service layer **before** any deletion.
Never rewrite both surfaces simultaneously.

The old surface code is the "tree." The service layer is the "fig" that gradually
wraps and replaces it. At every intermediate step, the system is fully functional.

### 2. Deep Modules (Ousterhout, *A Philosophy of Software Design*)

Every service module must be **deep**: small interface, much behavior behind it.
The cost of a module is its interface — every public type, trait, and function.
The benefit is the behavior it encapsulates. Deep = high benefit/cost ratio.

Apply the **deletion test** before extracting:
- Delete the surface code. If complexity reappears across N callers → extract it.
- Delete the service module. If complexity vanishes → it was a pass-through → don't
  create it; deepen or merge instead.

### 3. Surgical Changes (Karpathy)

Each commit touches exactly one domain extraction. No "while we're in the area"
refactors. No style changes. No renaming variables in adjacent code. No adding
doc comments to code you didn't change. Match existing style.

Every changed line traces directly to the extraction. If you can't explain why
a line changed in terms of the domain being extracted, it doesn't belong.

### 4. Vertical Tracer-Bullet TDD (Pocock)

One domain at a time. RED → GREEN → REFACTOR per behavior. Never write all tests
then all implementation (horizontal slicing). Each tracer bullet goes from test
through service to both surfaces.

Every test carries a `// REQ:` tag referencing a specification requirement.
Tests verify domain behavior through the service seam — not surface presentation.

## Core Principles

### P1 — Strangler Fig Sequence

Migration sequence per domain:
1. Create service operation with domain types
2. Wire CLI to call service → verify CLI still works
3. Wire API to call service → verify API still works
4. Delete duplicated logic from both surfaces
5. Verify full workspace: `cargo check --workspace && cargo test --workspace`

The system must be fully functional after every step. No big-bang rewrites.

### P2 — Depth Test

Before creating a service module, apply the deletion test:
- **Delete the surface code.** Complexity reappears in N callers? → Extract.
- **Delete the proposed module.** Complexity vanishes? → Don't create it.

### P3 — Dependency Direction

```
russell-cli  ──→ russell-services ──→ russell-core
russell-api  ──→ russell-services ──→ russell-skills
                                 ──→ russell-meta
                                 ──→ russell-session
```

Domain crates NEVER depend on `russell-services`. Neither `russell-cli` nor
`russell-api` directly depend on domain crates for business operations. Circular
dependency = wrong extraction boundary — stop and redesign.

### P4 — Constraint Enforcement

Use `constraint-forces` to classify every design decision:

| Decision | Force | Rationale |
|----------|-------|-----------|
| OCAP gates stay in domain crates | Prohibition (P1) | Operator Sovereignty is inviolable |
| Service layer is headless | Prohibition (P1.6) | No visual UI ever |
| Sentinel thresholds are Guardrails | Guardrail | Measured boundary, user-overridable |
| ServiceContext owns dependency graph | Guideline | Best practice, relax with reason |

When two constraints conflict, state the conflict and resolution explicitly.
Never silently relax a Prohibition or Guardrail.

### P5 — One Domain Per Commit

Each commit touches exactly one domain extraction. No cross-domain refactors.
No style changes in adjacent code. Every changed line traces to the extraction.

### P6 — Think Before Coding

Before implementing any extraction:
1. State assumptions about domain boundaries explicitly
2. If multiple interpretations exist, present all of them
3. If a simpler extraction exists, say so
4. If something is unclear, stop and ask — don't guess

## Process

### Phase 0 — Zoom Out

Use the `zoom-out` skill before any extraction. Produce:
1. **Module map** — every module involved, what each owns
2. **Caller graph** — who calls what through which seams
3. **Data flow** — how key data flows through the system
4. **Boundary summary** — where current code sits relative to module boundaries
5. **Key invariants** — constraints that aren't obvious from code

### Phase 1 — Audit and Classify

Use `improve-codebase-architecture` to walk the codebase. For each domain operation
that exists in more than one surface, classify:

| Classification | Meaning | Action |
|----------------|---------|--------|
| **Identical** | Same logic, different framing | Extract to service, thin adapters |
| **Divergent** | Different logic for same intent | Unify in service, parameterize variation |
| **Surface-only** | No counterpart in other surface | Evaluate — may belong in surface, not service |
| **Pass-through** | Surface just delegates to domain crate | Don't extract; surface stays as-is |

### Phase 2 — Classify Constraint Forces

Use `constraint-forces` for every design decision. Document in the plan.

### Phase 3 — Design the Service Crate

Define the service crate with modules per bounded context. Apply depth test to each:
- No more than 7 public functions per module (interface cost)
- Each function takes `&ServiceContext` + domain input, returns `Result<DomainType, ServiceError>`
- No surface types in signatures (no Axum `Json<>`, no CLI `println!` formatting)
- Configuration that varies per surface goes in `ServiceConfig`, not function signatures

### Phase 4 — Migrate One Domain (Strangler Fig Proof of Concept)

Select the smallest self-contained domain. Full cycle:

**4a — RED**: Write one failing test per service operation. Each carries `// REQ:`.
**4b — GREEN**: Implement the service operation with minimal code.
**4c — Wire CLI**: Change CLI to call service. Delete duplicate logic. `cargo test -p russell-cli`.
**4d — Wire API**: Change API to call same service. Delete duplicate logic. `cargo test -p russell-api-server`.
**4e — Verify**: `cargo check --workspace && cargo test --workspace`.
**4f — Delete**: Remove duplicated business logic from both surfaces.

### Phase 5 — Migrate Remaining Domains

Same cycle for each domain, in dependency order. One domain per commit.

### Phase 6 — Verify Surgical Completeness

```bash
cargo check --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

Verify dependency direction. Apply deletion test to every service module.

### Phase 7 — Document Open Questions

Record in `OPEN_QUESTIONS.md`.

## Anti-Patterns (Immediately Flag These)

1. **Horizontal migration** — migrating all domains before wiring any surface
2. **Shallow service module** — 20 public functions that just delegate
3. **Surface types in service signatures** — `Json<T>`, `println!`, HTTP status codes
4. **Big bang deletion** — deleting duplication before verifying both surfaces work
5. **Feature creep** — adding new functionality during migration
6. **Missing `// REQ:` tags** — tests without spec anchoring

## Checklist Per Domain Migration

```
[ ] RED: Service operation test written with // REQ: tag
[ ] GREEN: Minimal implementation passes test
[ ] CLI wired: calls service, formats terminal output
[ ] API wired: calls service, serializes JSON
[ ] Both surfaces verified
[ ] Duplicated logic deleted from both surfaces
[ ] Workspace verified: cargo check --workspace && cargo test --workspace
[ ] Deletion test passed: service module is deep, not shallow
[ ] Dependency direction verified: no circular deps
[ ] clippy clean
```