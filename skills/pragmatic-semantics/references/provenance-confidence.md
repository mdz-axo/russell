# Provenance and Confidence

ProvenanceSemiring, ProvenanceTag, DerivationPath, ActivationState (ACT-R decay), and confidence propagation algebra.

---

## Provenance and Confidence Propagation

The provenance semiring system (Scallop-inspired; Li et al. 2023) tracks how facts are derived and computes confidence via semiring algebra.

**Types:**

- `ProvenanceTag{confidence, is_derived, derivations: Vec<DerivationPath>}`
- `DerivationPath{rule_name, antecedent_ids, antecedent_confidences, path_confidence}`
- `ProvenanceSemiring` trait: `zero(), one(), conjoin(a,b), disjoin(a,b)`
- `MinMaxSemiring`: ⊗=min, ⊕=max (conservative default)
- `ProductSemiring`: ⊗=product, ⊕=noisy-or (probabilistic independence)

**Integration points:**

- Forward chaining tags derived facts with provenance
- `ProvenanceEngine` in `MemorySystem` provides `derive_with_provenance()` and `explain_confidence()`
- `ActivationState` (ACT-R; Anderson & Lebiere 1998) provides empirically-grounded memory decay: B_i = ln(Σ t_j^{-d})
