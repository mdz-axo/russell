---
title: "Russell ADR Template"
audience: [contributors, maintainers]
last_updated: 2026-05-25
version: "1.0.0"
status: "Active"
domain: "Cross-cutting"
ddmvss_categories: [curation, lifecycle]
---

# Russell ADR Template

**Purpose:** Template for Architecture Decision Records.

**Usage:** Copy this template to `decisions/NNNN-<short-title>.md` and fill in sections.

---

## Template

```markdown
---
title: "ADR-NNNN: <Short Title>"
audience: [architects, developers]
last_updated: YYYY-MM-DD
version: "1.0.0"
status: "Proposed | Accepted | Superseded | Deprecated"
domain: "Cross-cutting | specific domain"
ddmvss_categories: [category1, category2, ...]
supersedes: "ADR-XXXX" (if applicable)
superseded_by: "ADR-YYYY" (if applicable)
---

# ADR-NNNN: <Short Title>

## Status

**Proposed | Accepted | Superseded by ADR-YYYY | Deprecated**

## Context

Describe the forces at play, including:
- Technical constraints
- Business requirements
- Existing architecture
- Known risks

## Decision

State the decision clearly and concisely:
- What was decided
- Why this option was chosen
- What alternatives were considered

## Consequences

### Positive
- Benefit 1
- Benefit 2

### Negative
- Drawback 1
- Drawback 2

### Risks
- Risk 1 (mitigation)
- Risk 2 (mitigation)

## Implementation

Describe how to implement this decision:
- Code changes required
- Migration steps
- Testing strategy

## References

- Related ADRs
- External resources
- Research papers

## Appendix

### Alternatives Considered

#### Alternative 1: <Name>

**Description:** ...

**Pros:**
- ...

**Cons:**
- ...

**Why rejected:** ...

#### Alternative 2: <Name>

**Description:** ...

**Pros:**
- ...

**Cons:**
- ...

**Why rejected:** ...
```

---

## Guidelines

### 1. Numbering

- Use sequential numbers: ADR-0001, ADR-0002, ...
- Check existing ADRs to avoid conflicts
- Reserved ranges:
  - 0001-0099: Core architecture
  - 0100-0199: Sentinel subsystem
  - 0200-0299: Journal subsystem
  - 0300-0399: Jack subsystem
  - 0400-0499: Skill subsystem
  - 0500-0599: ACP subsystem
  - 0600-0699: Proprioception subsystem
  - 0700-0799: Cross-cutting concerns

### 2. Naming

- Use kebab-case: `0001-short-title.md`
- Keep title short and descriptive
- Avoid generic names like "architecture-decision"

### 3. Status Lifecycle

```
Proposed → Accepted → Superseded → Deprecated
```

- **Proposed:** Under review, not yet implemented
- **Accepted:** Approved and implemented
- **Superseded:** Replaced by newer ADR
- **Deprecated:** No longer relevant

### 4. Supersession

When ADR-MMMM supersedes ADR-NNNN:

1. Create ADR-MMMM with `supersedes: "ADR-NNNN"`
2. Update ADR-NNNN with `superseded_by: "ADR-MMMM"`
3. Update ADR-NNNN status to "Superseded"
4. Do not delete ADR-NNNN (historical record)

### 5. Deferral

For decisions outside current scope:

1. Create ADR in `decisions/deferred/`
2. Set status to "Proposed"
3. Document why deferred (e.g., "Requires Phase 3")
4. Move to `decisions/` when ready to implement

---

## Examples

### Example 1: Core Architecture

```markdown
---
title: "ADR-0001: Use Rust as Implementation Language"
audience: [architects, developers]
last_updated: 2026-04-17
version: "1.0.0"
status: "Accepted"
domain: "Cross-cutting"
ddmvss_categories: [domain, capability]
---

# ADR-0001: Use Rust as Implementation Language

## Status

**Accepted**

## Context

Russell requires:
- Memory safety without garbage collection
- Zero-cost abstractions
- Strong type system
- Excellent concurrency support
- Cross-platform compilation

## Decision

Use Rust as the implementation language.

**Rationale:**
- Memory safety prevents entire classes of bugs
- Zero-cost abstractions enable high performance
- Type system catches errors at compile time
- Async/await enables efficient concurrency
- Cargo simplifies dependency management

## Consequences

### Positive
- Memory safety guarantees
- High performance
- Strong compile-time checks
- Excellent tooling (cargo, clippy, rustfmt)

### Negative
- Steeper learning curve than Python/JavaScript
- Longer compile times
- Smaller ecosystem than some languages

### Risks
- Team unfamiliar with Rust (mitigation: training, pair programming)
- Longer development time initially (mitigation: start with MVP)

## Implementation

- Set up Rust workspace with `cargo init`
- Configure CI/CD with `cargo test`, `cargo clippy`
- Document Rust conventions in CONTRIBUTING.md

## References

- Rust Book: https://doc.rust-lang.org/book/
- Rust API Guidelines: https://rust-lang.github.io/api-guidelines/
```

### Example 2: Subsystem Decision

```markdown
---
title: "ADR-0201: Use SQLite for Journal Storage"
audience: [architects, developers]
last_updated: 2026-04-18
version: "1.0.0"
status: "Accepted"
domain: "Journal"
ddmvss_categories: [persistence]
---

# ADR-0201: Use SQLite for Journal Storage

## Status

**Accepted**

## Context

Journal requires:
- ACID transactions
- Hash chain integrity
- Efficient range queries
- Single-file storage
- Cross-platform support

## Decision

Use SQLite with WAL mode for journal storage.

**Rationale:**
- ACID transactions ensure consistency
- Single-file storage simplifies backup
- Efficient indexing enables fast queries
- Cross-platform support (Linux, macOS, Windows)
- Mature, battle-tested database

## Consequences

### Positive
- ACID guarantees
- Single-file storage
- Efficient queries
- No external dependencies

### Negative
- Single-writer limitation (mitigated by WAL mode)
- No built-in replication (not needed for single-host)

### Risks
- Database corruption (mitigation: hash chain, backups)
- Performance under high write load (mitigation: WAL mode, batching)

## Implementation

- Add `rusqlite` dependency to `russell-journal`
- Create schema in `migrations/`
- Implement `Journal` trait with SQLite backend
- Add hash chain verification on startup

## References

- SQLite: https://www.sqlite.org/
- SQLite WAL Mode: https://www.sqlite.org/wal.html
```

---

## Checklist

Before submitting an ADR:

- [ ] Number is unique and in correct range
- [ ] Title is short and descriptive
- [ ] Status is set correctly
- [ ] All sections completed
- [ ] Alternatives considered and documented
- [ ] Consequences (positive, negative, risks) identified
- [ ] Implementation steps are clear
- [ ] References included
- [ ] Frontmatter complete and valid

---

## References

- hKask ADR Template: `~/Clones/hKask/docs/standards/ADR_TEMPLATE.md`
- Michael Nygard. "Documenting Architecture Decisions." https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions
