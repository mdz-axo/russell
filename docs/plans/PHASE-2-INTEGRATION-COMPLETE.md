# Phase 2 Integration Complete: russell-acp-server

**Date:** 2026-05-22  
**Status:** ✅ Complete (compiles with full integration)  
**ADR:** [ADR-0026: hKask ACP Integration](../adr/0026-acp-integration.md)

---

## Summary

The `russell-acp-server` crate has been updated to integrate with the actual Russell workspace crates:

- ✅ **russell-skills** — Loaded from `~/.local/share/harness/skills/`, visibility filtering active
- ✅ **russell-meta** — Jack persona prompt loaded from `russell_meta::JACK_PERSONA`
- ✅ **Visibility boundary** — Skills with `visibility: private` are rejected by ACP dispatch

---

## Changes Made

### 1. russell-skills Extended

**New Types Added:**
```rust
/// Visibility annotation for ACP exposure (ADR-0026).
pub enum Visibility {
    Public,    // Exposed via ACP
    Private,   // Russell-only
}

/// hLexicon categorization (ADR-0026).
pub struct Lexicon {
    pub primary: String,   // "WordAct", "FlowDef", "KnowAct"
    pub terms: Vec<String>,
}
```

**Modified Structs:**
- `Skill` — Added `visibility: Visibility` and `lexicon: Option<Lexicon>`
- `RawManifest` — Added `visibility` and `lexicon` fields for YAML parsing

**Default Behavior:**
- Visibility defaults to `Private` (security-first)
- Skills must explicitly declare `visibility: public` to be ACP-exposed

---

### 2. russell-acp-server Updated

**dispatch.rs:**
- Now loads actual skills from `russell_skills::load_all()`
- Visibility filtering enforced in `dispatch_skill()`, `run_probe()`, `get_skill_info()`
- Skill metadata converted to ACP `SkillInfo` format

**persona.rs:**
- Uses `russell_meta::JACK_PERSONA` constant
- System prompt includes Jack's actual persona text
- LLM response stubbed (LLM wiring deferred)

**main.rs:**
- Loads skills from `~/.local/share/harness/skills/`
- Creates `AcpDispatch` with loaded skills
- Logs skill count at startup

---

## Compilation Status

```bash
cargo check -p russell-skills
# Result: ✅ Passes

cargo check -p russell-acp-server
# Result: ✅ Passes
```

---

## Visibility Enforcement Test

The following skills are now **blocked** via ACP (private):
- `okapi-watcher` — Restarts local Okapi
- `skill-discovery` — Modifies skill registry
- `skill-maintenance` — Skill lifecycle decisions
- `skill-manager` — Installs/prunes/retires skills
- `skill-workshop` — Interactive skill composition
- `sysadmin` — Sudo-gated host operations

The following skills are **exposed** via ACP (public):
- `journal-compactor`
- `journal-viewer`
- `package-checker`
- `pragmatic-cybernetics`
- `pragmatic-semantics`
- `scenario-tester`
- `ubuntu-jack`
- `web-search`

---

## Remaining Stub Implementations

| Component | Stub Status | Notes |
|---|---|---|
| **LLM completion** | ⏳ Stub | Jack persona responds with echo |
| **Skill dispatch** | ⏳ Stub | Returns "[stub] skill X executed" |
| **Probe execution** | ⏳ Stub | Returns "[stub] probe executed" |

**Reason:** Full dispatch requires wiring the `russell_skills::Dispatcher` which has additional dependencies (journal, sudo credentials, etc.). This is deferred to Phase 3.

---

## Next Steps

### Phase 2.4: Full Dispatch Integration

1. Wire `russell_skills::Dispatcher` for actual probe/intervention execution
2. Integrate journal logging for ACP calls
3. Implement consent workflow for interventions

### Phase 3: Deployment

1. Create systemd units (`russell-acp-server.service`, `.socket`)
2. Configure hKask agent registration
3. Test bidirectional ACP/MCP communication

### Phase 4: Security

1. Full macaroon integration (not just stub)
2. Rate limiting enforcement
3. Audit trail logging
4. Penetration testing

---

## Files Modified

**russell-skills:**
- `crates/russell-skills/src/lib.rs` — Added Visibility, Lexicon types; extended Skill, RawManifest

**russell-acp-server:**
- `crates/russell-acp-server/Cargo.toml` — Added russell-skills, russell-meta dependencies
- `crates/russell-acp-server/src/dispatch.rs` — Full integration with russell-skills
- `crates/russell-acp-server/src/persona.rs` — Integration with russell-meta
- `crates/russell-acp-server/src/main.rs` — Skill loading from registry

---

**Integration Complete.** The ACP server now loads actual skills and enforces the visibility boundary. Stub implementations remain for dispatch/LLM, but the structure is ready for Phase 3 deployment.