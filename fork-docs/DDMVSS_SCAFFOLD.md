---
title: "Russell DDMVSS Scaffold"
audience: [architects, maintainers]
last_updated: 2026-05-25
version: "1.0.0"
status: "Active"
domain: "Cross-cutting"
ddmvss_categories: [curation]
---

# Russell DDMVSS Scaffold

**Purpose:** Index of Russell's DDMVSS-aligned documentation corpus.

**Framework:** Based on hKask's DDMVSS (Domain-Driven Minimal Viable Specification Set)

---

## Documentation Corpus (19 Documents)

### Framework Documents (4)

| Document | Purpose | DDMVSS Categories |
|----------|---------|-------------------|
| [`architecture/russell-architecture-master.md`](architecture/russell-architecture-master.md) | Master index | All |
| [`architecture/DDMVSS.md`](architecture/DDMVSS.md) | Framework taxonomy | All |
| [`architecture/PRINCIPLES.md`](architecture/PRINCIPLES.md) | JR-1 through JR-7 | All |
| [`architecture/magna-carta.md`](architecture/magna-carta.md) | Operator sovereignty | Trust, Lifecycle |

### Specification Documents (4)

| Document | Purpose | DDMVSS Categories |
|----------|---------|-------------------|
| [`architecture/domain-and-capability.md`](architecture/domain-and-capability.md) | Bounded contexts, verbs | Domain, Capability |
| [`architecture/interface-and-composition.md`](architecture/interface-and-composition.md) | CLI/ACP/systemd, skills | Interface, Composition |
| [`architecture/trust-security-observability.md`](architecture/trust-security-observability.md) | IDRS, risk bands, journal | Trust, Observability |
| [`architecture/persistence-and-lifecycle.md`](architecture/persistence-and-lifecycle.md) | Storage, lifecycle | Persistence, Lifecycle |

### Standards Documents (4)

| Document | Purpose | DDMVSS Categories |
|----------|---------|-------------------|
| [`standards/DOCUMENTATION_STANDARDS.md`](standards/DOCUMENTATION_STANDARDS.md) | Documentation standards | Curation |
| [`standards/WRITING_EXCELLENCE.md`](standards/WRITING_EXCELLENCE.md) | Writing quality | Curation |
| [`standards/DEPENDENCY_POLICY.md`](standards/DEPENDENCY_POLICY.md) | Dependency management | Trust, Lifecycle |
| [`standards/ADR_TEMPLATE.md`](standards/ADR_TEMPLATE.md) | ADR template | Curation, Lifecycle |

### Operational Documents (7)

| Document | Purpose | DDMVSS Categories |
|----------|---------|-------------------|
| [`DDMVSS_SCAFFOLD.md`](DDMVSS_SCAFFOLD.md) | This document | Curation |
| [`OPEN_QUESTIONS.md`](OPEN_QUESTIONS.md) | Unresolved aspects | All |
| [`CI-CD-GUIDE.md`](CI-CD-GUIDE.md) | CI/CD guide | Lifecycle |
| [`DEPLOYMENT.md`](DEPLOYMENT.md) | Deployment guide | Lifecycle |
| [`plans/TODO.md`](plans/TODO.md) | Open work | All |
| [`status/PROJECT_STATUS.md`](status/PROJECT_STATUS.md) | Single source of truth | All |
| [`specifications/REQUIREMENTS.md`](specifications/REQUIREMENTS.md) | Goal specifications | Domain, Capability |

---

## Reference Documents

Located in `architecture/reference/`:

| Document | Purpose |
|----------|---------|
| `russell-erd.md` | Entity relationship diagrams |
| `subsystem-erds.md` | Per-subsystem ERDs |
| `vocabulary.md` | Russell vocabulary |
| `ports-inventory.md` | Hexagonal port inventory |
| `jack-persona.md` | Jack persona specification |
| `okapi-integration.md` | Okapi LLM integration |
| `hkask-integration.md` | hKask ACP integration |

---

## Decision Records

Located in `decisions/`:

- **Active ADRs:** `decisions/NNNN-*.md`
- **Deferred ADRs:** `decisions/deferred/NNNN-*.md`

See [`standards/ADR_TEMPLATE.md`](standards/ADR_TEMPLATE.md) for template.

---

## DDMVSS Categories

Russell's documentation is organized by 9 DDMVSS categories:

| Category | Description | Primary Documents |
|----------|-------------|-------------------|
| **Domain** | Bounded contexts, vocabulary | domain-and-capability.md |
| **Capability** | Verbs, grants | domain-and-capability.md |
| **Interface** | CLI, ACP, systemd | interface-and-composition.md |
| **Composition** | Skills, manifests | interface-and-composition.md |
| **Trust** | IDRS, risk bands, security | trust-security-observability.md, magna-carta.md |
| **Observability** | Journal, proprioception, CNS spans | trust-security-observability.md |
| **Persistence** | Storage, schemas | persistence-and-lifecycle.md |
| **Lifecycle** | Bootstrap, evolution, deprecation | persistence-and-lifecycle.md, magna-carta.md |
| **Curation** | Documentation standards | DOCUMENTATION_STANDARDS.md, WRITING_EXCELLENCE.md |

---

## Bounded Contexts

Russell has 8 bounded contexts:

| Context | Crate(s) | Verb |
|---------|----------|------|
| `sentinel` | `russell-sentinel` | Observe |
| `journal` | `russell-core` (journal module) | Remember |
| `jack` | `russell-meta` | Cry-for-help |
| `skill` | `russell-skills` | Act |
| `acp` | `russell-acp-server`, `russell-session` | Report |
| `proprioception` | `russell-proprio` | Self-watch |
| `profile` | `russell-core` (profile module) | — |
| `operator` | `russell-cli`, `russell-agent` | — |

---

## Principles

Russell is governed by 7 principles (JR-1 through JR-7):

| Principle | Statement |
|-----------|-----------|
| **JR-1** | Austere by default |
| **JR-2** | Observe > Recommend > Act |
| **JR-3** | The LLM never emits shell |
| **JR-4** | Small but present — the Nurse |
| **JR-5** | Proprioception — Jack watches Jack |
| **JR-6** | Reuse, don't depend |
| **JR-7** | Persistence is auditable |

See [`architecture/PRINCIPLES.md`](architecture/PRINCIPLES.md) for full definitions.

---

## Completeness Predicate

Russell's documentation is **complete** when:

1. All 19 core documents are Active
2. All 9 DDMVSS categories have at least one Active document
3. All bounded contexts are documented
4. All principles are defined and traced to contexts
5. All capabilities are documented with risk bands
6. All interfaces are documented with equivalence matrix
7. All storage schemas are documented
8. All lifecycle transitions are documented
9. Coherence score ≥ 0.7

---

## Maintenance

### Update Cadence

- **Weekly:** Review OPEN_QUESTIONS.md
- **Monthly:** Review PROJECT_STATUS.md
- **Quarterly:** Review all Active documents for staleness

### Update Process

1. Edit document
2. Update `last_updated` field
3. Bump `version` field
4. Commit with message: `docs: update <document-name>`
5. PR review required for Active documents

See [`standards/DOCUMENTATION_STANDARDS.md`](standards/DOCUMENTATION_STANDARDS.md) for full process.

---

## References

- hKask DDMVSS: `~/Clones/hKask/docs/architecture/DDMVSS.md`
- hKask Scaffold: `~/Clones/hKask/docs/DDMVSS_SCAFFOLD.md`
- Evans, E. (2003). *Domain-Driven Design*. Addison-Wesley.
