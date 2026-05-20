---
title: "Okapi Reference"
audience: [operators, developers, contributors]
last_updated: 2026-05-20
togaf_phase: "D"
version: "1.1.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Technology -->
<!-- VERSION: 1.1.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-20 -->


# Okapi Reference — For Russell

Okapi is Russell's **primary local inference engine**. It is a fork of Ollama that wraps llama.cpp with additional capabilities, Rust performance optimizations, and HuggingFace integrations.

## Architecture: Russell → hKask → Okapi

Russell does **not** connect directly to Okapi. Instead:

```
Russell (ACP Agent) → hKask (MCP Server Host) → Okapi (Inference Engine)
```

### Data Flow

1. Russell registers as an **ACP agent** with local hKask installation
2. Russell requests inference through hKask MCP tools (`hkask-mcp-inference`)
3. hKask routes request to Okapi via `/api/generate` or `/api/chat`
4. Okapi responds to hKask, which responds to Russell

### Why This Architecture?

| Benefit | Description |
|---------|-------------|
| **Unified authentication** | Russell inherits hKask's auth model |
| **Centralized routing** | hKask can route to different backends |
| **Capability discovery** | hKask exposes Okapi capabilities via MCP |
| **Observability** | All requests traced through hKask CNS spans |

## Connection

```
Okapi:   http://127.0.0.1:11435   (hKask's inference backend)
```

hKask connects to Okapi on port 11435. Russell connects to hKask via MCP.

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

Russell integrates with Okapi **through hKask**, not directly:

| Component | Role |
|-----------|------|
| Russell | ACP agent registered with hKask |
| hKask | MCP server host, routes inference requests |
| Okapi | Inference backend for hKask |

**Configuration:**
- Russell connects to hKask via MCP
- hKask connects to Okapi via HTTP (`http://127.0.0.1:11435`)
- Russell inherits hKask's authentication and capability discovery

## Skill: okapi-watcher

The `okapi-watcher` skill monitors Okapi health:

- `probe-health` — checks `/api/tags` reachability
- `probe-models` — lists loaded models with sizes
- `restart-okapi` — `systemctl --user restart okapi` (risk: low)

## Full Documentation

- Agent guide: `~/Clones/Okapi/AGENTS.md`
- API details: `~/Clones/Okapi/fork-docs/plans/KASK_INTEGRATION_POINTS.md`
