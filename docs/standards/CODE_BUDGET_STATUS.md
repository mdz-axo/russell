# Russell Code Budget — Status Report

**Date**: 2026-05-19  
**Baseline**: 26,426 lines Rust  
**Current**: 26,730 lines Rust (+304 net)  
**Target**: ≤12,000 lines Rust  
**Remaining**: 14,730 lines (55% reduction needed)  

---

## Completed Work (Phase 1)

### 1. Probe Consolidation ✓
- **Saved**: 123 lines
- **Method**: `impl_probe!` macro in `russell-sentinel/src/lib.rs`
- **Tests**: All 42 sentinel tests pass

### 2. Prompt Consolidation ✓
- **Saved**: 450 lines
- **Method**: Removed `prompt_unified.rs` + `prompt_knapsack.rs`, merged into `prompt_registry.rs`
- **Tests**: All meta tests pass

### 3. hKask Replicant Integration ✓
- **Artifacts**: 26 files (SKILL.md + skill.json for 12 skills, 3 templates, 4 docs)
- **Visibility**: public-except-episodic
- **Status**: Complete

### 4. CLI Utilities Module ✓
- **Added**: 73 lines
- **Functions**: 8 shared utilities
- **Purpose**: Reduce duplication across CLI commands

---

## Remaining Reduction Opportunities

| Priority | Task | Lines | Status |
|----------|------|-------|--------|
| High | Skill migration to YAML | ~3,000 | Templates ready |
| High | General C7 simplification | ~9,400 | Not started |
| Medium | CLI thinning | ~1,200 | Utils created |
| Medium | Error consolidation | ~500 | Not started |
| Low | Proprio refactoring | ~400 | Deferred |

---

## Test Status

**Total**: 295 tests passing across all packages
- `russell-core`: 7 tests
- `russell-meta`: 67 tests
- `russell-mcp`: 23 tests
- `russell-proprio`: 67 tests
- `russell-sentinel`: 42 tests
- `russell-skills`: 21 tests
- `russell-cli`: 64 tests
- `russell-testing`: 4 tests

---

## Next Actions

1. **Error consolidation** — Unify error types across core/mcp/meta (~500 lines)
2. **Skill migration to YAML** — Move probe logic to skill templates (~3,000 lines)
3. **CLI thinning** — Reduce orchestration, move to skills (~1,200 lines)
4. **Proprio refactoring** — Generic `gather_vital()` function (~400 lines)
5. **C7 simplification** — "When implementations diverge, one must yield" (~9,400 lines)

---

## Notes

- hKask artifacts (YAML/JSON/Markdown) excluded from Rust budget per AGENTS.md §12
- Test code in `russell-testing` excluded from budget
- Inline `#[cfg(test)]` code counts toward budget
- SQL migrations, prompts, skill manifests excluded from budget
