# Constraint System

ConstraintForce hierarchy, ConstraintKind with OT ranking, composed_satisfaction, ConstraintDistribution, and the constraint type hierarchy diagram.

---

## The Constraint System

Constraints are the typed output of the PSSD pipeline, with OT-style ranking (Prince & Smolensky 2004) and tree-structured propagation (Dechter 2003).

**ConstraintForce hierarchy:** `Guardrail ≈ Prohibition (inviolable) >> Guideline (relaxable) >> Evidence ≈ Hypothesis (informational)`

**Implementation:** `ConstraintForce` — 5 variants:

| Variant | Ontological × Epistemic | `is_met()` threshold | Composed |
|---------|------------------------|---------------------|----------|
| `Guardrail` | Prescriptive + Declarative | ≥ 1.0 - ε (ε = 1e-9) | Conjunctive (min) |
| `Prohibition` | Prescriptive + negative covenant | ≥ 1.0 - ε | Conjunctive (min) |
| `Guideline` | Prescriptive + Subjunctive/Probabilistic | ≥ 0.8 | Average (mean) |
| `Evidence` | Descriptive + Declarative | Always true | Not composed |
| `Hypothesis` | Descriptive + Subjunctive/Probabilistic | Always true | Not composed |

**`composed_satisfaction()` API:**

```rust
/// Returns None for: leaf constraints, children not found, Evidence/Hypothesis force.
pub fn composed_satisfaction(&self, all_constraints: &[Constraint]) -> Option<Confidence>
```

Composition rules: Guardrail/Prohibition → conjunctive `min()` of children; Guideline → `mean()` of children.

**ConstraintDistribution — cross-goal semantics:**

| Variant | Meaning |
|---------|---------|
| `PerGoal` (default) | Applies to each goal independently |
| `Combined` | Applies to the combined result across goals |
| `Sequential` | Applies to the order/sequence of goals |

**ConstraintKind:** 9 categories with OT rank ranges:

| Kind | Default Rank | Range |
|------|-------------|-------|
| Temporal | 35 | 31-40 |
| Entity | 45 | 41-50 |
| Relational | 55 | 51-60 |
| Value | 65 | 61-70 |
| Confidence | 75 | 71-80 |
| Coherence | 85 | 81-90 |
| Format | 95 | 91-100 |
| Scope | 105 | 101-110 |
| Quality | 115 | 111-120 |

---

## Constraint Type Hierarchy (Mermaid)

```mermaid
flowchart TD
    CS["ClassifiedSentence<br>(PSSD Tasks 1-4)"]

    CS -->|"category == Constraint"| DIRECT["Direct Constraint<br>TermProvenance::DirectlyStated"]
    CS -->|"category == Goal/Task"| GOAL["Goal Extraction"]

    SR["SentenceRelation<br>(9 RelationType variants)"]
    SR -->|"Causal/Conditional/Scope/Concessive"| DRC["derive_relation_constraints()<br>TermProvenance::RelationDerived"]
    SR -->|"Other 5 types"| LLM_IMP["LLM implicit extraction<br>TermProvenance::ImplicitInPrompt"]

    CTX["Conversation Context"] -->|"Prior turns"| INH["TermProvenance::ContextuallyInherited"]

    DIRECT --> CF{"ConstraintForce?"}
    DRC --> CF
    LLM_IMP --> CF
    INH --> CF

    CF -->|"Prescriptive+Declarative"| GR["Guardrail<br>is_met: ≥1.0-ε"]
    CF -->|"Negative covenant"| PR["Prohibition<br>is_met: ≥1.0-ε"]
    CF -->|"Prescriptive+Subj/Prob"| GL["Guideline<br>is_met: ≥0.8"]
    CF -->|"Descriptive+Declarative"| EV["Evidence<br>always met"]
    CF -->|"Descriptive+Subj/Prob"| HY["Hypothesis<br>always met"]

    style GR fill:#ffcdd2
    style PR fill:#ffcdd2
    style GL fill:#fff9c4
    style EV fill:#c8e6c9
    style HY fill:#c8e6c9
    style DRC fill:#fff3e0
```

---

## AcquisitionMethod Taxonomy

**Implementation:** `AcquisitionMethod` — 28 variants (`#[non_exhaustive]`), groupable into categories:

| Category | Variants |
|----------|---------|
| **User Input** | `UserInput`, `TextMessage`, `StructuredMessage`, `RichMessage` |
| **LLM Generation** | `LlmGeneration`, `SparAdvocateLlm`, `SparCriticLlm` |
| **Inference** | `ForwardChaining`, `BackwardChaining`, `DefeasibleReasoning`, `NativeInference`, `CognitiveReasoning` |
| **Memory/Retrieval** | `VectorSimilarity`, `SemanticRecall`, `SkillExtraction`, `SeedKnowledge` |
| **Encoding Pipeline** | `Structure`, `Print` |
| **External** | `Delegation`, `Import`, `ActualizerOutput`, `DtrustModule` |
| **System** | `DiffMutation`, `BlackboardPublish`, `OkhEvidence`, `AsExperience` |
| **Special** | `Test`, `Custom(String)` |
