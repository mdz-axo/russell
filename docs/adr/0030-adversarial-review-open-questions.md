# ADR-0030: Adversarial Review — Open Architectural Questions

**Status:** Active (tasks complete)  
**Date:** 2026-05-19  
**Last Updated:** 2026-05-20  
**Context:** Multi-perspective adversarial review (T16)

## Completed Tasks

The following tasks from the adversarial review action plan are **complete**:

| Task | Description | Status | Location |
|---|---|---|---|
| **S1** | Capability Attenuation | ✓ Complete | `russell-skills/src/dispatch.rs:426-436` |
| **S3** | Prompt Sanitization Pipeline | ✓ Complete | `russell-meta/src/help.rs` (ACTION detection in `compose_note`) |
| **A1** | Explicit Port Interfaces | ✓ Complete | `russell-core/src/journal/port.rs` (ADR-0033) |
| **P1** | Baseline Freshness Guard | ✓ Complete | `russell-core/src/journal/mod.rs:1423` |
| **C2** | Consent Expiry (5-min TTL) | ✓ Complete | ACP session interface (consent absorbed into ACP protocol) |
| **I2** | Dynamic GPU Detection | ✓ Complete | `russell-sentinel/src/probes/gpu.rs:28-79` |
| **F3** | EWMA Cold Start Acknowledgment | ✓ Complete | `russell-meta/src/help.rs` (SOAP composition) |
| **T3** | Scenario Tests | ✓ Complete | `skills/scenario-tester/scripts/scenario-test-*.sh` |

**Verification:**
- `cargo test --workspace`: 288 tests pass
- `cargo clippy -- -D warnings`: clean
- Scenario tests: `scenario-test-prompt-sanitization.sh`, `scenario-test-capability-attenuation.sh`

## Open Questions

The following design decisions remain open:

### Q1: Journal Port Granularity

Should `JournalWritePort` and `JournalReadPort` be separate traits (as
implemented), or should they be unified into a single `JournalPort` trait?

- **Separate (current):** Enforces OCAP principle — code that only reads
  cannot accidentally write. But adds two type parameters to threading.
- **Unified:** Simpler API surface. Write access implies read access in
  practice (the writer holds the connection).
- **Decision needed:** Which call sites genuinely need read-without-write?

### Q2: Shared Protocol Crate Publishing Model (T9)

Russell and hKask share the same operator and local Okapi backend but
have no shared types. JR-6 mandates "copy-with-provenance" over
dependencies, but this creates drift.

- **Option A:** Standalone `shared-protocol` crate, published to a
  private registry. Both projects depend on it.
- **Option B:** Copy-with-provenance (per JR-6). Manually sync types
  quarterly.
- **Option C:** Russell exposes a well-known MCP tool schema; hKask
  discovers tools at runtime. No shared compile-time types needed.

### Q3: Sandbox Depth for Skill Subprocesses

Current: `env_clear()` + path pinning + `ENV_ALLOWLIST`.

Should Russell adopt `landlock` (Linux LSM) for filesystem sandboxing of
skill subprocesses? Considerations:

- Single-operator threat model: the operator trusts themselves.
- Skills already have trust tiers (T1–T4).
- Landlock requires kernel 5.13+ and adds complexity.
- JR-1 says "when in doubt, cut."

**Recommendation:** Defer unless a multi-operator mode is ever added.

### Q4: Event Chain Bootstrap Seed

The genesis hash is currently `SHA-256(/etc/machine-id)`. This is:
- Deterministic per machine (good for verification)
- Not secret (bad for forgery resistance)

Should the seed include additional entropy? Options:
- Hardware RNG at first journal creation (stored in `baselines` or a
  new table)
- HMAC with a keystore-derived key (hKask alignment)

**Recommendation:** Current approach is sufficient for the single-operator
threat model. Add HMAC if the MCP server ever becomes network-accessible.

### Q5: Consent UX — Retiring Bare "yes"/"ok"

If bare natural-language consent is retired in favor of `/approve` only,
does this break Jack's conversational register? Jack's voice depends on
natural operator interaction.

**Current compromise (T5):** Both forms are accepted; `/approve` is
canonical. Natural-language phrases are whole-line exact matches only.
Expiry (5 min) prevents stale approvals.

**Future:** Consider making natural-language consent opt-in via config
(`consent_mode: strict | conversational`).

### Q6: Custom Probes from Skills

If skills can register custom probes via `ProbeCollector` (T13), what
prevents a malicious skill from registering a probe that exfiltrates data?

Mitigation path:
- Only T4 (operator-authored) skills may register probes.
- Safety scanner checks probe scripts.
- Probes are read-only (no mutations) — exfil requires network access,
  which `env_clear()` and safety scanner address.

### Q7: EWMA Cold Start

Baselines require 30 days of data. During the honeymoon window, should
Jack explicitly acknowledge "I don't have enough history to assess this"
in his SOAP output?

**Current:** `baseline_samples_present` precondition exists but is not
surfaced in the persona. The `BasenelineRow::is_stale()` method (Task 4.1)
provides the freshness check.

**Recommendation:** Add to Jack's prompt template: "If baselines show
fewer than 7 days of data, note 'limited history' in Assessment."

### Q8: Rate Limiter Configuration

The `RateLimiter` defaults to 3 requests/minute. Should this be:
- Configurable via `~/.config/harness/config.toml`?
- Self-tuning based on measured Okapi latency?
- Operator can set `RUSSELL_LLM_RATE_LIMIT=N` env var?

**Recommendation:** Add `llm_rate_limit_per_min` to config.toml. Env var
as override. Self-tuning deferred per JR-1.

### Q9: Full Clock Injection Migration

`Event::new_with_clock()` exists but 30+ call sites still use
`Event::new()` (which calls `SystemClock` internally). Should all call
sites be migrated?

**Recommendation:** Migrate only call sites that are tested or produce
events whose timestamps are asserted on. The sentinel cycle, proprio
cycle, and dispatch paths benefit most. Leave `Event::new()` as the
convenience form for code that doesn't need determinism.

### Q10: Reflex Budget Persistence

Currently `ReflexBudget` is reconstructed fresh each `sentinel-once`
invocation (no memory across invocations). The budget can't actually
prevent >5 interventions/hour because each invocation starts at 0.

Should the budget state be persisted (journal query: "count
`reflex_proposed` events in last hour") or held in a long-lived process?

**Recommendation:** Query the journal for `reflex_proposed` events in
the last hour at budget construction time. This makes the budget
effective across timer-driven invocations without requiring a daemon.

## Decision

Record these questions. Implement Q10 (journal-backed budget) as the
next discrete task. Others await operator input or hKask coordination.

## Consequences

- Open questions are tracked and won't be accidentally revisited.
- No premature decisions on multi-project coordination (Q2, Q4).
- The review's deferred items have a clear home for future work.
