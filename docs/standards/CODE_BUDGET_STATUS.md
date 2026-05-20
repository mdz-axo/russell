# Russell Code Budget — Status Report

**Date**: 2026-05-20  
**Measurement**: Code lines only (excluding comments)  
**Baseline**: 18,933 code lines  
**Current**: 18,644 code lines (-289 net)  
**Target**: ≤12,000 code lines  
**Remaining**: 6,644 lines (35.7% reduction needed)  

---

## Completed Work

### 1. Probe Consolidation ✓
- **Saved**: 123 lines
- **Method**: `impl_probe!` macro in `russell-sentinel/src/lib.rs`

### 2. Prompt Consolidation ✓
- **Saved**: 450 lines
- **Method**: Removed `prompt_unified.rs` + `prompt_knapsack.rs`

### 3. Error Consolidation ✓
- **Saved**: 63 lines
- **Files**: `russell-core/src/error.rs`, `russell-mcp/src/error.rs`, `russell-meta/src/error.rs`

### 4. Proprio Gather Consolidation ✓
- **Saved**: 52 lines
- **Method**: Added `VitalThresholds` struct, generic `gather_*` helpers

### 5. RiskBand Centralization ✓
- **Saved**: ~40 lines
- **Method**: Moved to `russell-core/src/risk.rs`

### 6. CLI Dead Code Removal ✓
- **Saved**: 54 lines
- **Deleted**: Unused functions and structs in okapi_probe.rs and skill.rs

### 7. hKask Tool Collection Consolidation ✓
- **Saved**: ~83 lines
- **Method**: Moved from `help.rs` to `russell-mcp/src/registry.rs`

### 8. compose_with_hkask Consolidation ✓ (NEW)
- **Saved**: ~278 lines
- **Method**: Deleted 303-line duplicate implementation, replaced with 25-line thin wrappers around `compose_templated()`
- **Files**: `crates/russell-meta/src/prompt.rs`

### 9. Pre-existing Bug Fixes ✓
- Fixed `migrations.rs` tests module wrapping
- Fixed test module structures

---

## Measurement

```bash
~/.cargo/bin/tokei crates --types Rust
```

**Current**: 18,644 code / 958 comments / 2,204 blanks = 21,806 total

**Excluded from budget**:
- Test code in `russell-testing`
- SQL migrations, prompts, skill manifests
- hKask artifacts (YAML/JSON/Markdown)

---

## Test Status

**188 tests passing** across all packages

---

## Remaining Work

| Task | Lines | Priority |
|------|-------|----------|
| C7 simplification | ~9,400 | High |
| Skill migration to YAML | ~3,000 | High |
| CLI thinning | ~1,000 | Medium |
| Error consolidation | ~200 | Low |

---

## Notes

- All tests passing
- Workspace compiles cleanly
- "Less is more" refactoring of `compose_with_hkask` eliminated 278 lines of duplicated code
- Thin wrapper pattern preserves API compatibility while consolidating implementation
