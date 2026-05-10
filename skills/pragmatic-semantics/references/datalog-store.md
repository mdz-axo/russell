# Datalog Store

CozoDB schema, Datalog query patterns, RDFS entailment, time-travel queries, text search, N-Quads RDF interoperability, and design principles for the semantic store.

---

## CozoDB and Datalog — The Semantic Store

CozoDB (a Datalog engine) serves as the primary semantic store. This replaces traditional SPARQL/triplestore approaches with Datalog's recursive query capabilities.

**Implementation:** `CozoStore`

**Key relations (actual CozoDB `:create` syntax):**

```cozo
:create semantic_facts {
    fact_id: String =>
    subject: String,
    predicate: String,
    object: String,
    graph: String,
    confidence: Float,
    valid_from: String,   -- RFC 3339 timestamps, NOT Int
    valid_to: String,     -- RFC 3339 timestamps, NOT Int
    prov_source: String,
    full_json: String
}

:create rdfs_classes {
    subclass: String,
    superclass: String
}

:create rdfs_types {
    entity: String,
    class: String
}

:create rdfs_domains {
    predicate: String,
    domain_class: String
}

:create rdfs_ranges {
    predicate: String,
    range_class: String
}
```

**Note:** CozoDB uses `=>` syntax to separate key columns (left) from value columns (right). `semantic_facts` has `fact_id` as the key; RDFS relations use composite keys (all columns are keys, no `=>`).

---

## Critical Datalog Patterns

**Fact storage with supersession** (Datalog query pattern):

```cozo
old_facts[fact_id, subject, predicate, object, graph, confidence, valid_from, prov_source, full_json] :=
    *semantic_facts{fact_id, subject, predicate, object, graph, confidence, valid_from, valid_to, prov_source, full_json},
    subject = $subject, predicate = $predicate, valid_to > $now
?[fact_id, subject, predicate, object, graph, confidence, valid_from, valid_to, prov_source, full_json] :=
    old_facts[fact_id, subject, predicate, object, graph, confidence, valid_from, prov_source, full_json],
    valid_to = $now
:put semantic_facts {fact_id => subject, predicate, object, graph, confidence, valid_from, valid_to, prov_source, full_json}
```

**RDFS transitive closure** (recursive Datalog pattern):

```cozo
types[class] := *rdfs_types{entity: $entity, class}
types[super] := types[sub], *rdfs_classes{subclass: sub, superclass: super}
?[class] := types[class]
```

**Time-travel queries** (CozoDB temporal access pattern):

```cozo
?[id, what, when_ts, full_json] :=
    *experiences @ {TIMESTAMP} {id, what, when_ts, full_json}
```

⚠️ **Note:** The timestamp is embedded directly in the query string because CozoDB's parser does not support parameter binding (`$param`) in the `@` position. This is a CozoDB limitation, not a design choice.

**Text search across SPO fields** (CozoDB text search pattern):

```cozo
?[full_json, confidence] :=
    *semantic_facts{subject, predicate, object, confidence, valid_to, full_json},
    valid_to > $now,
    or(
        str_includes(lowercase(subject), $search_term),
        str_includes(lowercase(predicate), $search_term),
        str_includes(lowercase(object), $search_term)
    )
:limit $limit
```

**Note:** CozoDB uses `or()` as a function-call form, not infix `or`.

**Design principle:** Searchable columns for Datalog queries + `full_json` column for complete Rust type reconstruction via serde. This gives Datalog's O(n) scan for structured queries while preserving full type fidelity.

---

## RDF Interoperability

The framework exports and imports semantic data via W3C N-Quads format.

**IRI namespace pattern:** `http://{system-domain}/`

- Subjects: `<http://{system-domain}/entity/{name}>`
- Predicates: `<http://{system-domain}/ontology#{verb}>`
- Graphs: `<http://{system-domain}/provenance/{fact_id}>`
