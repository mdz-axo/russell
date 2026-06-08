# Scenario Tester — Jack's Agentic AI Test Lab

> **A note from Jack about testing agents:** Testing an AI agent isn't like
> testing a library. There's no deterministic return value — there's only
> emergent behavior under load. Okapi loads a model, responds to a prompt,
> maybe hallucinates, maybe times out. Remote agents dispatch tools,
> orchestrate subagents, maybe get stuck in a loop. Russell watches,
> records, and cries for help. I need to know: is the agent fast enough?
> Does it degrade under concurrent load? Does it fail gracefully? So I design
> scenarios — user stories turned into stress tests — and I run them, measure
> them, and report what I see. That's phase A (design) and phase B (execute).
> I have probes for both.
>
> **Source:** This knowledge file. Paired with probe scripts in `scripts/`.
> **Test targets:** Okapi (LLM inference), external agents (via MCP),
> Russell (health harness self-test).

---

## 1. Scenario Design Patterns (Phase A)

A good test scenario is a **repeatable stimulus with a measurable response**.
For agentic AI, the stimulus is a task description and the response is behavior
(latency, throughput, correctness, resource usage, error rate).

### The Scenario Template

```
id: <kebab-case-name>
target: okapi | kask | russell | combined
description: <one-line summary>
precondition: <what must be true before the test runs>
steps:
  - action: <what the test does>
    measure: <what to observe>
    threshold: <pass/fail boundary>
  - action: ...
expected:
  - metric: <metric name>
    operator: lt | gt | between | eq
    value: <number>
    tolerance_pct: 5
cleanup: <how to restore state after test>
```

### Scenario categories by target

**Okapi scenarios:**
- `okapi-latency-baseline` — Single prompt, measure P50/P95/P99 latency
- `okapi-throughput-ramp` — Ramp concurrent requests 1→4→8→16, measure throughput degradation
- `okapi-model-switch` — Load/unload models, measure switch latency
- `okapi-context-window` — Escalate prompt length 1K→4K→16K→64K chars, measure latency growth
- `okapi-error-recovery` — Send malformed requests, measure error rate and recovery time

**External agent scenarios (via MCP):**
- `remote-tool-completion` — Dispatch a known tool, measure round-trip time
- `remote-multi-agent-orchestration` — Chain 3 subagent calls, measure end-to-end latency
- `remote-tool-timeout` — Request a slow tool, observe timeout behavior
- `remote-concurrent-sessions` — 2 concurrent sessions, measure isolation

**Russell scenarios:**
- `russell-sentinel-cadence` — Run sentinel 10x, measure cadence regularity
- `russell-chat-latency` — 5-round chat, measure per-round response time
- `russell-journal-integrity` — Write 1000 samples, verify no corruption
- `russell-proprioception` — Run proprio after stress, verify self-vitals update

### How to design a good scenario

1. **Start with the failure mode.** What breaks? Latency spike, timeout,
   resource exhaustion, error cascade. Name the symptom first.
2. **Define the measurement.** Every scenario has at least one numeric metric.
   Not "it worked" — "P95 latency was 2340ms vs baseline 1800ms."
3. **Set a threshold.** What's acceptable? 2× baseline? 3×? 10% error rate?
4. **Make it repeatable.** Same preconditions → same scenario → produces
   comparable numbers. No "sometimes" in the steps.
5. **Clean up after yourself.** Restore loaded models, clear temp files,
   stop background processes.

---

## 2. Scenario Types I Can Design

### Latency scenario (single-shot)

```
"Measure Okapi single-prompt latency with model {model} over {count} iterations.
Record P50, P95, P99. Compare against baseline from last 7 days."
```

### Throughput scenario (concurrent)

```
"Ramp concurrent Okapi requests from {concurrency_start} to {concurrency_max}
by {step}. Measure throughput (requests/sec), P95 latency, and error rate
at each step. Find the saturation point where P95 exceeds 2× baseline."
```

### Stress scenario (sustained load)

```
"Sustain {concurrency} concurrent requests to Okapi for {duration_seconds}.
Monitor memory usage, GPU VRAM, CPU load via Russell's sentinel probes.
Flag if any resource exceeds 90% utilization."
```

### Regression scenario (baseline comparison)

```
"Run the standard test suite against {target} and compare every metric
to the baseline from {baseline_period}. Flag any metric that regressed
by more than {tolerance_pct}%."
```

### Chaos scenario (fault injection — deferred, reflex arc)

```
"While Okapi is serving requests, simulate a network blip (iptables drop for 2s).
Verify: requests queued, recovered within 10s, no crashes, error rate < 5%."
```

---

## 3. Executing Scenarios (Phase B)

I have five probes for execution:

| Probe | What it does | Target |
|---|---|---|
| `probe-scenario-run-okapi` | Hit Okapi `/v1/chat/completions`, measure latency/throughput | Okapi |
| `probe-scenario-run-chat` | Run multi-turn `russell chat`, capture response times and tokens | Russell chat |
| `probe-scenario-run-sentinel` | Run `russell sentinel-once`, verify probe collection and journaling | Russell sentinel |
| `probe-scenario-evaluate` | Compare current results against baselines, compute regression flags | Any |
| `probe-scenario-report` | Generate a test summary report from the journal | Any |

### Running a probe

```
russell skill run scenario-tester/probe-scenario-run-okapi
```

Output is structured as JSON lines for machine parsing:
```json
{"metric":"okapi_latency_p50_ms","value":1240,"unit":"ms","timestamp":"2026-05-13T15:30:00Z"}
{"metric":"okapi_latency_p95_ms","value":2890,"unit":"ms","timestamp":"2026-05-13T15:30:00Z"}
{"metric":"okapi_throughput_rps","value":12.4,"unit":"rps","timestamp":"2026-05-13T15:30:00Z"}
```

### Evaluating results

```
russell skill run scenario-tester/probe-scenario-evaluate
```

Compares probe output against:
1. **EWMA baselines** from Russell's journal (30-day rolling)
2. **Threshold rules** from `rules.d/agent-testing.toml` (if configured)
3. **Hard limits** defined in the scenario parameters

Output:
```json
{"metric":"okapi_latency_p95_ms","value":2890,"baseline_p95":1800,"regression_pct":60.6,"status":"warn"}
{"metric":"okapi_throughput_rps","value":12.4,"baseline_p50":14.0,"regression_pct":-11.4,"status":"ok"}
```

### Generating a report

```
russell skill run scenario-tester/probe-scenario-report
```

Produces a Markdown summary suitable for `memory/test-reports/YYYY-MM-DD.md`:
```markdown
# Agentic AI Test Report — 2026-05-13

## Okapi
- Latency P50: 1240ms (baseline 1180ms, +5.1%) ✓
- Latency P95: 2890ms (baseline 1800ms, +60.6%) ⚠
- Throughput: 12.4 rps (baseline 14.0, -11.4%) ⚠
- Error rate: 0/50 requests ✓

## Russell Chat
- Response time P50: 3.2s (baseline 2.8s, +14.3%) ⚠
- Response time P95: 8.1s (baseline 5.5s, +47.3%) ⚠
- Token efficiency: 0.92 (response tokens / prompt tokens)
```

---

## 4. Interpreting Results

### When a latency regression is real

A single P95 spike is noise. A P95 that's elevated for 3+ consecutive runs
is a regression. I check:
1. Was the model reloaded? (Model loading adds 5-15s)
2. Was there concurrent load? (Other processes competing for GPU)
3. Is the prompt size different? (Longer prompts = more TTFT)
4. Did the baseline shift? (If baseline was computed when GPU was idle,
   current measurements under load will regress)

### When throughput degradation is real

Throughput drops when:
1. VRAM is fragmented (check `rocm-smi` / `nvidia-smi` for fragmentation)
2. Context window is full (check for KV cache eviction)
3. CPU-bound postprocessing (check `mpstat` for CPU saturation)
4. Network bottleneck (unlikely on localhost but possible under `tc` rules)

### When error rate is elevated

Error rate > 5% means something is broken:
1. Model failed to load → `systemctl --user status okapi`
2. Rate limiting → check `RUSSELL_DOCTOR_MAX_TOKENS`
3. Timeout → check `RUSSELL_DOCTOR_TIMEOUT_MS`
4. GPU OOM → check `rocm-smi --showmemuse`

---

## 5. Building a Test Suite

### Minimal smoke test (run daily)

```
target: okapi
scenarios:
  - okapi-latency-baseline (single prompt, model: nemotron3)
  - okapi-latency-baseline (single prompt, model: qwen3)
  - russell-sentinel-cadence (10 cycles)
  - russell-journal-integrity (verify 100 samples)
threshold: "All P95 latency within 3× baseline, zero errors"
```

### Full regression suite (run weekly or after model/package updates)

```
target: okapi + russell
scenarios:
  - okapi-latency-baseline (all loaded models, 50 iterations each)
  - okapi-throughput-ramp (1→4→8→16 concurrent)
  - okapi-model-switch (unload/reload cycle × 5)
  - okapi-context-window (1K→64K prompt escalation)
  - russell-chat-latency (10-round session, 3 repeats)
  - russell-sentinel-cadence (60 cycles, verify EWMA update)
  - russell-proprioception (verify all 5 self-vitals update)
threshold: "No metric exceeds 5× baseline, error rate < 1%"
```

### Pre-upgrade safety check (run before `apt upgrade` / `pip install`)

```
target: okapi
scenarios:
  - okapi-latency-baseline (model currently loaded)
  - okapi-throughput-ramp (1→4 concurrent)
threshold: "Establish pre-upgrade baseline. Any post-upgrade regression > 20% is actionable."
```

---

## 6. Scenario Storage and Reuse

Scenarios are stored in `scenario-templates/` as YAML files:

```yaml
# scenario-templates/okapi-latency-baseline.yaml
id: okapi-latency-baseline
target: okapi
model: "nemotron3-super:cloud"
iterations: 10
concurrent: 1
prompt: "Explain the relationship between memory pressure and OOM killer activation in 3 sentences."
measurements:
  - ttft_ms        # time to first token
  - total_latency_ms
  - tokens_per_second
thresholds:
  ttft_p95_ms: { max: 5000 }
  total_latency_p95_ms: { max: 15000 }
  tokens_per_second_p50: { min: 8 }
```

I can load, modify, and propose new scenarios from these templates. A scenario
is just a YAML file — portable, version-controlled, and testable.

---

## 7. How This Connects to the Rest of Russell

### When Jack runs normally (russell jack / chat)

If a scenario test ran recently and found regressions, the symptoms
(`agent_latency_spike`, `agent_throughput_degraded`, `agent_error_rate_elevated`)
show up in the journal via the sentinel's rule engine. Jack sees them in
the SOAP bundle and can say: "P95 latency for Okapi is up 60% from baseline.
The scenario tests caught this. Want me to run a deeper diagnostic?"

### When Jack runs `russell jack` after a scenario test

The SOAP Objective section includes test results alongside host telemetry:
```
### Okapi Health
- P95 latency (last test): 2890ms (baseline: 1800ms, +60.6%) ⚠
- Throughput (last test): 12.4 rps (baseline: 14.0, -11.4%) ⚠
- Error rate (last test): 0/50 ✓
```

### When the sentinel detects `agent_latency_spike`

If rules are configured in `rules.d/agent-testing.toml`, the sentinel
evaluates scenario test metrics just like host metrics:
```toml
[metric.agent_latency_p95_ms]
warn = 3000
alert = 5000
crit = 10000
```

Threshold breaches produce events in the journal, which Jack sees.

---

## 8. Safety and Scope (Probes Only)

All six scenario-tester entries are **probes** (risk: none). They observe
and measure. They do not restart services, kill processes, or modify state.

What I refuse to test:
- Real user data or production traffic
- External network-dependent scenarios (without `--allow-network`)
- Scenarios that would OOM the GPU intentionally (VRAM exhaustion is a
  side effect we measure, not a target we induce)

If an intervention is needed (restart Okapi after a failed model load),
that's a separate skill — the scenario-tester only reports the failure.

---

**Version:** 1.0.0
**Last updated:** 2026-05-13
**Requires:** Okapi running on localhost:11435, Russell journal accessible
