---
title: "Russell Replicant — hKask Universal Agentic Registry"
audience: [operators, developers, agents]
last_updated: 2026-05-24
togaf_phase: "Preliminary"
version: "1.1.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Business Architecture -->
<!-- VERSION: 1.1.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-24 -->

---
title: "Russell Replicant — hKask Universal Agentic Registry"
audience: [operators, developers, agents]
last_updated: 2026-05-24
togaf_phase: "Preliminary"
version: "1.1.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Business Architecture -->
<!-- VERSION: 1.1.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-24 -->

# Russell Replicant — hKask Universal Agentic Registry

**Replicant ID**: `russell`  
**Type**: Cybernetic Health Harness  
**Platform**: Linux (single-host, single-operator)  
**Persona**: Jack the Nurse  

## Overview

Russell is a replicant in the hKask Universal Agentic Registry — a cybernetic health harness that observes Linux AI/ML workstations, remembers what it saw in a SQLite journal, and reports through the ACP (Agent Client Protocol) server and a local CLI. When asked, Russell cries for help via a local LLM (Okapi by default).

## Visibility Model

Russell follows a **public-except-episodic** visibility model:

| Artifact Type | Visibility | Rationale |
|---|---|---|
| **Semantic Memory** | `public` | Skills, manifests, templates — shareable knowledge |
| **Templates** | `public` | Prompt templates, skill templates — reusable patterns |
| **Manifests** | `public` | Skill manifests, replicant descriptor — public registry |
| **References** | `public` | Documentation, ADRs, specifications — open source |
| **Episodic Memory** | `private` | Journal entries, operator sessions — instance-specific |

## Skills Catalog

Russell ships with 13 public skills:

| Skill | Symptoms | Probes | Interventions | Visibility |
|---|---|---|---|---|
| `okapi-watcher` | llm_slow, resource_exhaustion, gpu_fallback_to_cpu | 3 | 1 | public |
| `skill-manager` | skill_not_in_catalog, skill_version_stale, skill_coverage_gap | 4 | 6 | public |
| `skill-workshop` | skill_composition, skill_adaptation | 2 | 4 | public |
| `skill-maintenance` | skill_stale, skill_coverage_gap | 2 | 2 | public |
| `skill-discovery` | skill_missing, skill_search | 2 | 3 | public |
| `sysadmin` | zombie_accumulation, clock_skew, systemd_degraded | 2 | 3 | public |
| `scenario-tester` | performance_regression, threshold_breach | 7 | 0 | public |
| `journal-compactor` | journal_size_excessive | 1 | 1 | public |
| `pragmatic-cybernetics` | cybernetic_principles | 0 | 0 | public |
| `pragmatic-semantics` | semantic_clarity | 0 | 0 | public |
| `ubuntu-jack` | ubuntu_specific_health | 2 | 1 | public |
| `web-search` | web_research_needed | 1 | 0 | public |
| `package-checker` | package_outdated, package_missing | 1 | 0 | public |

**Total**: 25 probes, 21 interventions, 30 symptoms in catalog

## Architecture

Russell implements the **Observe > Recommend > Act** posture (JR-2):

1. **Sentinel** — 5-minute cadence probe collection
2. **Journal** — SQLite with hash-chain integrity (tamper-evident)
3. **Nurse (Jack)** — LLM consultation via Okapi/OpenRouter
4. **Proprioception** — Self-observation (7 self-vitals)
5. **Skills** — YAML manifests + scripts (IDRS-contract mutations)
6. **ACP Server** — Agent Client Protocol for hKask integration (primary interface)

## IDRS Contract

All mutations satisfy the IDRS contract:

- **I** — Idempotent (second run = first run's end state)
- **D** — Dry-run (`--dry-run` flag produces would-do record)
- **R** — Rollback (pre-state captured, reverse intervention)
- **S** — Structured log (`harness.event.v1` in journal)

## hKask Integration

Russell skills are compatible with hKask's skill registry:

- `SKILL.md` — Universal skill definition (OpenClaw-compatible)
- `skill.json` — Manifest descriptor (SchemaStore-registered)
- `manifest.yaml` — Russell-native format (backward-compatible)

Skills can be published to the hKask registry via HCS-26 topic registries.

## Installation

```bash
git clone https://github.com/mdz-axolotl/russell.git
cd russell
cargo install --path crates/russell-cli
russell skill list
russell jack
```

## Replicant Metadata

```json
{
  "replicant": {
    "id": "russell",
    "type": "health-harness",
    "platform": "linux",
    "persona": "jack-nurse",
    "visibility": {
      "model": "public-except-episodic",
      "semantic": "public",
      "templates": "public",
      "manifests": "public",
      "references": "public",
      "episodic": "private"
    }
  }
}
```

## References

- [AGENTS.md](AGENTS.md) — Binding orientation for contributors
- [docs/README.md](docs/README.md) — Architecture portal
- [docs/specifications/MVP_SPEC.md](docs/specifications/MVP_SPEC.md) — MVP boundary
- [docs/architecture/PRINCIPLES_CATALOG.md](docs/architecture/PRINCIPLES_CATALOG.md) — JR-1 through JR-7
- [docs/architecture/THE_JACK.md](docs/architecture/THE_JACK.md) — Jack persona specification

---

*Russell is a single-host, single-operator harness. He does not mutate host state outside his own skill sandbox, and never acts on LLM output as shell commands (JR-3).*
