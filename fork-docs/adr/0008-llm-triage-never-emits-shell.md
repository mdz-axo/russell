---
title: "ADR-0008: LLM Never Emits Shell"
audience: [developers, architects, agents]
last_updated: 2026-06-07
ddmvss_context: "jack"
ddmvss_artifact: "adr"
version: "1.0.0"
status: "Superseded by ADR-0050"
---



<!--
audience: Doctor / LLM client contributors
last-reviewed: 2026-04-17
-->

# ADR-0008: LLM triage — the LLM never emits shell

- **Status:** Accepted
- **Date:** 2026-04-17
- **Deciders:** Project founders
- **Tags:** `doctor`, `llm`, `safety`, `trust-boundary`

## Context

[`cybernetic-health-harness.md` §12.2](../../cybernetic-health-harness.md)
describes the triage loop: the Doctor assembles a SOAP bundle,
invokes an LLM for differential diagnosis, executes probes,
asks the LLM to pick an intervention, runs it subject to risk
caps. If the LLM is allowed to generate command strings
directly, a single hallucination becomes a mutation on the
host. This is unacceptable given the charter's
**"observe > recommend > act"** posture.

Simultaneously, the LLM's capability Russell actually needs is
**ranking**: given a structured bundle (samples, probe outputs,
manifest) and a set of candidate IDs, pick and justify one.

## Decision

The LLM is constrained to a **classifier-over-IDs** role. The
dispatcher is the sole translator from ID to command.

Concrete constraints:

1. **Input to the LLM** is a fully-formed SOAP-shaped context
   plus the set of loaded manifests with probe IDs,
   intervention IDs, and their risk bands. The LLM sees
   names, descriptions, risk, and prior probe outputs.
2. **Output from the LLM** is a JSON object of the form:
   ```json
   {
     "differential": [
       { "hypothesis": "...", "evidence_refs": ["probe_id", ...], "confidence": 0.72 }
     ],
     "probe_plan": ["probe_id_a", "probe_id_b"],
     "intervention_plan": [
       { "id": "intervention_id", "justification": "...", "confidence": 0.68 }
     ]
   }
   ```
   Validated against a strict JSON Schema; free-text
   `cmd`, `args`, `sh`, `shell`, `exec`, `bash` fields are
   rejected. No tool-calling surface that admits shell is
   exposed.
3. **The dispatcher validates** every ID the LLM returns
   against the loaded manifest's known ID set (poka-yoke).
   An unknown ID is rejected without execution and logged
   as `doctor.llm.rejected_id`.
4. **Shell-shaped escapes are rejected** even in
   free-text fields: the schema validator refuses any
   value in `justification` longer than a configured
   limit (2048 chars) or containing control characters;
   the dispatcher only consumes IDs anyway.
5. **Confidence floor.** If the top intervention plan's
   self-reported confidence is below 0.60, the Doctor
   downgrades the run to `pending_confirm` regardless of
   risk band.
6. **Two-failure halt.** Two consecutive evaluation
   failures on the same symptom force the Doctor into
   human-handoff mode; no further intervention is
   auto-applied on that evidence_id.
7. **Full transcript logged** into the evidence bundle at
   `llm-transcript.jsonl`: each request, each response,
   each validation decision, with timestamps.
8. **Default backend is local Ollama** with a configurable
   model (default: a small instruct-tuned model). Remote
   backends are opt-in per `profile.json.network.llm_egress`.

## Consequences

### Positive

- An LLM misstep cannot become a mutation; the worst case
  is a rejected ID or a low-confidence proposal that is
  journaled and deferred.
- The trust boundary is crisp: manifest authors are
  trusted with shell; the LLM is not.
- The same pattern applies to self-triage
  ([ADR-0015](0015-proprioception-self-health.md)) with
  the same guarantees.

### Negative / accepted costs

- The LLM cannot improvise a fix that is not already in
  some skill manifest. This is the **correct behaviour**:
  improvised fixes are precisely what Russell is
  designed to prevent.
- Skill coverage is now a first-class bottleneck: unknown
  symptoms produce "no skill matches" rather than a
  clever recovery. We accept this in exchange for
  predictability.
- Manifests must grow as the symptom catalog grows.

### Neutral

- Local LLM latency is fine for SOAP runs (<10s typical
  on the target hardware).

## Alternatives considered

### Allow the LLM to propose commands as a last resort

Rejected. Any "only when other paths fail" escape hatch
eventually gets used; the safety argument collapses.

### Function-calling with a generic `run_command(cmd: str)` tool

Rejected for the same reason. The tool's surface is the
thing we are bounding.

### Sandboxed command execution (seccomp, bubblewrap)

Useful as defense-in-depth, but does not substitute for
the ID-only discipline: a sandboxed `rm -rf ~/.cache`
still does damage an operator did not authorize. We may
still adopt bubblewrap for probe execution in a future
ADR.

### Human-only triage (no LLM)

Rejected at the design stage. Rule-based differential
remains the fallback when the LLM is unavailable or low-
confidence; the LLM adds value by ranking in high-
cardinality symptom spaces.

## Implementation notes

- `russell-meta::llm` owns the request/response schema
  and validation.
- The transcript format `harness.llm-transcript.v1` is
  snapshot-tested with `insta`.
- The `evidence_read` MCP tool renders the transcript
  alongside the SOAP.
- Prompt templates live in-repo under
  `crates/russell-meta/src/llm/prompts/`. Changes to
  prompts are reviewed like code; a prompt change that
  expands the LLM's output surface is a locked decision
  and requires a superseding ADR.

## References

- [`cybernetic-health-harness.md` §12.2](../../cybernetic-health-harness.md)
- [`../standards/safety.md`](../standards/safety.md) §8
- [ADR-0007](deferred/0007-yaml-manifest-subprocess-skill-model.md)
- [ADR-0015](0015-proprioception-self-health.md)
