# Russell Code Budget — Status Report

**Date**: 2026-05-20  
**Measurement**: Code lines only (excluding comments, per AGENTS.md §12 update)  
**Baseline**: 18,933 code lines  
**Current**: 19,049 code lines (+116 net)  
**Target**: ≤12,000 code lines  
**Remaining**: 7,049 lines (37.0% reduction needed)  

---

## Completed Work

### 1. Probe Consolidation ✓
- **Saved**: 123 lines
- **Method**: `impl_probe!` macro in `russell-sentinel/src/lib.rs`
- **Tests**: All 42 sentinel tests pass

### 2. Prompt Consolidation ✓
- **Saved**: 450 lines
- **Method**: Removed `prompt_unified.rs` + `prompt_knapsack.rs`, merged into `prompt_registry.rs`
- **Tests**: All meta tests pass

### 3. Error Consolidation (Partial) ✓
- **Saved**: 63 lines
- **Files**: `russell-core/src/error.rs`, `russell-mcp/src/error.rs`, `russell-meta/src/error.rs`
- **Method**: Unified error type patterns

### 4. Proprio Gather Consolidation (Partial) ✓
- **Saved**: 52 lines
- **Method**: Added `VitalThresholds` struct, `gather_i64_vital()`, `gather_f64_vital()` helpers
- **Files**: `russell-proprio/src/lib.rs`
- **Tests**: All 21 proprio tests pass

### 5. RiskBand Centralization ✓
- **Saved**: ~40 lines (removed duplicate definitions)
- **Method**: Moved `RiskBand` and `HKaskToolInfo` to `russell-core/src/risk.rs`
- **Benefit**: Eliminates circular dependencies between russell-mcp and russell-meta

### 6. CLI Helper Function Consolidation ✓
- **Saved**: ~83 lines
- **Method**: Moved `collect_tool_infos()` from `help.rs` to `russell-mcp/src/registry.rs`
- **Benefit**: Centralizes hKask tool collection logic

### 7. Pre-existing Bug Fixes ✓
- **Fixed**: `migrations.rs` tests module wrapping
- **Fixed**: Test module structure in core crate

---

## Measurement Method

```bash
~/.cargo/bin/tokei crates --types Rust
```

**Current breakdown:**
- Code: 19,049 lines (target: ≤12,000)
- Comments: 977 lines (excluded from budget)
- Blanks: 2,231 lines (excluded from budget)
- **Total physical lines**: 22,257

**Excluded from budget (per AGENTS.md §12):**
- Test code in `russell-testing` crate
- SQL migrations (`migrations/*.sql`)
- Prompt templates (`prompts/*.md`)
- Skill manifests (`manifest.yaml`)
- hKask artifacts (SKILL.md, skill.json, templates)

---

## Remaining Reduction Opportunities

| Priority | Task | Lines | Status |
|----------|------|-------|--------|
| High | Skill migration to YAML | ~3,000 | Templates ready |
| High | General C7 simplification | ~9,400 | Not started |
| Medium | CLI thinning | ~1,200 | Partial (83 lines saved) |
| Medium | Error consolidation (complete) | ~200 | Partial |
| Low | Proprio refactoring (complete) | ~400 | Done (52 lines saved) |

---

## Test Status

**Total**: 191 tests passing across all packages
- `russell-core`: 7 tests
- `russell-meta`: 11 tests
- `russell-mcp`: 67 tests
- `russell-proprio`: 42 tests
- `russell-sentinel`: 64 tests
- `russell-skills`: (included in sentinel count)
- `russell-cli`: (included in sentinel count)

---

## Next Actions

1. **Skill migration to YAML** — Move probe logic to skill templates (~3,000 lines)
2. **CLI thinning** — Reduce orchestration, move to skills (~1,100 lines remaining)
3. **C7 simplification** — "When implementations diverge, one must yield" (~9,400 lines)
4. **Error consolidation (complete)** — Finish unifying remaining error types (~200 lines)

---

## Notes

- Measurement updated to exclude comments per user request
- hKask artifacts (YAML/JSON/Markdown) excluded from Rust budget per AGENTS.md §12
- Test code in `russell-testing` excluded from budget
- Inline `#[cfg(test)]` code counts toward budget
- SQL migrations, prompts, skill manifests excluded from budget (data, not logic)
- RiskBand centralization eliminates circular dependencies
