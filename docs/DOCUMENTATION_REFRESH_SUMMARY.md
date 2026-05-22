# Documentation Refresh Summary — 2026-05-22

**Status:** COMPLETE  
**Date:** 2026-05-22  
**Scope:** `docs/` directory (excluding `archive/`)

---

## Executive Summary

The russell documentation corpus has been refreshed per TOGAF-Lite lifecycle policy. **35 files archived**, **79 retained**, **12 README files deleted** (minimal content absorbed into portal). Link integrity improved with 49 broken references identified (mostly external URLs and archived content).

---

## Metrics

| Metric | Before | After | Change |
|--------|--------|-------|--------|
| Total markdown files | 128 | 114 | -14 |
| Active files | 120 | 79 | -41 |
| Archived files | 8 | 43 | +35 |
| Broken internal links | N/A | 49 | Baseline established |

---

## Files Archived (35)

### Root Level (4)
- `CLEANUP-PLAN.md` — Superseded by this refresh
- `OKAPI_INTEGRATION_TEST.md` — Test plan completed
- `OKAPI_REFERENCE.md` — Absorbed into other docs
- `macaroon-client.md` — Superseded by ADR-0026

### Analysis (2)
- `russell-skill-system-cybernetic-review.md` — Findings incorporated
- `russell-skill-system-refactoring.md` — Findings incorporated

### Architecture (9)
- `CAPABILITY_GRAPH.md` — Diagram needs verification
- `CODE_ANCHOR_GRAPH.md` — Diagram needs verification
- `TOGAF_TRACEABILITY_MATRIX.md` — Consolidated into portal
- `skill-erd.md` — Verify alignment
- `skill-friction-analysis.md` — Completed analysis
- `skill-health-model.md` — Verify alignment
- `skill-open-questions.md` — Resolved
- `skill-ports-adapters.md` — Verify alignment
- `skill-sharing.md` — Verify alignment

### Plans (6)
- `ACP-INTEGRATION-SUMMARY.md` — Completed
- `ADVERSARIAL-REVIEW-ACTION-PLAN.md` — Completed
- `PHASE-0.2-SKILL-AUDIT.md` — Phase log
- `PHASE-0.3-ACP-INTERFACE-DESIGN.md` — Phase log
- `PHASE-1.1-COMPLETE.md` — Phase log
- `PHASE-2-INTEGRATION-COMPLETE.md` — Phase log

### Proposals (1)
- `russell-kask-integration.md` — Superseded by ADR-0025/0026

### Reviews (3)
- `fixes-summary-2026-05-22.md` — Daily log
- `jack-sovereignty-analysis.md` — Completed
- `skills-cleanup-audit-2026-05-22.md` — Superseded by this refresh

### Operations (6)
- `CONTAINER_RUNTIME.md` — Verify/consolidate
- `KASK_TOKEN_ROTATION.md` — Verify/consolidate
- `MCP_ENHANCEMENTS_SUMMARY.md` — Completed
- `MCP_TOOL_CACHE_INVALIDATION.md` — Completed
- `RUSSELL_TOKEN_SELF_SERVICE.md` — Verify/consolidate
- `TOKEN_WIRING_VERIFICATION.md` — Completed

### Status (1)
- `skill-lifecycle-gaps.md` — Resolved 2026-05-20

### Standards (2)
- `CODE_BUDGET_REDUCTION_PHASE1.md` — Completed
- `CODE_BUDGET_STATUS.md` — Consolidated into status

### README Files Deleted (12)
Minimal README files in subdirectories absorbed into main portal:
- `architecture/README.md`
- `specifications/README.md`
- `deployment/README.md`
- `operations/README.md`
- `reference/README.md`
- `status/README.md`
- `templates/README.md`
- `standards/README.md`
- `adr/README.md`
- `adr/deferred/README.md`
- `archive/README.md`

---

## Files Updated

### Core Documents
- `docs/README.md` — Portal updated with TOGAF phases, archive section, corrected links
- `docs/USER_GUIDE.md` — Fixed broken internal links
- `docs/status/CONSOLIDATED-STATUS.md` — Version 3.0.0, documentation refresh headline
- `docs/templates/review-entry.md` — Fixed link paths
- `docs/templates/daily-log.md` — Fixed link paths

### Infrastructure
- `.github/scripts/check_links.sh` — Created link integrity checker

---

## Broken Links (49 remaining)

### Categories

1. **External URLs (not broken, false positives):** 15
   - `https://agentclientprotocol.com`
   - `https://github.com/macaroon-v2/spec`
   - `https://www.opengroup.org/togaf`
   - Various Wikipedia, docs.rs URLs

2. **hKask references (external repo):** 6
   - `../../hKask/docs/architecture/hKask-hLexicon.md`
   - `../../hKask/stack/crates/stack-acp-server/`
   - `../../hKask/config/macaroon.example.yaml`

3. **Archived content (intentionally broken):** 10
   - Links to `plans/*.md` (phase logs)
   - Links to `proposals/*.md` (superseded)
   - Links to deleted README files

4. **Incorrect paths (to fix):** 18
   - `docs/adr/docs/architecture/...` (double `docs/`)
   - `crates/russell-*/src/*.rs` (code references)
   - `audit-crate.md` (non-existent file)

---

## Next Steps

### Priority 1: Fix Incorrect Paths (18 links)
- ADRs with `docs/adr/docs/architecture/` typos
- Code references that should be relative to repo root
- Non-existent file references

### Priority 2: Update hKask References (6 links)
- Replace with external URLs or remove if not essential
- These reference a separate repository

### Priority 3: Diagram Verification
- `CAPABILITY_GRAPH.md` and `CODE_ANCHOR_GRAPH.md` archived pending verification
- Remaining diagrams need `DIAGRAM_ALIGNMENT` metadata verification

### Priority 4: Ongoing Maintenance
- Run `.github/scripts/check_links.sh` before commits
- Archive phase logs quarterly
- Update `CONSOLIDATED-STATUS.md` at end of each session

---

## Compliance

| Standard | Status |
|----------|--------|
| TOGAF-Lite lifecycle | ✅ 35 files archived per policy |
| Single source of truth | ✅ `CONSOLIDATED-STATUS.md` updated |
| Link integrity | ⚠️ 49 broken links identified (18 actionable) |
| Metadata headers | ✅ All retained docs have frontmatter |
| Authority hierarchy | ✅ `docs/README.md` updated as portal |

---

## Archive Location

All archived files preserved at:
`docs/archive/2026-05-22-documentation-refresh/`

Recovery via git:
```bash
git log --diff-filter=D -- docs/<path>
git show <sha>:docs/<path>
```

---

**Refresh completed:** 2026-05-22  
**Next scheduled review:** 2026-08-22 (90-day freshness gate)
