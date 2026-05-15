---
title: "ADR-0024: Skill Registry, Workshop, and Lifecycle — Discovery-to-Retirement Pipeline"
audience: [developers, architects]
last_updated: 2026-05-13
togaf_phase: "H"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Data Architecture / Application Architecture -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Accepted -->
<!-- LAST_UPDATED: 2026-05-13 -->

# ADR-0024: Skill Registry, Workshop, and Lifecycle — Discovery-to-Retirement Pipeline

- **Status:** Accepted
- **Date:** 2026-05-13
- **Deciders:** Project operator
- **Tags:** `skills`, `registry`, `workshop`, `lifecycle`, `phase-4`

## Context

Russell's skill system (ADR-0023, Phase 3) supports loading validated
YAML manifest skills from `~/.local/share/harness/skills/` with
poka-yoke enforcement. Skills are manually installed by copying
directories. The web-search and skill-discovery knowledge skills
(2026-05-13) teach Jack how to use MCP tools to search for skills
on the web, but Russell has no:

1. **Registry** — a canonical index mapping symptoms → skills.
   Jack can describe a problem but has no lookup table to answer
   "which of my installed skills addresses `vram_oom`?" without
   full-text scanning every manifest.

2. **Workshop** — an interactive mode where the operator and Jack
   collaborate to discover, evaluate, build, adapt, and maintain
   skills. Currently, skill creation is a manual `mkdir` + `vim`
   + `cargo build` loop.

3. **Lifecycle** — formal states (discovered → installed → active
   → stale → deprecated → retired) with automated staleness
   detection and evaluation prompts.

4. **Cache** — a local metadata store for discovered-but-not-yet-
   installed skills, avoiding repeated web searches.

OpenClaw's patterns (ClawHub registry, Skill Workshop plugin,
security scanner, versioning, hot reload) demonstrate that a
registry + workshop pipeline transforms skills from static config
into a living capability catalogue. Russell should adopt the
lightweight subset appropriate to its scope.

## Decision

### 1. Registry: local lookup table + remote sources

**Registry file:** `~/.local/share/harness/registry/local-cache.yaml`

```yaml
# Local cache of known skills — both installed and discovered.
# The registry is the authoritative lookup: symptom → skill.
skills:
  okapi-watcher:
    status: active
    version: 0.1.0
    symptoms: [llm_slow, resource_exhaustion, gpu_fallback_to_cpu]
    source: bundled
    installed: 2026-05-09
    last_evaluated: 2026-05-13
    coverage_score: 0.85
  web-search:
    status: active
    version: 1.0.0
    symptoms: [search_capability_needed, web_knowledge_gap, ...]
    source: bundled
    installed: 2026-05-13
```

**Key design decision:** The registry is rebuildable from
`~/.local/share/harness/skills/` (JR-7 — journal is canonical,
derived stores are rebuildable). The cache adds metadata
(status, scores, evaluation dates) not present in manifests.

**Lookup functions:**
- `symptom → [skill]` — which skills address this symptom?
- `skill → symptoms` — what does this skill cover?
- `gap analysis` — which catalogued symptoms have no installed skill?

**Remote sources:** Configured in `~/.config/harness/registry-sources.yaml`:

```yaml
sources:
  - name: russell-official
    url: https://github.com/axolotl/russell-skills
    kind: github-repo
  - name: community
    url: https://github.com/operator/russell-custom-skills
    kind: github-repo
```

Remote sources are queried via the web-search MCP bridge when the
operator runs `russell skill search <query>`.

### 2. Workshop: interactive skill lifecycle REPL

**Command:** `russell workshop`

A readline REPL where Jack (the Nurse) helps the operator with the
full skill lifecycle. Jack's knowledge of the lifecycle comes from
two new knowledge skills: `skill-workshop` (bootstrap + compose) and
`skill-maintenance` (evaluate, staleness, lifecycle transitions).

**Workshop modes:**

| Command | Phase | Description |
|---|---|---|
| `search <query>` | Discover | Search registry + remote sources for skills |
| `fetch <slug>` | Discover | Download a skill manifest from a remote source |
| `evaluate <slug>` | Evaluate | Show manifest, scripts diff, safety scan |
| `build <name>` | Compose | Interactive skill creation with Jack |
| `adapt <slug>` | Adapt | Modify an existing skill interactively |
| `check` | Maintain | Audit all installed skills: staleness, gaps |
| `prune <slug>` | Retire | Deprecate or remove a skill |
| `/list` | — | Show all skills with lifecycle status |
| `/gaps` | — | Show symptoms with no installed skill |
| `/lookup <symptom>` | — | Which skills address this symptom? |

**State machine:**
```
  discovered → evaluated → installed → active → stale_warning → deprecated → retired
       ↑            ↑          ↑          ↑           ↑              ↑
       |____________|__________|__________|___________|______________|
            (any phase can loop back to evaluate)
```

- **discovered** — found via search, not yet inspected
- **evaluated** — manifest reviewed, safety scanned, pending install decision
- **installed** — copied to `~/.local/share/harness/skills/`, poka-yoke passed
- **active** — loaded by harness, used in Jack sessions
- **stale_warning** — `authored` date > 6mo, or `valid_until` passed
- **deprecated** — superseded or no longer relevant, still on disk
- **retired** — removed from skills directory, kept in registry as historical

### 3. Safety scanner (IDRS applied to skill ingestion)

Before any skill enters `installed` state, a safety scanner checks:
- **Prompt injection** — `ignore prior instructions`, `you are now`, `SYSTEM:`
- **Shell pipe attacks** — `curl.*|.*(sh|bash)`, `wget.*|.*(sh|bash)`
- **Secret exfiltration** — `curl.*(env|/etc)`, `nc.*(env|/etc)`
- **Destructive commands** — `rm -rf /`, `chmod 777`, `mkfs`, `dd if=`
- **Poka-yoke** — existing validation (symptoms, rollback, scripts)

Findings are classified `info` / `warn` / `block`. `block` findings
prevent installation until operator overrides.

### 4. Knowledge skills

Two new knowledge-only skills (no probes, no interventions) teach Jack
the workshop lifecycle:

- **`skill-workshop`** — bootstrap/compose mode: how to search for skills,
  evaluate manifests, write probe scripts interactively, safety scan rules,
  the creation template.

- **`skill-maintenance`** — evaluate/maintain mode: how to audit installed
  skills for staleness (`authored` age, `valid_until`), detect coverage
  gaps (catalogued symptoms without skills), flag unused symptoms, score
  skill quality, guide deprecation/retirement.

These are loaded as KNOWLEDGE.md content into Jack's system prompt when
the workshop REPL session starts.

## Consequences

### Positive

- **Skill selection becomes a database operation** — symptom → skill lookup
  replaces manual scanning, making Jack more effective at proposing probes.
- **Skill creation becomes collaborative** — the operator describes what
  they need, Jack proposes a manifest, iterates on probe scripts, validates,
  and installs — all in one session.
- **Skills don't rot silently** — staleness detection surfaces out-of-date
  skills before they give wrong advice.
- **Coverage becomes measurable** — gap analysis tells the operator which
  catalogued symptoms have no installed skill, making capability gaps
  explicit.

### Negative

- **Registry adds state** — `local-cache.yaml` is derived, not canonical
  (JR-7: rebuildable from journal + manifests). Must stay rebuildable.
- **Workshop REPL adds surface area** — another CLI mode, another prompt
  shape, more LLM context budget. Mitigated by loading only the relevant
  knowledge skills.
- **Safety scanner adds complexity** — regex-based scanning is imperfect but
  catches the most common classes of malicious skill content.

### Neutral

- **Remote registry is optional** — `russell skill search` falls back to
  local-only when no sources are configured or the web search MCP bridge
  is unavailable.
- **Workshop is opt-in** — `russell skill list` and `russell skill run`
  continue to work without the registry or workshop.

## Implementation Notes

- The registry cache is `serde_yaml` serialized, matching the existing
  manifest parsing pattern.
- The workshop REPL reuses the `russell chat` readline and LLM dispatch
  infrastructure (russell-doctor).
- The safety scanner is a pure function: `scan(content: &str) -> Vec<Finding>`.
- Knowledge skills follow the existing pattern (manifest.yaml + KNOWLEDGE.md,
  no probes, no interventions, loaded at session start).
- Remote source fetching uses the web-search MCP bridge when available,
  falling back to `curl` subprocess for HTTP fetches.

## References

- ADR-0007: YAML Manifest Subprocess Skill Model
- ADR-0023: Lift ADR-0007 — Phase 3 Skills and Dispatch
- `skills/web-search/KNOWLEDGE.md` — MCP bridge protocol
- `skills/skill-discovery/KNOWLEDGE.md` — skill lifecycle guide
- OpenClaw ClawHub: https://github.com/openclaw/clawhub
- OpenClaw Skill Workshop: https://docs.openclaw.ai/plugins/skill-workshop
