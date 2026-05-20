---
name: okapi-watcher
version: 0.3.0
author: Russell (Jack the Nurse)
license: MIT OR Apache-2.0
description: Monitor local Okapi instance health and model inventory. Probes Okapi health, available models, and GPU library status. Can restart Okapi service if degraded.
symptoms:
  - llm_slow
  - resource_exhaustion
  - gpu_fallback_to_cpu
tags:
  - llm
  - okapi
  - health-monitor
  - gpu
permissions:
  - shell
  - network
config:
  okapi_url:
    type: string
    required: false
    default: "http://localhost:11434"
    description: "Okapi API endpoint"
entryPoint:
  type: shell
  path: "./scripts/probe-health.sh"
triggers:
  schedule: "*/5 * * * *"
visibility: public
replicant: russell
artifact_type: skill
---

# Okapi Watcher Skill

Monitors the local Okapi LLM instance for health issues, model availability, and GPU library status.

## Probes

| ID | Command | Timeout | Capture |
|---|---|---|---|
| `probe-health` | `./scripts/probe-health.sh` | 10s | stdout |
| `probe-models` | `./scripts/probe-models.sh` | 10s | stdout |
| `probe-gpu-libs` | `./scripts/probe-gpu-libs.sh` | 15s | stdout |

## Interventions

| ID | Command | Risk | Rollback |
|---|---|---|---|
| `restart-okapi` | `systemctl --user restart okapi` | low | none_needed |

## When Applied

- OS family: Linux
- Symptoms: `llm_slow`, `resource_exhaustion`, `gpu_fallback_to_cpu`

## Safety

- Max auto-risk: `low`
- Interventions above `low` require operator consent

## References

- [Okapi AGENTS.md](~/Clones/Okapi/AGENTS.md)
- [Kask Integration Points](~/Clones/Okapi/fork-docs/plans/KASK_INTEGRATION_POINTS.md)

## hKask Replicant Metadata

This skill is part of the **Russell** replicant profile in the hKask Universal Agentic Registry.

- **Replicant ID**: `russell`
- **Visibility**: `public` (semantic memory artifact)
- **Episodic Memory**: Not included (private to Russell instance)
- **Registry Topic**: HCS-2 skill discovery registry

> Russell is a cybernetic health harness for Linux AI/ML workstations. He observes, remembers, and reports — never mutates without IDRS contract.
