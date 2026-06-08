---
title: "Russell Architecture Master Index"
audience: [architects, developers, agents]
last_updated: 2026-06-07
version: "1.1.0"
status: "Active"
domain: "Cross-cutting"
ddmvss_categories: [domain, capability, interface, composition, trust, observability, persistence, lifecycle, curation]
---

# Russell Architecture Master Index

**Purpose:** Thin pointer document to Russell's DDMVSS-aligned architecture corpus.

**Russell is a cybernetic health harness for a single Linux AI/ML workstation.** He observes the host on a 5-minute cadence, remembers what he saw in a SQLite journal, reports through ACP, watches himself (proprioception), and cries for help via a local LLM when asked.

---

## Architecture Documents

| Document | Type | DDMVSS Categories | Purpose |
|----------|------|-------------------|---------|
| [`DDMVSS.md`](DDMVSS.md) | FRAMEWORK | All 9 | DDMVSS taxonomy and methodology |
| [`PRINCIPLES.md`](PRINCIPLES.md) | FRAMEWORK | All 9 | JR-1 through JR-7, C1-C4 |
| [`magna-carta.md`](magna-carta.md) | FRAMEWORK | Trust, Lifecycle, Capability, Composition | Operator sovereignty, affirmative consent, generative space, clear boundaries (OCAP), single-host and lifecycle constraints |
| [`domain-and-capability.md`](domain-and-capability.md) | SPEC | Domain, Capability | Bounded contexts, verbs, capabilities |
| [`interface-and-composition.md`](interface-and-composition.md) | SPEC | Interface, Composition | CLI/ACP/systemd surfaces, skill composition |
| [`trust-security-observability.md`](trust-security-observability.md) | SPEC | Trust, Observability | IDRS contract, risk bands, proprioception |
| [`persistence-and-lifecycle.md`](persistence-and-lifecycle.md) | SPEC | Persistence, Lifecycle | Journal schema, skill lifecycle, ADR lifecycle |

## Reference Documents

| Document | Purpose |
|----------|---------|
| [`reference/russell-erd.md`](reference/russell-erd.md) | Entity relationship diagrams |
| [`reference/subsystem-erds.md`](reference/subsystem-erds.md) | Per-subsystem ERDs |
| [`reference/vocabulary.md`](reference/vocabulary.md) | Russell vocabulary (sentinel, journal, jack, skill, etc.) |
| [`reference/ports-inventory.md`](reference/ports-inventory.md) | Hexagonal port inventory |
| [`reference/jack-persona.md`](reference/jack-persona.md) | Jack persona specification |
| [`reference/okapi-integration.md`](reference/okapi-integration.md) | Okapi LLM integration contract |
| [`reference/hkask-integration.md`](reference/hkask-integration.md) | ACP integration |

## Specifications

| Document | Purpose |
|----------|---------|
| [`../specifications/REQUIREMENTS.md`](../specifications/REQUIREMENTS.md) | Goal specifications |
| [`../specifications/TRACEABILITY_MATRIX.md`](../specifications/TRACEABILITY_MATRIX.md) | Code → test traceability |
| [`../specifications/MODEL_CATALOG.md`](../specifications/MODEL_CATALOG.md) | LLM model catalog |

## Standards

| Document | Purpose |
|----------|---------|
| [`../standards/DOCUMENTATION_STANDARDS.md`](../standards/DOCUMENTATION_STANDARDS.md) | Documentation standards |
| [`../standards/WRITING_EXCELLENCE.md`](../standards/WRITING_EXCELLENCE.md) | Writing quality protocol |
| [`../standards/DEPENDENCY_POLICY.md`](../standards/DEPENDENCY_POLICY.md) | Dependency policy |
| [`../standards/ADR_TEMPLATE.md`](../standards/ADR_TEMPLATE.md) | ADR template |

## Operational

| Document | Purpose |
|----------|---------|
| [`../CI-CD-GUIDE.md`](../CI-CD-GUIDE.md) | CI/CD guide |
| [`../DEPLOYMENT.md`](../DEPLOYMENT.md) | Deployment guide |
| [`../status/PROJECT_STATUS.md`](../status/PROJECT_STATUS.md) | Single source of truth |
| [`../plans/TODO.md`](../plans/TODO.md) | Open work |
| [`../OPEN_QUESTIONS.md`](../OPEN_QUESTIONS.md) | Unresolved aspects |

---

## Bounded Contexts

Russell's bounded contexts (discovered from code):

| Context | Crate(s) | Verb | Ubiquitous Language |
|---------|----------|------|---------------------|
| `sentinel` | `russell-sentinel` | Observe | probe, sample, cadence, breach, rule |
| `journal` | `russell-core` (journal module) | Remember | event, hash-chain, baseline, migration, evidence |
| `jack` | `russell-meta` | Cry-for-help | persona, SOAP, prompt, inference, consent, action |
| `skill` | `russell-skills` | Act | manifest, dispatcher, IDRS, risk-band, registry |
| `acp` | `russell-acp-server`, `russell-session` | Report | session, turn, macaroon, capability, attenuation |
| `proprioception` | `russell-proprio` | Self-watch | self-vital, reflex, autoimmune, drift |
| `profile` | `russell-core` (profile module) | — | host-info, gpu-info, machine-profile |
| `operator` | `russell-cli`, `russell-agent` | — | command, pod, lifecycle, install, deploy |

---

## References

- DDMVSS framework (see architecture/DDMVSS.md)
- DDMVSS scaffold (see DDMVSS_SCAFFOLD.md)
