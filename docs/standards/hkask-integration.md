---
title: "Russell in hKask — Replicant Integration Guide"
audience: [developers, operators, agents]
last_updated: 2026-05-19
togaf_phase: "H"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Change Management — hKask Integration -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-19 -->

# Russell in hKask — Replicant Integration Guide

## Overview

Russell is a **replicant** in the hKask Universal Agentic Registry — a cybernetic health harness for Linux AI/ML workstations that observes, remembers, and reports. Russell's skills, templates, and manifests are published to the hKask registry with **public visibility** (except episodic memory, which remains private).

## Visibility Model

Russell follows a **public-except-episodic** visibility model:

```yaml
visibility:
  model: public-except-episodic
  semantic: public      # Skills, knowledge, templates
  templates: public     # Jinja2 skill templates
  manifests: public     # skill.json, SKILL.md, manifest.yaml
  references: public    # Documentation, ADRs, specs
  episodic: private     # Journal entries, operator sessions
```

### Rationale

| Artifact | Visibility | Why |
|---|---|---|
| **Semantic Memory** | `public` | Shareable knowledge — skills, manifests, templates |
| **Templates** | `public` |Reusable patterns for skill creation |
| **Manifests** | `public` | Registry discovery — skill.json, SKILL.md |
| **References** | `public` | Open source documentation |
| **Episodic Memory** | `private` | Instance-specific journal (operator privacy) |

## hKask Artifact Structure

Each Russell skill includes three artifact types:

### 1. Russell Native (`manifest.yaml`)

```yaml
id: okapi-watcher
version: 0.3.0
symptoms: [llm_slow, resource_exhaustion, gpu_fallback_to_cpu]
probes:
  - id: probe-health
    cmd: ["bash", "./scripts/probe-health.sh"]
interventions:
  - id: restart-okapi
    cmd: ["systemctl", "--user", "restart", "okapi"]
    risk: low
    rollback: none_needed
```

### 2. hKask Universal (`SKILL.md`)

```markdown
---
name: okapi-watcher
version: 0.3.0
author: Russell (Jack the Nurse)
license: MIT OR Apache-2.0
symptoms: [llm_slow, resource_exhaustion]
visibility: public
replicant: russell
---

# Okapi Watcher Skill

Monitors local Okapi LLM instance health...
```

### 3. hKask Descriptor (`skill.json`)

```json
{
  "$schema": "https://raw.githubusercontent.com/hashgraph-online/registry-broker-skills/main/schemas/skill.schema.json",
  "name": "okapi-watcher",
  "version": "0.3.0",
  "replicant": {
    "id": "russell",
    "visibility": "public",
    "artifact_type": "semantic_memory",
    "episodic_memory": false
  }
}
```

## Skill Templates

Russell provides Jinja2 templates for generating hKask-compatible skills:

### Template Location

```
skills/templates/
  - russell-skill.yaml.j2  # Russell native manifest
  - SKILL.md.j2            # hKask universal skill
  - skill.json.j2          # hKask descriptor
```

### Usage

```bash
# Generate new skill with hKask artifacts
russell skill create my-skill --template russell-skill.yaml.j2

# Render hKask artifacts
russell skill render --skill my-skill --template SKILL.md.j2
russell skill render --skill my-skill --template skill.json.j2
```

## Publishing to hKask Registry

### 1. Lint Skill Package

```bash
npx @hol-org/registry skills lint --dir ./skills/my-skill
```

### 2. Verify Against Schema

```bash
npx @hol-org/registry skills verify --name "my-skill" --tier basic --account-id 0.0.1234
```

### 3. Publish to Registry

```bash
npx @hol-org/registry skills publish --dir ./skills/my-skill --account-id 0.0.1234
```

### 4. HCS-26 Manifest

For decentralized registry publishing, create `SKILL.manifest.json`:

```json
{
  "name": "my-skill",
  "version": "1.0.0",
  "files": [
    {
      "path": "SKILL.md",
      "hrl": "hcs://1/0.0.44444",
      "sha256": "...",
      "mime": "text/markdown"
    },
    {
      "path": "skill.json",
      "hrl": "hcs://1/0.0.55555",
      "sha256": "...",
      "mime": "application/json"
    },
    {
      "path": "manifest.yaml",
      "hrl": "hcs://1/0.0.66666",
      "sha256": "...",
      "mime": "text/yaml"
    }
  ]
}
```

## Replicant Metadata

All Russell skills include replicant metadata:

```yaml
replicant:
  id: russell
  visibility: public
  artifact_type: semantic_memory
  episodic_memory: false
```

This metadata signals:
- **Replicant ID**: `russell` — part of Russell replicant profile
- **Visibility**: `public` — discoverable in hKask registry
- **Artifact Type**: `semantic_memory` — shareable knowledge (not episodic)
- **Episodic Memory**: `false` — not instance-specific journal data

## Central Registry Files

Russell maintains central hKask compatibility files:

| File | Purpose | Location |
|---|---|---|
| `llms.txt` | Replicant index, artifact URLs | `/llms.txt` |
| `REPLICANT.md` | Replicant profile, visibility model | `/REPLICANT.md` |
| `skills/templates/*.j2` | Skill generation templates | `/skills/templates/` |

## Skills Catalog

Russell ships with 12 public skills (all hKask-compatible):

| Skill | Symptoms | Probes | Interventions | hK Status |
|---|---|---|---|---|
| `okapi-watcher` | 3 | 3 | 1 | ✅ SKILL.md + skill.json |
| `skill-manager` | 6 | 3 | 6 | ✅ SKILL.md + skill.json |
| `skill-workshop` | 2 | 2 | 4 | ✅ SKILL.md + skill.json |
| `skill-maintenance` | 2 | 2 | 2 | ✅ SKILL.md + skill.json |
| `skill-discovery` | 2 | 2 | 3 | ✅ SKILL.md + skill.json |
| `sysadmin` | 3 | 2 | 3 | ✅ SKILL.md + skill.json |
| `scenario-tester` | 1 | 7 | 0 | ✅ SKILL.md + skill.json |
| `journal-compactor` | 1 | 1 | 1 | ✅ SKILL.md + skill.json |
| `pragmatic-cybernetics` | 1 | 0 | 0 | ✅ SKILL.md + skill.json |
| `pragmatic-semantics` | 1 | 0 | 0 | ✅ SKILL.md + skill.json |
| `ubuntu-jack` | 1 | 2 | 1 | ✅ SKILL.md + skill.json |
| `web-search` | 1 | 1 | 0 | ✅ SKILL.md + skill.json |

**Total**: 25 probes, 21 interventions, 30 symptoms

## IDRS Contract

All Russell mutations satisfy the IDRS contract (JR-2, JR-3):

- **I** — Idempotent
- **D** — Dry-run
- **R** — Rollback
- **S** — Structured log

hKask skills inherit this contract via Russell's dispatcher.

## References

- [HCS-26 Standard](https://github.com/hiero-ledger/hiero-consensus-specifications/blob/main/docs/standards/hcs-26.md)
- [OpenClaw Skill Manifest Reference](https://openclawai.me/blog/skill-manifest-reference)
- [SchemaStore #5405](https://github.com/SchemaStore/schemastore/issues/5405)
- [AGENTS.md](../../AGENTS.md) — Russell binding orientation

---

*Russell is a single-host, single-operator harness. He observes, remembers, and reports — never mutates without IDRS contract.*