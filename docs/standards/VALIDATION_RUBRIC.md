---
title: "Documentation Excellence Validation Rubric"
audience: [architects, developers, agents]
last_updated: 2026-05-13
togaf_phase: "Preliminary"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Cross-cutting -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-13 -->

<!-- DIAGRAM_ALIGNMENT
id: DIAG-RUBRIC-001
type: ER diagram
verified_date: 2026-05-13
verified_against: TOGAF_LITE_FOR_OPEN_SOURCE.md, WRITING_EXCELLENCE.md, DOCUMENTATION_STANDARDS.md
reference_sources: All three governing standards
status: VERIFIED
-->

# Documentation Excellence Validation Rubric

## 1. TOGAF-Lite Structural Conformance (RDF Triples)

Every document MUST satisfy these triples:

```
<doc> rdf:type togl:Document .
<doc> togl:hasFrontmatter [
    togl:title "string" ;
    togl:audience (operators | developers | contributors | architects | agents) ;
    togl:last_updated "YYYY-MM-DD" ;
    togl:togaf_phase (Preliminary | A | B | C | D | E | F | G | H | Requirements Management) ;
    togl:version "MAJOR.MINOR.PATCH" ;
    togl:status (Active | Proposed | Superseded | Deprecated | Draft)
] .
```

**Pass criteria:** All six frontmatter fields present in YAML block AND HTML comment block.

**Fail criteria:** Any field missing → `togl:validationScore 0`.

## 2. Diagram Alignment Contract

```
<doc> togl:hasDiagram <diag> .
<diag> togl:hasAlignment [
    togl:id "DIAG-<TOPIC>-<LABEL>-<NNN>" ;
    togl:type (flowchart | sequenceDiagram | stateDiagram-v2 | gantt | erDiagram) ;
    togl:verified_date "YYYY-MM-DD" ;
    togl:verified_against "path/to/source" ;
    togl:reference_sources "citation" ;
    togl:status (VERIFIED | STALE | DEPRECATED)
] .
```

**Pass criteria:** Every ` ```mermaid ` block has corresponding `DIAGRAM_ALIGNMENT` HTML comment.

## 3. Writing Excellence Four Tests (Scoring)

```
<doc> we:evaluatedBy [
    we:hopper_test (PASS | FAIL) ;    # Zero-context accessibility
    we:lovelace_test (PASS | FAIL) ;  # Independent verifiability
    we:schriver_test (PASS | FAIL) ;  # 30-second findability
    we:gente_test (PASS | FAIL)      # Agent-correctness
] .
```

**Pass criteria:** ≥ 2 of 4 tests pass for publication. ≥ 3 of 4 for excellence.

**Hopper:** First reading without prior context → navigable headings, no assumed knowledge.
**Lovelace:** Specification independently testable → data models, API contracts, state transitions.
**Schriver:** Target info within 30 seconds → table of contents, scannable headings, navigation table.
**Gente:** AI agent consumes as sole truth → valid frontmatter, consistent structure, no stale claims.

## 4. Voice and Style Constraints

```
<doc> we:voice [
    we:register (formal-technical | narrative-metaphorical | procedural-operational) ;
    we:maxSentenceLength 35 ;
    we:usesActiveVoice true ;
    we:usesPresentTense true ;
    we:usesRFC2119Keywords true  # for specs
] .
```

**Fail criteria:** Sentence > 35 words, passive voice, hedging language ("should probably", "might", "could potentially").

## 5. Structural Pattern

```
<section> we:followsPattern "Statement → Evidence → Diagram → Implications" .
```

**Pass criteria:** Every `##` section states claim first, provides evidence (code path/citation), references diagram if present, states implications.

## 6. Freshness Contract

```
<doc> togl:hasLastUpdated <date> .
<date> togl:daysAgo ?days .
?days togl:freshnessState (fresh | review | stale) .
```

**fresh:** days ≤ 90
**review:** 91 ≤ days ≤ 180
**stale:** days ≥ 181 → must update, archive, or explicitly exempt.

## 7. Authority Hierarchy Enforcement

```
<docA> togl:authoritativeTier 1 .  # AGENTS.md
<docB> togl:authoritativeTier 2 .  # docs/README.md
<docC> togl:authoritativeTier 3 .  # status docs
<docD> togl:authoritativeTier 4 .  # specs
<docE> togl:authoritativeTier 5 .  # principles
<docF> togl:authoritativeTier 6 .  # ADRs

<docLower> togl:contradicts <docHigher> .
→ VIOLATION: lower-tier contradicts higher-tier
```

## 8. Cross-Repository Provenance (JR-6)

```
<doc> togl:isCopyOf <upstream> .
<upstream> togl:sourcePath "path" ;
           togl:upstreamCommit "sha" ;
           togl:syncPolicy "manual | auto" .
```

**Pass criteria:** Every standards file copied across repos has REUSE_MANIFEST.md entry.

## 9. Diataxis Classification

```
<doc> diataxis:quadrant (tutorial | how-to | reference | explanation) .
```

**Fail criteria:** Document claims multiple quadrants without section-level disambiguation.

## 10. Validation Summary Table

| Check | Field | Pass | Fail |
|-------|-------|------|------|
| Frontmatter complete | title, audience, last_updated, togaf_phase, version, status | 6/6 | < 6 |
| HTML comment block | TOGAF_DOMAIN, VERSION, STATUS, LAST_UPDATED | 4/4 | < 4 |
| Diagram alignment | DIAGRAM_ALIGNMENT for every mermaid block | 1:1 | missing |
| Hopper test | Zero-context accessibility | PASS | FAIL |
| Lovelace test | Independent verifiability | PASS | FAIL |
| Schriver test | 30-second findability | PASS | FAIL |
| Gente test | Agent-correctness | PASS | FAIL |
| Sentence length | ≤ 35 words | PASS | FAIL |
| Voice | Active, present tense, no hedging | PASS | FAIL |
| Freshness | ≤ 90 days | fresh | review / stale |
| Authority | No contradictions with higher tier | PASS | VIOLATION |
| Provenance | Cross-repo copies in REUSE_MANIFEST | PASS | FAIL |
