---
title: "ADR-0016: MVP Doctor — Local-First Ollama with Opt-In OpenRouter"
audience: [developers, architects, agents]
last_updated: 2026-05-09
ddmvss_context: "jack"
ddmvss_artifact: "adr"
version: "2.0.0"
status: "Superseded (naming)"
---



# ADR-0016: MVP Doctor — Local-First Ollama with Opt-In OpenRouter

- **Status:** Accepted (superseded v1.0.0)
- **Date:** 2026-04-18 (original), revised 2026-05-09
- **Deciders:** Project founders
- **Tags:** `doctor`, `llm`, `ollama`, `openrouter`, `mvp`
- **Supersedes:** ADR-0016 v1.0.0 (2026-04-18)
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

**v2 revision (2026-05-09).** The original ADR made OpenRouter the
default backend. Operator preference and the machine's local-first
posture flipped this: **Okapi is the default**, OpenRouter is
opt-in. The default model is `nemotron-3-super:cloud`. Russell now
checks for Okapi at `help` time and attempts to start it if it's
not running. OpenRouter remains available, and its ZDR enforcement
is preserved unchanged when used.

## Decision

### 1. MVP Doctor is a single verb

`russell jack [--note "..."]` is the only Doctor-facing CLI verb
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

### 2. Backend selection — default is Ollama

Configuration flows from `~/.config/harness/russell.env` (see
[`../specifications/PERSISTENCE_CATALOG.md`](../specifications/PERSISTENCE_CATALOG.md) §2.5):

```
RUSSELL_DOCTOR_BACKEND=okapi            # okapi | openrouter | mock | offline
RUSSELL_DOCTOR_MODEL=nemotron-3-super:cloud  # default model
OPENROUTER_API_KEY=sk-or-...           # only required for openrouter backend
```

Selection logic at startup:

1. If `RUSSELL_DOCTOR_BACKEND` is set, use it.
2. Else, default to `ollama`.

There is no longer a magic auto-detection path that silently
switches to OpenRouter when a key is present. OpenRouter is
**explicit opt-in** via `RUSSELL_DOCTOR_BACKEND=openrouter`.

### 3. Okapi backend — the default

The Okapi backend:

- **Base URL:** `http://127.0.0.1:11435/v1` (Okapi's
  OpenAI-compatible endpoint, overridable).
- **Default model:** `nemotron-3-super:cloud`.
- **Timeout:** 60 seconds. One attempt. No retry.
- **Auto-start.** Before sending the prompt, Russell checks
  whether Okapi is reachable via a quick GET to `/api/tags`
  (3-second timeout). If not reachable, he runs
  `systemctl --user start okapi` and waits 3 seconds for it
  to become ready. This is a best-effort convenience; it does
  not install or configure Okapi.
- **Fallback on failure.** If Ollama cannot be reached after
  the auto-start attempt, or if the chat completion fails, the
  offline rule-based summary is emitted instead.

### 4. OpenRouter backend — opt-in with ZDR enforced

When `RUSSELL_DOCTOR_BACKEND=openrouter`, the backend operates
as in ADR-0016 v1.0.0:

- **Base URL:** `https://openrouter.ai/api/v1` (overridable).
- **Default model:** whatever `RUSSELL_DOCTOR_MODEL` says, or
  `nemotron-3-super:cloud`.
- **Timeout:** 60 seconds. One attempt. No retry.
- **Provider preferences sent in every request:**

  ```json
  "provider": { "zdr": true, "data_collection": "deny" }
  ```

  This enforces zero data retention at the per-request level.
  A request that cannot be routed to a ZDR endpoint fails
  rather than silently routing to a retaining provider.

- **Identification:** `HTTP-Referer` set to a constant
  `https://russell.local/` and `X-Title` to `"Russell"`, per
  OpenRouter's app-attribution conventions.

### 5. Offline fallback is mandatory

If any of these is true, `russell jack` emits the **rule-based
fallback** instead of calling the LLM:

- The read from Ollama's `/api/tags` fails (i.e., Ollama not
  running) and the `systemctl --user start ollama` auto-start
  attempt also fails.
- The HTTP call to the configured backend fails (network,
  timeout, 5xx).
- The response body does not parse as a valid OpenAI-compatible
  chat completion.

The fallback reads the same SOAP bundle and produces a short,
deterministic, Jack-voiced summary: severity counts, most-recent
events, sentinel freshness, one suggested next-step per rules.
**Jack is never silent.**

### 6. Persona is a reviewed file

The LLM's system prompt is loaded verbatim from
`crates/russell-doctor/prompts/jack.md`. Modifying Jack's voice
is a reviewed code change. The persona's design is documented at
[`../architecture/THE_JACK.md`](../architecture/THE_JACK.md).

### 7. Evidence bundle and journal table

Every `russell jack` invocation writes:

- A new row to the `help_sessions` table (migration `0002`).
- A directory `~/.local/state/harness/evidence/help/<session-id>/`
  containing `soap.md`, `request.json`, `response.json`, and
  `transcript.jsonl`.
- A corresponding `events` row with `action: "help"`, scope
  `host`, and `evidence_ref` pointing at the bundle.

Retention for evidence bundles is 90 days (manual in MVP; reaper
in Phase 2) per
[`../specifications/PERSISTENCE_CATALOG.md`](../specifications/PERSISTENCE_CATALOG.md) §2.3.

### 8. How this preserves JR-3 / ADR-0008

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

- Russell can help an operator on day one — with zero network
  dependency (Ollama default).
- No API key required for the default path. One less obstacle
  to getting started.
- All host telemetry stays local in the Ollama case. Privacy
  is the default, not a ZDR configuration flag.
- When OpenRouter is used, `zdr: true` is still enforced per-request.
- Offline fallback removes the "LLM dependency" failure mode.
- Single round-trip, no streaming, no retry → simple to reason
  about, easy to test (mock backend round-trip + offline path).
- Clean architectural seam to the full triage loop.

### Negative / accepted costs

- Jack cannot follow up on his own answers. One-shot only.
- Okapi must be running and the model (`nemotron-3-super:cloud`)
  must be available before the first `russell jack`. Russell does
  not manage model downloads — the operator does.
- Auto-start (`systemctl --user start okapi`) assumes the
  operator has set up a user-scoped systemd unit for Okapi.
  If they haven't, the auto-start is a harmless no-op and the
  fallback kicks in.
- No cost-control / rate-limit awareness in MVP.
- OpenRouter introduces a third-party dependency when opted
  in, but under JR-6 we copy the HTTP wire pattern rather than
  depend on the `stack-llm` crate. The only runtime dep is
  `reqwest`.

### Neutral

- The OpenRouter path is fully preserved. Existing operators
  who prefer it set `RUSSELL_DOCTOR_BACKEND=openrouter` and
  carry on unchanged.

## Alternatives Considered

### A. Keep OpenRouter as default (v1.0.0 position)

Rejected in v2. The operator wants a local-first posture.
Ollama + DeepSeek V4 Pro is the default; OpenRouter is a
conscious opt-in for users who want a frontier cloud model.

### B. Depend on `stack-llm` directly

Rejected. Violates JR-6: introduces a transitive dependency on
`stack_types` and the whole slate/stack workspace. Russell
operator would need that workspace to build.

### C. Allow tool-calling

Rejected. Opens the door to the LLM constructing structured
actions. Every door we open increases the surface of JR-3
violation. MVP Jack returns plain text; nothing more.

### D. Multi-turn conversation

Rejected for MVP. A single round-trip is enough to help the
operator in most "what am I seeing?" moments. Multi-turn
introduces a state machine that earns its weight only when
skills land.

### E. Retry on 5xx / timeout

Rejected for MVP. Retries mask intermittent provider issues;
the operator should know the call failed and get the offline
fallback instead.

## Implementation Notes

- Russell does **not** use the `stack-llm` trait. Russell's
  Doctor crate (`russell-doctor`) defines a minimal local
  trait `LlmClient` with one method `chat(&self, prompt:
  SoapPrompt) -> Result<LlmResponse, DoctorError>`.
- The implementations are:
  - `OpenRouterClient` — copies HTTP wire patterns from
    `stack-llm/src/openai.rs` and `wire.rs` per
    [`../operations/REUSE_MANIFEST.md`](../operations/REUSE_MANIFEST.md).
    Used for both Ollama and OpenRouter backends (both are
    OpenAI-compatible).
  - `MockClient` — deterministic scripted responses for tests.
  - `OfflineFallback` — the rule-based summary; always
    available, cannot fail.
- **Ollama auto-start.** `help.rs` contains `ollama_health_check`
  (GET to `/api/tags`, 3s timeout) and `ollama_start`
  (`systemctl --user start ollama`). These run inside the
  Doctor's async context before the LLM call.
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
- Ollama docs: `ollama.com/docs`.
- OpenRouter docs: `openrouter.ai/docs/guides/features/zdr`,
  `openrouter.ai/docs/guides/routing/provider-selection`.
