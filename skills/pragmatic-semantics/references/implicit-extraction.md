# Implicit Extraction

RelationType (9 variants), derive_relation_constraints() mapping, TermProvenance (4 variants), SentenceRelation, and the implicit constraint extraction pipeline.

---

## RelationType — Inter-Sentence Semantics

**Implementation:** `RelationType` — 9 variants representing semantic relationships severed during SVO decimation:

| Variant | Marker | Example | Constraint? |
|---------|--------|---------|-------------|
| `Causal` | "because" | "because X, Y" — X causes/justifies Y | Yes → Relational |
| `Conditional` | "if...then" | "if X, then Y" — X conditions Y | Yes → Scope |
| `TemporalCondition` | "when" | "when X, Y" — temporal conditioning | LLM only |
| `Concessive` | "although" | "although X, Y" — X holds despite Y | Yes → Quality |
| `Contrastive` | "but" | "but X" — contrastive force | LLM only |
| `Scope` | "only from" | "only from X" — scope restriction | Yes → Scope (Guardrail!) |
| `Purpose` | "in order to" | "in order to X" — teleological | LLM only |
| `Presupposition` | (pragmatic) | Unstated assumption | LLM only |
| `Conjunctive` | "and" | Simple conjunction | LLM only |

Carried by `SentenceRelation {from_index, to_index, relation, marker, confidence}`.

---

## Implicit Constraint Extraction Pipeline

**Implementation:** `derive_relation_constraints()` — pure, deterministic, no LLM:

| RelationType | Constraint Kind | Force | Notes |
|---|---|---|---|
| `Causal` | `Relational` | `Guideline` | Cause → validity requirement (skipped if already classified as Constraint) |
| `Conditional` | `Scope` | `Guideline` | Condition → conditional scope constraint |
| `Scope` | `Scope` | **`Guardrail`** | Scope restriction — inviolable! |
| `Concessive` | `Quality` | `Guideline` | Conceded element → quality floor |
| Others | — | — | Handled by LLM-based `extract_implicit_constraints()` |

All derived constraints carry `TermProvenance::RelationDerived {relation_type, from_index, to_index}`.

---

## TermProvenance — 4 Variants

| Variant | Meaning | Source |
|---------|---------|--------|
| `DirectlyStated` (default) | Explicitly stated by user | Current utterance |
| `ImplicitInPrompt{implicit_type, confidence}` | Inferred from linguistic structure | Entailment, presupposition, speech act |
| `ContextuallyInherited{source_turn, source_description}` | Inherited from conversation context | Prior turns, established preferences |
| `RelationDerived{relation_type, from_index, to_index}` | Derived from inter-sentence relations | `derive_relation_constraints()` output |
