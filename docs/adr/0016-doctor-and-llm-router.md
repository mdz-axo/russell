---
title: "ADR-0016: MVP Doctor — Single Round-Trip to a ZDR Frontier LLM"
audience: [developers, architects, agents]
last_updated: 2026-04-18
togaf_phase: "H — Change Management"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Change Management -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-04-18 -->

# ADR-0016: MVP Doctor — Single Round-Trip to a ZDR Frontier LLM

- **Status:** Accepted
- **Date:** 2026-04-18
- **Deciders:** Project founders
- **Tags:** `doctor`, `llm`, `openrouter`, `zdr`, `mvp`
- **Supersedes:** *(none)*
- **Relates to:** ADR-0008 (LLM never emits shell)

## Context

Under JR-4 (small but present: the Doctor), Russell must be able
to **cry for help** from day one. At the same time, JR-1
(austerity) forbids building the full LLM-assisted triage loop
described in [`cybernetic-health-harness.md` §12.2](../../cybernetic-health-harness.md),
which requires skill manifests, dispatchers, and a differential
ranking protocol — all deferred under ADR-0007.

The MVP question: **what is the smallest Doctor that is
architecturally faithful to the full design?** The answer must
preserve ADR-0008's hard rule (the LLM never emits shell), must
not compromise the IDRS posture (JR-2), and must leave a clean
seam for the richer triage loop when ADR-0007 lifts.

Simultaneously, operator data privacy rules out sending evidence
to providers who may log prompts. OpenRouter's per-request `zdr:
true` parameter, combined with Kimi K2's zero-retention endpoint,
solves this while giving us access to a frontier open-weight
reasoning model (1T params, 32B active, 131k-262k context).

## Decision

### 1. MVP Doctor is a single verb

`russell help [--note "..."]` is the only Doctor-facing CLI verb
in MVP. Its behaviour:

1. **Gather**: last 24 hours of samples, last 20 events, current
   profile summary, last Sentinel cycle age. All read-only.
2. **Compose** a SOAP-shaped prompt
   ([`../templates/soap-bundle.md`](../templates/soap-bundle.md))
   with:
   - **Subjective** ← operator's `--note` (optional).
   - **Objective** ← gathered evidence.
   - **Assessment, Plan** ← left empty for the LLM to fill.
3. **Call** the LLM via a single round-trip (no streaming, no
   retry on semantic errors, no tool use).
4. **Journal** the full exchange.
5. **Print** the model's response to stdout, verbatim.

The Doctor does **not**:

- Parse the model's output for commands.
- Execute anything the model says.
- Retry with a modified prompt.
- Maintain conversational state across invocations.
- Use tool-calling, function-calling, or any protocol that
  could construct an executable action.

### 2. Backend selection via env

Configuration flows from `~/.config/harness/russell.env` (see
[`../specifications/PERSISTENCE_CATALOG.md`](../specifications/PERSISTENCE_CATALOG.md) §2.5):

```
OPENROUTER_API_KEY=sk-or-...
RUSSELL_DOCTOR_BACKEND=openrouter   # openrouter | ollama | mock
RUSSELL_DOCTOR_MODEL=moonshotai/kimi-k2.5
```

Selection logic at startup:

1. If `RUSSELL_DOCTOR_BACKEND` is set, use it.
2. Else if `OPENROUTER_API_KEY` is present, default to `openrouter`.
3. Else, default to `mock` (which produces the offline-fallback
   response — see §4).

### 3. OpenRouter backend with ZDR enforced

The OpenRouter backend:

- **Base URL:** `https://openrouter.ai/api/v1` (overridable for
  testing only).
- **Default model:** `moonshotai/kimi-k2.5`.
- **Timeout:** 60 seconds. One attempt. No retry.
- **Provider preferences sent in every request:**

  ```json
  "provider": { "zdr": true, "data_collection": "deny" }
  ```

  This enforces zero data retention at the per-request level
  (see `openrouter.ai/docs/guides/features/zdr`). A request that
  cannot be routed to a ZDR endpoint fails rather than silently
  routing to a retaining provider.

- **Identification:** `HTTP-Referer` set to a constant
  `https://russell.local/` and `X-Title` to `"Russell"`, per
  OpenRouter's app-attribution conventions.

### 4. Offline fallback is mandatory

If any of these is true, `russell help` emits the **rule-based
fallback** instead of calling the LLM:

- `OPENROUTER_API_KEY` is unset AND `RUSSELL_DOCTOR_BACKEND` is
  not explicitly `ollama` or `mock`.
- The HTTP call fails (network, timeout, 5xx).
- The response body does not parse as a valid OpenAI-compatible
  chat completion.

The fallback reads the same SOAP bundle and produces a short,
deterministic, Jack-voiced summary: severity counts, most-recent
events, sentinel freshness, one suggested next-step per rules.
**Jack is never silent.**

### 5. Persona is a reviewed file

The LLM's system prompt is loaded verbatim from
`crates/russell-doctor/prompts/jack.md`. Modifying Jack's voice
is a reviewed code change. The persona's design is documented at
[`../architecture/THE_JACK.md`](../architecture/THE_JACK.md).

### 6. Evidence bundle and journal table

Every `russell help` invocation writes:

- A new row to the `help_sessions` table (migration `0002`).
- A directory `~/.local/state/harness/evidence/help/<session-id>/`
  containing `soap.md`, `request.json`, `response.json`, and
  `transcript.jsonl`.
- A corresponding `events` row with `action: "help"`, scope
  `host`, and `evidence_ref` pointing at the bundle.

Retention for evidence bundles is 90 days (manual in MVP; reaper
in Phase 2) per
[`../specifications/PERSISTENCE_CATALOG.md`](../specifications/PERSISTENCE_CATALOG.md) §2.3.

### 7. How this preserves JR-3 / ADR-0008

ADR-0008 says the LLM never emits shell *in a context where the
dispatcher executes its output*. MVP Russell has no dispatcher.
The LLM's output is printed; nothing consumes it programmatically.
The seam for the richer triage loop (where the LLM would select
manifest IDs) remains intact: when ADR-0007 lifts, the Doctor's
call-site grows to add ID validation against loaded manifests,
and the persona gets a new system-prompt section. No rework of
the wire path.

## Consequences

### Positive

- Russell can help an operator on day one.
- Zero ambiguity about privacy (`zdr: true`, per-request).
- Offline fallback removes the "LLM dependency" failure mode.
- Single round-trip, no streaming, no retry → simple to reason
  about, easy to test (mock backend round-trip + offline path).
- Clean architectural seam to the full triage loop.

### Negative / accepted costs

- Jack cannot follow up on his own answers. One-shot only.
- No cost-control / rate-limit awareness in MVP. The operator
  pays per call; no backpressure.
- The operator's `--note` + the last 24h of samples are sent to
  OpenRouter on every call. The ZDR parameter mitigates this,
  but the operator should understand the envelope (documented
  in `PERSISTENCE_CATALOG.md` §6).
- Kimi K2.5 availability on a ZDR endpoint depends on OpenRouter
  provider coverage; if Moonshot's own endpoint goes non-ZDR,
  the fallback kicks in and the operator is notified.

### Neutral

- OpenRouter introduces a third-party dependency on day one, but
  under JR-6 we copy the HTTP wire pattern rather than depend on
  the `stack-llm` crate. The only runtime dep is `reqwest`.

## Alternatives Considered

### A. Depend on `stack-llm` directly

Rejected. Violates JR-6: introduces a transitive dependency on
`stack_types` and the whole slate/stack workspace. Russell
operator would need that workspace to build.

### B. Vendor `stack-llm` as a subtree

Rejected. Too much code for MVP needs. `stack-llm` has 3,086
lines; MVP needs ~300 of those (openai HTTP call, error
mapping, Kimi content-normalisation). The right granularity is
**copy the functions we need into Russell-shaped types**, cited
in the file headers.

### C. Use Ollama by default

Rejected as the default. Ollama + a local Kimi is reasonable
when the operator runs local inference, but it is not what the
operator asked for. OpenRouter + ZDR + frontier model is the
MVP default. Local Ollama is available via
`RUSSELL_DOCTOR_BACKEND=ollama`.

### D. Allow tool-calling

Rejected. Opens the door to the LLM constructing structured
actions. Every door we open increases the surface of JR-3
violation. MVP Jack returns plain text; nothing more.

### E. Multi-turn conversation

Rejected for MVP. A single round-trip is enough to help the
operator in most "what am I seeing?" moments. Multi-turn
introduces a state machine that earns its weight only when
skills land.

### F. Retry on 5xx / timeout

Rejected for MVP. Retries mask intermittent provider issues;
the operator should know the call failed and get the offline
fallback instead. The retry crate from `stack-llm` is
deliberately *not* copied for MVP.

## Implementation Notes

- Russell does **not** use the `stack-llm` trait. Russell's
  Doctor crate (`russell-doctor`) defines a minimal local
  trait `LlmClient` with one method `chat(&self, prompt:
  SoapPrompt) -> Result<LlmResponse, DoctorError>`.
- The implementations are:
  - `OpenRouterClient` — copies HTTP wire patterns from
    `stack-llm/src/openai.rs` and `wire.rs` per
    [`../operations/REUSE_MANIFEST.md`](../operations/REUSE_MANIFEST.md).
  - `MockClient` — deterministic scripted responses for tests.
  - `OfflineFallback` — the rule-based summary; always
    available, cannot fail.
- The env loader `russell-core::env::load_env` reads
  `~/.config/harness/russell.env` (if present) at startup, per
  ADR-0017.
- Migration `0002_help_sessions.sql` adds the table.
- Test coverage: mock backend round-trip snapshot, offline
  fallback snapshot, env-loading tests, journal migration
  idempotence (rerun `cargo test -p russell-core`).

## References

- [ADR-0008](0008-llm-triage-never-emits-shell.md) — the
  structural rule this ADR preserves.
- [ADR-0017](0017-reuse-over-dependency.md) — the JR-6
  mechanism we follow for copying.
- [`../specifications/MVP_SPEC.md`](../specifications/MVP_SPEC.md) §2.1 — the verb spec.
- [`../architecture/THE_JACK.md`](../architecture/THE_JACK.md) — persona design.
- OpenRouter docs: `openrouter.ai/docs/guides/features/zdr`,
  `openrouter.ai/docs/guides/routing/provider-selection`.
- Moonshot Kimi K2.5: `openrouter.ai/moonshotai/kimi-k2.5`.
