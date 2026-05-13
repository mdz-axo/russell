---
title: "Okapi Reference"
audience: [operators, developers, contributors]
last_updated: 2026-05-12
togaf_phase: "D"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Technology -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-12 -->


# Okapi Reference — For Russell

Okapi is Russell's **primary local inference engine**. It is a fork of Ollama that wraps llama.cpp with additional capabilities, Rust performance optimizations, and HuggingFace integrations.

## Connection

```
Okapi:   http://127.0.0.1:11435   (Russell's default backend)
```

Russell connects to Okapi on port 11435. The systemd user service is `okapi.service`.

## What Okapi Adds Over Ollama

| Capability | How to Use |
|-----------|-----------|
| **LoRA adapter hot-swap** | `POST /api/adapters/load {"path": "adapter.gguf", "scale": 1.0}` |
| **Token probabilities** | `"options": {"n_probs": 5}` in any request |
| **MoE expert offloading** | `"options": {"num_moe_offload": 64}` |
| **Grammar constraints** | `"options": {"grammar": "<GBNF string>"}` |
| **Mirostat sampling** | `"options": {"mirostat": 2, "mirostat_tau": 5.0}` |
| **Prometheus metrics** | `GET /metrics` |
| **Engine status** | `GET /api/engine/status` |

## API Compatibility

Okapi is wire-compatible with Ollama's API:

- `/api/tags` — list models
- `/api/generate` — single-shot generation
- `/v1/chat/completions` — OpenAI-compatible chat endpoint (used by Russell)
- `/api/show` — model info
- `/api/pull` — pull models

Okapi shares the same model store as Ollama. Any model available to `ollama list` is also available to Okapi.

## Russell Integration

- **Default backend**: `RUSSELL_DOCTOR_BACKEND=okapi` (default when unset)
- **Legacy alias**: Setting `RUSSELL_DOCTOR_BACKEND=ollama` routes to Okapi
- **Base URL override**: `RUSSELL_DOCTOR_BASE_URL=http://127.0.0.1:11435/v1`
- **Auto-start**: Russell will `systemctl --user start okapi` if not reachable
- **Health check**: `GET /api/tags` with 3s timeout
- **API key**: `"okapi"` (bearer token, same pattern as Ollama)

## Skill: okapi-watcher

The `okapi-watcher` skill monitors Okapi health:

- `probe-health` — checks `/api/tags` reachability
- `probe-models` — lists loaded models with sizes
- `restart-okapi` — `systemctl --user restart okapi` (risk: low)

## Full Documentation

- Agent guide: `~/Clones/Okapi/AGENTS.md`
- API details: `~/Clones/Okapi/fork-docs/plans/KASK_INTEGRATION_POINTS.md`
