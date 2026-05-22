# Phase 0.2: Skill Audit Report — hLexicon + Visibility

**Date:** 2026-05-22  
**Status:** ✅ Complete  
**ADR:** [ADR-0026: hKask ACP Integration](../adr/0026-acp-integration.md)

---

## Summary

All 14 Russell skills have been audited and updated with:
1. **hLexicon categorization** (WordAct/FlowDef/KnowAct domain + specific terms)
2. **Visibility annotation** (public/private for ACP exposure boundary)

---

## Audit Results

| Skill | Visibility | Lexicon Domain | Lexicon Terms | ACP Exposure |
|-------|------------|----------------|---------------|--------------|
| **journal-compactor** | public | FlowDef | `sequence, transform, schedule, filter, expire` | ✅ Exposed |
| **journal-viewer** | public | FlowDef | `sequence, filter, route` | ✅ Exposed |
| **okapi-watcher** | private | FlowDef | `detect, escalate, compensate, timeout, fallback` | ❌ Local |
| **package-checker** | public | KnowAct | `detect, classify, report, monitor` | ✅ Exposed |
| **pragmatic-cybernetics** | public | KnowAct | `recognize, classify, detect, monitor, evaluate, reflect, ground` | ✅ Exposed |
| **pragmatic-semantics** | public | KnowAct | `parse, classify, infer, discriminate, ground` | ✅ Exposed |
| **scenario-tester** | public | FlowDef | `sequence, parallel, iteration, evaluate, calibrate, report` | ✅ Exposed |
| **skill-discovery** | private | FlowDef | `sequence, filter, route, match, acquire` | ❌ Local |
| **skill-maintenance** | private | FlowDef | `schedule, expire, monitor, evaluate, detect` | ❌ Local |
| **skill-manager** | private | FlowDef | `compose, create, abolish, sequence, compensate, merge, prune` | ❌ Local |
| **skill-workshop** | private | WordAct | `query, prompt, instruct, propose, collaborate` | ❌ Local |
| **sysadmin** | private | FlowDef | `command, escalate, abort, sequence, compensate, catch, fallback` | ❌ Local |
| **ubuntu-jack** | public | WordAct | `assert, report, acknowledge, declare` | ✅ Exposed |
| **web-search** | public | WordAct | `query, probe, report, summon, challenge` | ✅ Exposed |

---

## Visibility Distribution

| Visibility | Count | Skills |
|------------|-------|--------|
| **Public** (ACP-exposed) | 8 | journal-compactor, journal-viewer, package-checker, pragmatic-cybernetics, pragmatic-semantics, scenario-tester, ubuntu-jack, web-search |
| **Private** (Russell-only) | 6 | okapi-watcher, skill-discovery, skill-maintenance, skill-manager, skill-workshop, sysadmin |

**Note:** Earlier analysis estimated 8 public / 8 private. Actual audit shows 8 public / 6 private = 14 total skills.

---

## Lexicon Domain Distribution

| Domain | Count | Skills |
|--------|-------|--------|
| **WordAct** (LLM prompting) | 3 | skill-workshop, ubuntu-jack, web-search |
| **FlowDef** (workflow patterns) | 8 | journal-compactor, journal-viewer, okapi-watcher, scenario-tester, skill-discovery, skill-maintenance, skill-manager, sysadmin |
| **KnowAct** (cognition) | 3 | package-checker, pragmatic-cybernetics, pragmatic-semantics |

---

## Security Boundary Rationale

### Public Skills (8) — ACP Exposed

These skills are **read-only or informational** — no host mutation risk:

| Skill | Why Public |
|-------|------------|
| journal-compactor | Journal maintenance (VACUUM, prune) — Russell's own persistence |
| journal-viewer | Read-only journal queries |
| package-checker | Package audit (read-only); updates require consent but are informational |
| pragmatic-cybernetics | Knowledge reference (no probes/interventions) |
| pragmatic-semantics | Knowledge reference (no probes/interventions) |
| scenario-tester | Testing framework — read-only observation |
| ubuntu-jack | Knowledge reference (no probes/interventions) |
| web-search | Read-only web queries via MCP |

### Private Skills (6) — Russell-Only

These skills involve **host mutations, sudo operations, or skill registry changes**:

| Skill | Why Private |
|-------|-------------|
| okapi-watcher | Restarts local Okapi (systemd mutation) |
| skill-discovery | Modifies skill registry (downloads, installs) |
| skill-maintenance | Skill lifecycle decisions (deprecation, retirement) |
| skill-manager | Skill registry mutations (build, install, prune, retire) |
| skill-workshop | Interactive skill composition (creates/modifies skills) |
| sysadmin | Sudo-gated host operations (systemd, clock sync, zombie reap, swap flush) |

---

## Manifest Changes

Each skill manifest now includes:

```yaml
# hLexicon categorization (ADR-0026)
visibility: public | private
lexicon:
  primary: WordAct | FlowDef | KnowAct
  terms: [term1, term2, ...]  # 3-7 terms from hLexicon
```

**Example:**
```yaml
id: web-search
visibility: public
lexicon:
  primary: WordAct
  terms: [query, probe, report, summon, challenge]
```

---

## ACP Exposure Enforcement

The `russell-acp-server` crate will implement visibility filtering:

```rust
impl AcpHandler {
    fn load_public_skills(&self) -> Vec<Skill> {
        self.skill_registry
            .all_skills()
            .filter(|s| s.visibility == Visibility::Public)
            .collect()
    }

    async fn dispatch_skill(&self, id: &str, args: &Value) -> Result<String> {
        let skill = self.get_skill(id)?;
        if skill.visibility == Visibility::Private {
            return Err(AcpError::SkillNotExposed(id.to_string()));
        }
        self.dispatcher.run(id, args).await
    }
}
```

**Error response for private skill via ACP:**
```json
{
  "error": "SkillNotExposed",
  "message": "skill 'okapi-watcher' is private and not exposed via ACP",
  "visibility": "private"
}
```

---

## Next Steps

1. **Phase 0.3:** Design ACP server interface (methods, types, error taxonomy)
2. **Phase 1.1:** Create `russell-acp-server` crate
3. **Phase 1.3:** Implement visibility filter in ACP dispatch layer

---

## Files Modified

All 14 skill manifests updated:

```
~/.local/share/harness/skills/
  journal-compactor/manifest.yaml    ✅
  journal-viewer/manifest.yaml       ✅
  okapi-watcher/manifest.yaml        ✅
  package-checker/manifest.yaml      ✅
  pragmatic-cybernetics/manifest.yaml ✅
  pragmatic-semantics/manifest.yaml  ✅
  scenario-tester/manifest.yaml      ✅
  skill-discovery/manifest.yaml      ✅
  skill-maintenance/manifest.yaml    ✅
  skill-manager/manifest.yaml        ✅
  skill-workshop/manifest.yaml       ✅
  sysadmin/manifest.yaml             ✅
  ubuntu-jack/manifest.yaml          ✅
  web-search/manifest.yaml           ✅
```

---

**Audit Complete.** Ready for Phase 0.3 (ACP interface design).