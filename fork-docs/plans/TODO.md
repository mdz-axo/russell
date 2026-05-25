---
title: "Russell TODO"
audience: [developers, maintainers]
last_updated: 2026-05-25
version: "1.0.0"
status: "Active"
domain: "Cross-cutting"
ddmvss_categories: [domain, capability, interface, composition, trust, observability, persistence, lifecycle, curation]
---

# Russell TODO

**Purpose:** Track open work items for Russell.

**Process:** When an item is completed, remove it from this document and file an ADR if applicable.

---

## High Priority

### TODO-1: Implement Formal Completeness Predicate

**Category:** Domain, Capability  
**Context:** OQ-1, OQ-2  
**Description:** Define machine-checkable criteria for "Russell is complete"

**Steps:**
1. Research hKask's completeness predicate implementation
2. Define per-context predicates for each of 8 bounded contexts
3. Define system-wide integration predicate
4. Implement `is_complete()` function in `russell-core`
5. Add CI check that runs completeness predicate

**Estimated effort:** 2-3 days

---

### TODO-2: Create Formal Vocabulary Catalog

**Category:** Domain  
**Context:** OQ-5, OQ-6  
**Description:** Create `reference/vocabulary.md` with formal definitions of all Russell terms

**Steps:**
1. Extract all terms from existing documentation
2. Write formal definitions for each term
3. Add cross-references to related terms
4. Create linter that checks for vocabulary violations
5. Add vocabulary check to CI

**Estimated effort:** 1-2 days

---

### TODO-3: Implement Curation Decision Log

**Category:** Curation  
**Context:** OQ-4  
**Description:** Track curation decisions (Merge/Revise/Defer/Discard) for all documents

**Steps:**
1. Add `curation_decision` field to document frontmatter
2. Create `curation-log.md` to track decisions over time
3. Update DOCUMENTATION_STANDARDS.md with curation process
4. Add curation decision to ADR template

**Estimated effort:** 1 day

---

## Medium Priority

### TODO-4: Implement Skill Composition

**Category:** Composition  
**Context:** OQ-7, OQ-8  
**Description:** Allow skills to compose (skill A invokes skill B)

**Steps:**
1. Design composition model (declarative vs imperative)
2. Define cascade rules for failures, rollbacks, risk bands
3. Update skill manifest schema to support composition
4. Implement composition in dispatcher
5. Add tests for composed skills
6. File ADR documenting decision

**Estimated effort:** 3-5 days

---

### TODO-5: Enforce Interface Equivalence

**Category:** Interface  
**Context:** OQ-9  
**Description:** Ensure CLI ≡ ACP ≡ systemd equivalence

**Steps:**
1. Create integration tests that exercise all three surfaces
2. Generate equivalence matrix from tests
3. Add CI check that verifies equivalence
4. Document any intentional asymmetries

**Estimated effort:** 2-3 days

---

### TODO-6: Implement CNS Span Coverage Check

**Category:** Observability  
**Context:** OQ-10  
**Description:** Ensure all capability invocations emit CNS spans

**Steps:**
1. Create linter that checks for missing spans
2. Add span emission to all capability operations
3. Add CI check that verifies span coverage
4. Document span naming conventions

**Estimated effort:** 2-3 days

---

### TODO-7: Implement Journal Compaction

**Category:** Persistence  
**Context:** OQ-11  
**Description:** Compact journal when it exceeds size/time threshold

**Steps:**
1. Design compaction strategy (time-based, size-based, or hybrid)
2. Implement compaction in `russell-journal`
3. Add `russell compact` CLI command
4. Add systemd timer for automatic compaction
5. File ADR documenting decision

**Estimated effort:** 2-3 days

---

## Low Priority

### TODO-8: Define Skill Retirement Policy

**Category:** Lifecycle  
**Context:** OQ-12  
**Description:** Define when skills should be retired vs deprecated

**Steps:**
1. Research best practices for software deprecation
2. Define policy (e.g., deprecate for 90 days, then retire)
3. Update skill lifecycle documentation
4. Add warnings for deprecated skills
5. File ADR documenting decision

**Estimated effort:** 1 day

---

### TODO-9: Implement Macaroon Rotation

**Category:** Trust  
**Context:** OQ-13  
**Description:** Rotate macaroon tokens periodically

**Steps:**
1. Design rotation strategy (time-based, use-based, or TTL-only)
2. Implement rotation in `russell-acp-server`
3. Add rotation timer or counter
4. Test rotation with hKask integration
5. File ADR documenting decision

**Estimated effort:** 2-3 days

---

### TODO-10: Implement Evidence Bundle Sealing

**Category:** Trust  
**Context:** OQ-14  
**Description:** Cryptographically seal evidence bundles (ADR-0032)

**Steps:**
1. Review ADR-0032 implementation plan
2. Implement sealing in `russell-journal`
3. Add verification on journal read
4. Test with existing evidence bundles
5. Update ADR-0032 status to "Accepted"

**Estimated effort:** 2-3 days

---

### TODO-11: Align with hKask DDMVSS

**Category:** Curation  
**Context:** OQ-15  
**Description:** Ensure Russell's DDMVSS stays aligned with hKask's DDMVSS

**Steps:**
1. Review hKask DDMVSS documentation
2. Compare with Russell DDMVSS documentation
3. Identify divergences
4. Update Russell documentation to align
5. Add CI check that validates alignment

**Estimated effort:** 1-2 days

---

## Backlog

### BACKLOG-1: Add GUI Dashboard

**Category:** Interface  
**Description:** Create web-based dashboard for Russell status

**Status:** Deferred (not in MVP scope)

---

### BACKLOG-2: Add Multi-Language Support

**Category:** Interface  
**Description:** Support multiple languages for Jack's responses

**Status:** Deferred (not in MVP scope)

---

### BACKLOG-3: Add Mobile App

**Category:** Interface  
**Description:** Create mobile app for remote monitoring

**Status:** Deferred (not in MVP scope, violates magna-carta.md single-host constraint)

---

## Completed

### DONE-1: Migrate to DDMVSS

**Completed:** 2026-05-25  
**Description:** Migrated documentation from TOGAF-Lite to DDMVSS

**Result:** 19 core documents created, all aligned with hKask DDMVSS framework

---

### DONE-2: Define Bounded Contexts

**Completed:** 2026-05-25  
**Description:** Identified and documented 8 bounded contexts

**Result:** See `architecture/domain-and-capability.md` §1

---

### DONE-3: Define Principles

**Completed:** 2026-05-25  
**Description:** Documented JR-1 through JR-7 principles

**Result:** See `architecture/PRINCIPLES.md`

---

## References

- Open Questions: `OPEN_QUESTIONS.md`
- Project Status: `status/PROJECT_STATUS.md`
- Requirements: `specifications/REQUIREMENTS.md`
