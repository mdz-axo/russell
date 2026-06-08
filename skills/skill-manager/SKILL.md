---
name: skill-manager
version: 1.0.0
author: Russell (Jack the Nurse)
license: MIT
description: Enables Jack to build, install, modify, prune, and measure skills autonomously from within russell chat. Bundled meta-skill wrapping russell skill CLI subcommands.
symptoms:
  - skill_not_in_catalog
  - skill_version_stale
  - skill_install_failed
  - skill_manifest_invalid
  - skill_probe_script_missing
  - skill_coverage_gap
tags:
  - skill-management
  - meta-skill
  - automation
  - russell
permissions:
  - shell
  - filesystem
config:
  skills_dir:
    type: string
    required: false
    default: "~/.local/share/harness/skills"
    description: "Base skills directory"
entryPoint:
  type: shell
  path: "russell skill list"
visibility: public
replicant: russell
artifact_type: skill
---

# Skill Manager Skill

Enables Jack to build, install, modify, prune, and measure skills autonomously from within `russell chat`.

## Probes

| ID | Command | Timeout | Capture |
|---|---|---|---|
| `list-skills` | `russell skill list` | 10s | stdout |
| `stats` | `russell skill stats` | 10s | stdout |
| `check` | `russell skill check` | 10s | stdout |

## Interventions

| ID | Command | Risk | Rollback |
|---|---|---|---|
| `install` | `russell skill install` | low | none_needed |
| `build` | `russell skill build` | low | none_needed |
| `create-manifest` | `russell skill put` | low | none_needed |
| `prune` | `russell skill prune` | low | restore |
| `restore` | `russell skill restore` | low | none_needed |
| `delete` | `russell skill retire` | medium | none_needed |

## When Applied

- OS family: Linux
- Symptoms: skill_not_in_catalog, skill_version_stale, skill_install_failed, skill_manifest_invalid, skill_probe_script_missing, skill_coverage_gap

## Safety

- Max auto-risk: `low`
- Interventions above `low` require operator consent

## References

- docs/architecture/skill-self-management-strategy.md
- docs/adr/0024-skill-registry-workshop-lifecycle.md
- skills/skill-workshop/KNOWLEDGE.md
- skills/skill-maintenance/KNOWLEDGE.md

## Replicant Metadata

This skill is part of the **Russell** replicant profile.

- **Replicant ID**: `russell`
- **Visibility**: `public` (semantic memory artifact)
- **Episodic Memory**: Not included (private to Russell instance)

> Russell is a cybernetic health harness for Linux AI/ML workstations. He observes, remembers, and reports — never mutates without IDRS contract.
