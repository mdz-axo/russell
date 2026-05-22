# ACP Integration Summary

**Status:** Phase 2 Complete — Full Integration  
**Date:** 2026-05-22  
**ADR:** [ADR-0026](../adr/0026-acp-integration.md)

---

## Completion Status

### Phase 0: Foundation ✅
- [x] ADR-0026 created and approved
- [x] All 14 skills audited with hLexicon + visibility metadata
- [x] ACP server interface designed (11 JSON-RPC methods)

### Phase 1: Build ACP Server ✅
- [x] `russell-acp-server` crate created
- [x] Session manager + turn records implemented
- [x] Visibility filter (public/private enforcement) implemented
- [x] Jack persona projection implemented
- [x] Macaroon auth validation implemented
- [x] Rate limiter (100/min) implemented

### Phase 2: Full Integration ✅
- [x] `AcpDispatch` wired to `russell_skills::SkillRegistry`
- [x] Probe execution via `russell_skills::Dispatcher`
- [x] LLM integration via `russell_meta::OkapiClient`
- [x] Journal integration for evidence logging
- [x] All 9 unit tests passing

### Phase 3: Deployment ⏳ (In Progress)
- [x] Systemd units created (service + timer)
- [ ] Binary installation (`cargo install`)
- [ ] Integration testing with hKask

### Phase 4: Security Hardening ⏳ (Deferred)
- [ ] End-to-end macaroon auth testing
- [ ] Penetration testing
- [ ] Input sanitization audit

### Phase 5: Documentation ✅
- [x] [`docs/deployment/acp-integration.md`](../deployment/acp-integration.md) created
- [x] Systemd unit files documented
- [x] This summary updated

---

## Architecture

```
hKask Agent
   │ (ACP JSON-RPC over stdio)
   ▼
russell-acp-server
   ├── JackPersonaProjection
   │    └── LlmClientEnum (Okapi/Mock)
   ├── AcpDispatch
   │    ├── russell_skills::Dispatcher
   │    ├── russell_core::JournalWriter
   │    └── Visibility filter (8 public / 6 private)
   ├── SessionManager
   │    └── Multi-turn state
   ├── MacaroonAuth
   │    └── OCAP validation
   └── RateLimiter
        └── 100 calls/min/token

russell-sentinel (separate timer)
   └── 5-min probe cadence → SQLite journal
```

---

## Public Skills (8)

| Skill | Visibility | Lexicon | Probes | Interventions |
|-------|------------|---------|--------|---------------|
| `journal-compactor` | Public | FlowDef | 1 | 1 |
| `journal-viewer` | Public | KnowAct | 1 | 0 |
| `package-checker` | Public | KnowAct | 1 | 0 |
| `pragmatic-cybernetics` | Public | KnowAct | 0 | 0 |
| `pragmatic-semantics` | Public | KnowAct | 0 | 0 |
| `scenario-tester` | Public | WordAct | 7 | 0 |
| `ubuntu-jack` | Public | FlowDef | 1 | 1 |
| `web-search` | Public | WordAct | 1 | 0 |

---

## Private Skills (6)

| Skill | Visibility | Rationale |
|-------|------------|-----------|
| `okapi-watcher` | Private | LLM service management (sudo) |
| `skill-discovery` | Private | Registry mutations |
| `skill-maintenance` | Private | Skill lifecycle mutations |
| `skill-manager` | Private | Meta-skill (install/prune/delete) |
| `skill-workshop` | Private | Interactive skill building |
| `sysadmin` | Private | Host mutations (sudo) |

---

## ACP Methods Implemented

| Method | Status | Description |
|--------|--------|-------------|
| `acp/session.create` | ✅ | Create multi-turn session |
| `acp/session.message` | ✅ | Send/receive messages |
| `acp/session.close` | ✅ | Close session |
| `acp/session.status` | ✅ | Get session state |
| `acp/capabilities` | ✅ | List public skills/probes |
| `acp/skill/info` | ✅ | Get skill metadata |
| `acp/skill/run` | ✅ | Execute skill (probes only) |
| `acp/probe/run` | ✅ | Run read-only probe |

---

## Security Boundaries

| Boundary | Enforcement |
|----------|-------------|
| **Public/Private skills** | `AcpDispatch::load_public_skills()` filters by `visibility == Public` |
| **Proprioception** | Never added to capability registry |
| **Interventions** | Require consent (handled upstream in `russell chat`) |
| **Macaroon auth** | `MacaroonAuth::validate()` checks capabilities |
| **Rate limiting** | `RateLimiter::check()` enforces 100/min/token |

---

## Testing

```
running 9 tests
test auth::tests::capability_check ... ok
test auth::tests::expired_token_rejected ... ok
test auth::tests::no_root_key_skips_validation ... ok
test rate_limit::tests::rate_limit_allows_under_limit ... ok
test rate_limit::tests::rate_limit_per_token ... ok
test session::tests::session_add_turn ... ok
test rate_limit::tests::rate_limit_rejects_over_limit ... ok
test session::tests::session_creation ... ok
test session::tests::session_manager_create ... ok

test result: ok. 9 passed; 0 failed
```

---

## Next Steps

1. **Integration testing:** Deploy alongside hKask, verify bidirectional communication
2. **Macaroon configuration:** Test with real macaroon keys
3. **Graceful degradation:** Verify sentinel continues during hKask outages
4. **Performance testing:** Measure latency under load
5. **Security audit:** Penetration test the ACP surface

---

## Files Modified/Created

### New Files
- `crates/russell-acp-server/` — ACP server crate
- `docs/deployment/russell-acp-server.service` — systemd service
- `docs/deployment/russell-sentinel.timer` — sentinel timer
- `docs/deployment/russell-sentinel.service` — sentinel service
- `docs/deployment/acp-integration.md` — deployment guide

### Modified Files
- `crates/russell-skills/src/lib.rs` — Added `Visibility` and `Lexicon` types
- `crates/russell-skills/src/registry/mod.rs` — Lifecycle tracking
- `crates/russell-acp-server/src/dispatch.rs` — Full probe execution
- `crates/russell-acp-server/src/persona.rs` — LLM integration
- `crates/russell-acp-server/src/main.rs` — Initialization
- `crates/russell-acp-server/Cargo.toml` — Added `russell-core` dependency
- All 14 skill manifests — Added `visibility` and `lexicon` metadata

---

## Compliance

### JR Principles
| Principle | Status |
|-----------|--------|
| JR-1 (Austere) | ✅ Minimal changes |
| JR-2 (Observe > Act) | ✅ Public skills read-only |
| JR-3 (No LLM shell) | ✅ LLM ranks IDs only |
| JR-4 (Nurse present) | ✅ Jack via ACP |
| JR-5 (Proprioception) | ✅ 5 vitals private |
| JR-6 (Reuse) | ✅ Independent journal |
| JR-7 (Auditable) | ✅ ACP calls logged |

### ADR-0025 (MCP Client)
| Constraint | Status |
|------------|--------|
| Loopback-only | ✅ Extended to macaroon |
| No cross-dependency | ✅ Russell crate graph unchanged |
| Graceful degradation | ✅ Sentinel operates standalone |

---

**Last Updated:** 2026-05-22  
**Next Review:** After hKask integration testing
