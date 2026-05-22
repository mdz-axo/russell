# Documentation Classification — 2026-05-22 Refresh

**Generated:** 2026-05-22  
**Purpose:** Ground-truth inventory mapping every file in `docs/` to disposition

## Summary

| Metric | Count |
|--------|-------|
| Total markdown files | 128 |
| Active (excluding archive) | 120 |
| In archive | 8 |
| **Retain-and-update** | 67 |
| **Archive** | 41 |
| **Delete** | 12 |

## Classification Table

### Retain-and-Update (67 files)

| File | Rationale |
|------|-----------|
| `docs/README.md` | Portal document — authoritative |
| `docs/USER_GUIDE.md` | Operator guidance — authoritative |
| `AGENTS.md` | Contributor orientation — binding |
| `cybernetic-health-harness.md` | Canonical design document |
| `MACHINE_PROFILE.md` | Observed machine profile |
| `docs/adr/*.md` (17 files) | Architecture decision records — immutable |
| `docs/adr/deferred/*.md` (7 files) | Deferred ADRs — sequencing facts |
| `docs/adr/README.md` | ADR index |
| `docs/architecture/PRINCIPLES_CATALOG.md` | JR-1 through JR-7 — binding |
| `docs/architecture/THE_JACK.md` | Nurse persona spec — binding |
| `docs/architecture/overview.md` | Architecture overview — needs ACP update |
| `docs/architecture/ecosystem-integration.md` | hKask integration — needs verification |
| `docs/architecture/skill-self-management-strategy.md` | Skill lifecycle design — active |
| `docs/specifications/MVP_SPEC.md` | Pinned MVP boundary — authoritative |
| `docs/specifications/PERSISTENCE_CATALOG.md` | Data stores — authoritative |
| `docs/specifications/README.md` | Specifications index |
| `docs/standards/DOCUMENTATION_STANDARDS.md` | Documentation governance — binding |
| `docs/standards/TOGAF_LITE_FOR_OPEN_SOURCE.md` | TOGAF-Lite pattern — binding |
| `docs/standards/WRITING_EXCELLENCE.md` | Writing rubric — binding |
| `docs/standards/VALIDATION_RUBRIC.md` | Validation standard — binding |
| `docs/standards/README.md` | Standards index |
| `docs/standards/adr.md` | ADR standard — binding |
| `docs/standards/agent-operating-rules.md` | Agent rules — binding |
| `docs/standards/coding-rust.md` | Rust coding standard — binding |
| `docs/standards/commits.md` | Commit standard — binding |
| `docs/standards/hkask-integration.md` | hKask integration standard — binding |
| `docs/standards/safety.md` | IDRS contract — binding |
| `docs/standards/skill-building-rules.md` | Skill development — binding |
| `docs/status/CONSOLIDATED-STATUS.md` | Single source of truth — authoritative |
| `docs/status/README.md` | Status index |
| `docs/deployment/INSTALL.md` | Installation guide — authoritative |
| `docs/deployment/QUICKSTART.md` | Quick start — authoritative |
| `docs/deployment/acp-integration.md` | ACP integration — needs verification |
| `docs/operations/INSTALL.md` | Operations install — needs dedup |
| `docs/operations/README.md` | Operations index |
| `docs/operations/REUSE_MANIFEST.md` | Reuse manifest — authoritative |
| `docs/reference/cli.md` | CLI reference — needs ACP context |
| `docs/templates/README.md` | Templates index |
| `docs/templates/adr-template.md` | ADR template — canonical |
| `docs/templates/daily-log.md` | Daily log template — canonical |
| `docs/templates/review-entry.md` | Review template — canonical |
| `docs/templates/soap-bundle.md` | SOAP template — canonical |
| `crates/*/README.md` | Crate documentation |
| `docs/specifications/disk-pkg-hygiene/*.md` (8 files) | Disk/pkg hygiene spec — active work |

### Archive (41 files)

| File | Rationale | Destination |
|------|-----------|-------------|
| `docs/CLEANUP-PLAN.md` | Implementation plan — superseded by this refresh | `archive/2026-05-22-doc-refresh/CLEANUP-PLAN.md` |
| `docs/OKAPI_INTEGRATION_TEST.md` | Test plan — completed, move to archive | `archive/2026-05-22-doc-refresh/` |
| `docs/OKAPI_REFERENCE.md` | Reference doc — absorbed into OTHER docs | `archive/2026-05-22-doc-refresh/` |
| `docs/macaroon-client.md` | Implementation notes — superseded | `archive/2026-05-22-doc-refresh/` |
| `docs/analysis/russell-skill-system-cybernetic-review.md` | Analysis — completed, archive | `archive/2026-05-22-doc-refresh/` |
| `docs/analysis/russell-skill-system-refactoring.md` | Analysis — completed, archive | `archive/2026-05-22-doc-refresh/` |
| `docs/architecture/CAPABILITY_GRAPH.md` | Diagram — verify or archive | `archive/2026-05-22-doc-refresh/` |
| `docs/architecture/CODE_ANCHOR_GRAPH.md` | Diagram — verify or archive | `archive/2026-05-22-doc-refresh/` |
| `docs/architecture/TOGAF_TRACEABILITY_MATRIX.md` | Traceability — consolidate into portal | `archive/2026-05-22-doc-refresh/` |
| `docs/architecture/skill-erd.md` | Skill ERD — verify alignment | `archive/2026-05-22-doc-refresh/` |
| `docs/architecture/skill-friction-analysis.md` | Analysis — completed | `archive/2026-05-22-doc-refresh/` |
| `docs/architecture/skill-health-model.md` | Model — verify or archive | `archive/2026-05-22-doc-refresh/` |
| `docs/architecture/skill-open-questions.md` | Open questions — resolved | `archive/2026-05-22-doc-refresh/` |
| `docs/architecture/skill-ports-adapters.md` | Design doc — verify alignment | `archive/2026-05-22-doc-refresh/` |
| `docs/architecture/skill-sharing.md` | Design — verify or archive | `archive/2026-05-22-doc-refresh/` |
| `docs/plans/ACP-INTEGRATION-SUMMARY.md` | Plan summary — completed | `archive/2026-05-22-doc-refresh/` |
| `docs/plans/ADVERSARIAL-REVIEW-ACTION-PLAN.md` | Action plan — completed | `archive/2026-05-22-doc-refresh/` |
| `docs/plans/PHASE-0.2-SKILL-AUDIT.md` | Phase audit — completed | `archive/2026-05-22-doc-refresh/` |
| `docs/plans/PHASE-0.3-ACP-INTERFACE-DESIGN.md` | Phase design — completed | `archive/2026-05-22-doc-refresh/` |
| `docs/plans/PHASE-1.1-COMPLETE.md` | Phase log — completed | `archive/2026-05-22-doc-refresh/` |
| `docs/plans/PHASE-2-INTEGRATION-COMPLETE.md` | Phase log — completed | `archive/2026-05-22-doc-refresh/` |
| `docs/proposals/russell-kask-integration.md` | Proposal — superseded by ADR-0025/0026 | `archive/2026-05-22-doc-refresh/` |
| `docs/reviews/fixes-summary-2026-05-22.md` | Daily log — archive after 30 days | `archive/2026-05-22-doc-refresh/` |
| `docs/reviews/jack-sovereignty-analysis.md` | Review — completed | `archive/2026-05-22-doc-refresh/` |
| `docs/reviews/skills-cleanup-audit-2026-05-22.md` | Audit — completed, this supersedes | `archive/2026-05-22-doc-refresh/` |
| `docs/operations/CONTAINER_RUNTIME.md` | Operations — verify or consolidate | `archive/2026-05-22-doc-refresh/` |
| `docs/operations/KASK_TOKEN_ROTATION.md` | Token ops — verify or consolidate | `archive/2026-05-22-doc-refresh/` |
| `docs/operations/MCP_ENHANCEMENTS_SUMMARY.md` | Summary — completed | `archive/2026-05-22-doc-refresh/` |
| `docs/operations/MCP_TOOL_CACHE_INVALIDATION.md` | Implementation note — completed | `archive/2026-05-22-doc-refresh/` |
| `docs/operations/RUSSELL_TOKEN_SELF_SERVICE.md` | Token ops — verify or consolidate | `archive/2026-05-22-doc-refresh/` |
| `docs/operations/TOKEN_WIRING_VERIFICATION.md` | Verification — completed | `archive/2026-05-22-doc-refresh/` |
| `docs/status/skill-lifecycle-gaps.md` | Gap analysis — resolved 2026-05-20 | `archive/2026-05-22-doc-refresh/` |
| `docs/standards/CODE_BUDGET_REDUCTION_PHASE1.md` | Budget plan — completed | `archive/2026-05-22-doc-refresh/` |
| `docs/standards/CODE_BUDGET_STATUS.md` | Budget status — consolidate into CONSOLIDATED-STATUS | `archive/2026-05-22-doc-refresh/` |

### Delete (12 files)

| File | Rationale |
|------|-----------|
| `docs/examples/*.md` | Empty directory or placeholder |
| `docs/architecture/README.md` | Minimal content — absorb into docs/README.md |
| `docs/specifications/README.md` | Minimal content — absorb into docs/README.md |
| `docs/deployment/README.md` | Minimal content — absorb into docs/README.md |
| `docs/operations/README.md` | Minimal content — absorb into docs/README.md |
| `docs/reference/README.md` | Minimal content — absorb into docs/README.md |
| `docs/status/README.md` | Minimal content — absorb into docs/README.md |
| `docs/templates/README.md` | Minimal content — absorb into docs/README.md |
| `docs/standards/README.md` | Minimal content — absorb into docs/README.md |
| `docs/adr/README.md` | Minimal content — absorb into docs/README.md |
| `docs/adr/deferred/README.md` | Minimal content — absorb into docs/README.md |
| `docs/archive/README.md` | Minimal content — absorb into docs/README.md |

## Notes

1. **Phase completion logs** (`PHASE-*.md` in `plans/`) are archival candidates — they served their purpose as completion markers but are now historical records. Git history preserves them; active docs should point to `CONSOLIDATED-STATUS.md` for current state.

2. **Analysis documents** in `analysis/` and `reviews/` are working artifacts that served their purpose. Archive them after their findings are incorporated into active architecture documents.

3. **Skill architecture documents** in `architecture/` need verification against current `russell-skills` crate. Those that describe implemented patterns stay; those that describe aspirational patterns archive.

4. **Operations documents** have duplication between `deployment/` and `operations/`. Consolidate into single authoritative location.

5. **Diagram verification** — `CAPABILITY_GRAPH.md` and `CODE_ANCHOR_GRAPH.md` need `DIAGRAM_ALIGNMENT` verification. If stale, archive.
