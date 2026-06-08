---
title: "Russell Open Questions"
audience: [architects, developers, agents]
last_updated: 2026-05-25
version: "1.0.0"
status: "Active"
domain: "Cross-cutting"
ddmvss_categories: [domain, capability, interface, composition, trust, observability, persistence, lifecycle, curation]
---

# Russell Open Questions

**Purpose:** Track unresolved aspects of Russell's architecture and implementation.

**Process:** When a question is resolved, file an ADR and remove it from this document.

---

## Completeness

### OQ-1: Formal Completeness Predicate

**Question:** How do we formally define "Russell is complete"? What are the machine-checkable criteria?

**Context:** DDMVSS requires a completeness predicate, but Russell lacks a machine-checkable "done" definition.

**Options:**
1. Per-bounded-context completeness (each context has its own predicate)
2. System-wide completeness (single predicate for all of Russell)
3. Hybrid (context predicates + cross-context integration predicate)

**Status:** Open

---

### OQ-2: Completeness Scope

**Question:** Should completeness be per-bounded-context or system-wide?

**Context:** Russell has 8 bounded contexts. Should each have its own completeness predicate, or should there be a single system-wide predicate?

**Status:** Open (see OQ-1)

---

## Curation

### OQ-3: Curation Authority

**Question:** Who curates Russell's specs? The operator? An automated process?

**Context:** DDMVSS requires curation decisions (Merge/Revise/Defer/Discard), but Russell has no defined curator.

**Options:**
1. Operator curates manually
2. Automated curation bot
3. Hybrid (automated evaluation, operator approval)

**Status:** Open

---

### OQ-4: Curation Decision Log

**Question:** How do we track curation decisions over time?

**Context:** ADRs lack explicit Merge/Revise/Defer/Discard decisions.

**Options:**
1. Add curation decision field to ADR frontmatter
2. Separate curation log file
3. Git commit messages with curation tags

**Status:** Open

---

## Vocabulary

### OQ-5: Formal Vocabulary Catalog

**Question:** Should Russell have a formal vocabulary catalog?

**Context:** Russell has domain-specific terms (sentinel, journal, jack, skill, etc.) but no formal catalog with definitions and allocations.

**Options:**
1. Create `reference/vocabulary.md` with formal definitions
2. Create Russell-specific vocabulary catalog
3. No formal catalog (current informal approach)

**Status:** Open

---

### OQ-6: Vocabulary Drift Prevention

**Question:** How do we prevent vocabulary drift (e.g., "sentinel" vs "telemetry collector")?

**Context:** Without enforcement, documentation may use inconsistent terms.

**Options:**
1. Linter that checks for vocabulary violations
2. Code review checklist
3. Automated documentation generation from code

**Status:** Open

---

## Composition

### OQ-7: Skill Composition

**Question:** How do skills compose? Can skill A invoke skill B?

**Context:** Current skill model is flat (no composition). Should skills be composable?

**Options:**
1. No composition (current model)
2. Declarative composition (manifest references)
3. Imperative composition (skill invokes skill)

**Status:** Open

---

### OQ-8: Cascade Rules

**Question:** What are the cascade rules for skill composition?

**Context:** If skills compose, what are the rules for cascading failures, rollbacks, and risk bands?

**Status:** Open (depends on OQ-7)

---

## Interface

### OQ-9: Interface Equivalence Enforcement

**Question:** How do we enforce CLI ≡ ACP ≡ systemd equivalence?

**Context:** The three surfaces should exercise the same functional core, but there's no automated check.

**Options:**
1. Integration tests that exercise all three surfaces
2. Code generation from single source
3. Manual review (current approach)

**Status:** Open

---

## Observability

### OQ-10: CNS Span Coverage

**Question:** How do we ensure all capability invocations emit CNS spans?

**Context:** DDMVSS requires CNS spans for all operations, but Russell has no automated check.

**Options:**
1. Linter that checks for missing spans
2. Integration tests that verify span emission
3. Code review checklist

**Status:** Open

---

## Persistence

### OQ-11: Journal Compaction Strategy

**Question:** When and how do we compact the journal?

**Context:** Journal grows over time. Current retention is 90 days, but no compaction strategy is defined.

**Options:**
1. Time-based compaction (delete rows older than 90 days)
2. Size-based compaction (compact when journal exceeds threshold)
3. Hybrid (time + size)

**Status:** Open

---

## Lifecycle

### OQ-12: Skill Retirement Policy

**Question:** When should skills be retired vs deprecated?

**Context:** Skill lifecycle has both "deprecated" and "retired" states. When should each be used?

**Options:**
1. Deprecate first, retire after N days
2. Retire immediately (no deprecation)
3. Operator decides case-by-case

**Status:** Open

---

## Security

### OQ-13: Macaroon Rotation

**Question:** How often should macaroon tokens be rotated?

**Context:** ACP uses macaroon-based OCAP tokens, but no rotation policy is defined.

**Options:**
1. Time-based rotation (every N days)
2. Use-based rotation (after N uses)
3. No rotation (tokens expire via TTL)

**Status:** Open

---

### OQ-14: Evidence Bundle Sealing

**Question:** Should evidence bundles be cryptographically sealed?

**Context:** ADR-0032 proposes evidence bundle sealing, but implementation is deferred.

**Options:**
1. Implement sealing (per ADR-0032)
2. No sealing (current approach)
3. Optional sealing (operator configures)

**Status:** Deferred (see ADR-0032)

---

## Integration

### OQ-15: DDMVSS Alignment

**Question:** How do we ensure Russell's DDMVSS stays internally consistent?

**Context:** Russell's DDMVSS defines the project's architecture, but may drift over time.

**Options:**
1. Regular review of documentation consistency
2. Automated validation (CI check against DDMVSS schema)
3. No alignment (Russell evolves independently)

**Status:** Open

---

## Resolved Questions

### RQ-1: Journal Storage

**Question:** What storage engine for the journal?

**Answer:** SQLite with WAL mode (ADR-0004)

---

### RQ-2: LLM Backend

**Question:** What LLM backend for Jack?

**Answer:** Okapi (default) or OpenRouter (opt-in) (ADR-0008, ADR-0016)

---

### RQ-3: Skill Manifest Format

**Question:** What format for skill manifests?

**Answer:** YAML (ADR-0023)

---

## References

- DDMVSS framework (see architecture/DDMVSS.md)
- DDMVSS: `architecture/DDMVSS.md` §5
