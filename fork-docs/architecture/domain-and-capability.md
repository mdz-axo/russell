---
title: "Russell Domain and Capability"
audience: [architects, developers, agents]
last_updated: 2026-05-25
version: "1.0.0"
status: "Active"
domain: "Cross-cutting"
ddmvss_categories: [domain, capability]
---

# Russell Domain and Capability

**Purpose:** Define Russell's bounded contexts, domain verbs, and capability grants.

**Bounded context:** Cybernetic health harness for a single Linux AI/ML workstation.

---

## 1. Domain Ontology

### 1.1 Bounded Contexts

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

### 1.2 Entity Inventory

| Entity | Context | Storage | Retention |
|--------|---------|---------|-----------|
| `Sample` | sentinel | `journal.db::samples` | 90 days |
| `Event` | journal | `journal.db::events` | 90 days |
| `Baseline` | journal | `journal.db::baselines` | Indefinite |
| `Evidence` | journal | `~/.local/state/harness/evidence/` | 90 days |
| `Skill` | skill | `~/.local/share/harness/skills/` | Indefinite |
| `Session` | acp | In-memory | Session lifetime |
| `SelfVital` | proprioception | `journal.db::samples` (scope=self) | 90 days |
| `Profile` | profile | `~/.local/state/harness/profile.json` | Indefinite |

### 1.3 Vocabulary Catalog

Russell's domain-specific terms:

| Term | Domain | Definition |
|------|--------|-----------|
| `sentinel` | Observe | Continuous low-cost telemetry collector; writes `samples` rows |
| `journal` | Remember | SQLite database with hash chain; stores events, samples, baselines |
| `jack` | Cry-for-help | Persona that consults LLM; assembles SOAP bundle |
| `skill` | Act | YAML manifest + scripts; IDRS-compliant mutations |
| `proprioception` | Self-watch | Russell's self-observation; 9 self-vitals |
| `IDRS` | Constrain | Idempotent / Dry-run / Rollback / Structured-log |
| `risk-band` | Constrain | none / low / medium / high / critical |
| `consent` | Gate | Operator approval for interventions |
| `probe` | Observe | Read-only observation; risk: none |
| `intervention` | Act | Mutation; risk: low+ |
| `SOAP` | Cry-for-help | Subjective / Objective / Assessment / Plan bundle |
| `EWMA` | Remember | Exponentially Weighted Moving Average baseline |

---

## 2. Capability Grants

### 2.1 Verb Inventory

| Verb | Resource | Action | Risk Band | IDRS? |
|------|----------|--------|-----------|-------|
| `observe` | `probe:{id}` | Execute | none | Yes |
| `remember` | `journal:*` | Write | none | Yes |
| `cry-for-help` | `jack:*` | Consult | none | N/A |
| `act` | `intervention:{id}` | Execute | low+ | Yes |
| `report` | `acp:*` | Respond | none | N/A |
| `self-watch` | `proprioception:*` | Observe | none | Yes |
| `install` | `skill:{id}` | Install | low | Yes |
| `prune` | `skill:{id}` | Prune | low | Yes |
| `retire` | `skill:{id}` | Retire | medium | Yes |
| `query` | `journal:*` | Read | none | N/A |
| `export` | `evidence:{id}` | Export | none | N/A |

### 2.2 Capability Grant Table

| Operation | Resource | Action | Capability Required | Attenuatable? | CNS Span |
|-----------|----------|--------|---------------------|---------------|----------|
| Run probe | `probe:{id}` | Execute | `probe:execute` | Yes | `cns.probe.execute` |
| Run intervention | `intervention:{id}` | Execute | `intervention:execute` | Yes | `cns.intervention.execute` |
| Install skill | `skill:{id}` | Install | `skill:install` | Yes | `cns.skill.install` |
| Prune skill | `skill:{id}` | Prune | `skill:prune` | Yes | `cns.skill.prune` |
| Retire skill | `skill:{id}` | Retire | `skill:retire` | No (root only) | `cns.skill.retire` |
| Query journal | `journal:*` | Read | `journal:read` | Yes | `cns.journal.read` |
| Export evidence | `evidence:{id}` | Export | `evidence:export` | Yes | `cns.evidence.export` |
| Consult Jack | `jack:*` | Consult | `jack:consult` | Yes | `cns.jack.consult` |

**POLA enforcement:** Every operation requires presenting a capability token with matching `(resource, resource_id, action)`. No ambient authority.

### 2.3 Risk Band Policy

| Risk Band | Auto-execute? | Consent Required? | Example |
|-----------|---------------|-------------------|---------|
| `none` | Yes | No | Probes (read-only) |
| `low` | Yes | No | Restart service, toggle config |
| `medium` | No | Yes | Edit config, install package |
| `high` | No | Yes + confirmation | Kernel module reload |
| `critical` | Never | Explicit approval | Delete files, format disk |

**Honeymoon period:** For the first 30 days after bootstrap, Russell clamps effective `max_auto_risk` to `low` for any skill with `risk: high` interventions.

---

## 3. Focusing Assumptions

### FA-D1: Domain Vocabulary is Bounded to 50 Terms

**Statement:** Russell's domain vocabulary is bounded to 50 terms.

**Rationale:** Miller's law — 7±2 categories, 8 bounded contexts.

**Evidence:** Current vocabulary catalog lists 12 terms (see §1.3).

---

### FA-D2: One Bounded Context per Crate

**Statement:** Each bounded context maps to one or more Rust crates. No context without a crate.

**Rationale:** Code structure is the source of truth for context boundaries.

**Evidence:** See §1.1 Bounded Contexts table.

---

### FA-C1: All Capabilities Surface Through CLI ≡ ACP ≡ systemd

**Statement:** All capabilities surface through three interfaces: CLI, ACP, systemd.

**Rationale:** Three projections of one core. Collapses entire UX specification dimension.

**Evidence:** See `interface-and-composition.md` §2 Interface Equivalence Matrix.

---

## 4. Cross-References

| Category | Relation |
|----------|----------|
| Capability | Domain verbs → capability grants |
| Persistence | Domain entities → storage schemas |
| Curation | Domain terms validated during curation |
| Trust | Capabilities governed by risk bands |
| Interface | Capabilities surface through interfaces |
| Composition | Capabilities compose via skill manifests |

---

## 5. Completeness Checklist

- [x] Every probe type has a named term in Russell vocabulary
- [x] Bounded context map drawn
- [x] No entity without a storage schema
- [x] Every domain verb has a capability grant
- [x] Risk band policy defined
- [x] Attenuation policy defined

---

## References

- DDMVSS framework (see architecture/DDMVSS.md) §3
- Evans, E. (2003). *Domain-Driven Design*. Addison-Wesley.
- Miller, M.S. (2003). "Robust Composition." PhD thesis, Johns Hopkins.
