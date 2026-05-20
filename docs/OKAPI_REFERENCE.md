---
title: "Okapi Reference — Macaroon Authorization"
audience: [operators, developers, Russell developers]
last_updated: 2026-05-20
togaf_phase: "D"
version: "2.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Technology -->
<!-- VERSION: 2.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-20 -->


# Okapi Reference — For Russell

Okapi is Russell's **primary local inference engine**. It is a fork of Ollama that wraps llama.cpp with additional capabilities, Rust performance optimizations, and HuggingFace integrations.

## Architecture: Russell → hKask → Okapi (Macaroon-Based)

Russell does **not** connect directly to Okapi. Instead:

```
Russell (ACP Agent) → hKask (MCP Server Host) → Okapi (Inference Engine)
         │                    │                        │
         │ Macaroon with      │ Macaroon +             │ Verify HMAC chain
         │ skill caveat       │ discharge macaroon     │ Enforce caveats
         ▼                    ▼                        ▼
```

### Data Flow with Macaroons

1. **hKask issues macaroon to Russell** during skill registration:
   ```
   Macaroon: {
     iid: "russell-prod-1",
     skill: "evolution-watcher",
     before: "2026-05-21T00:00:00Z",
     third_party: "okapi-access"
   }
   ```

2. **Russell requests discharge from hKask MCP** for Okapi access:
   ```
   Russell → hKask MCP: POST /discharge
   Discharge: {
     okapi_access: true,
     models: ["qwen3:8b", "qwen3:70b"],
     before: "2026-05-20T12:00:00Z"
   }
   ```

3. **Russell binds discharge to primary macaroon** and invokes skill

4. **hKask forwards to Okapi** with bound macaroon:
   ```http
   POST http://127.0.0.1:11435/api/generate
   Authorization: Bearer <bound-macaroon>
   ```

5. **Okapi verifies macaroon** and enforces caveats

### Why Macaroons?

| Benefit | Description |
|---------|-------------|
| **Capability-based** | Russell's `evolution-watcher` skill cannot access `rdf-embedding` endpoints |
| **Decentralized** | No auth database required — Okapi verifies with root key only |
| **Delegation** | hKask can delegate Okapi access without sharing credentials |
| **Audit trail** | Caveat chain shows full authorization path |
| **Efficient** | HMAC verification is ~100x faster than JWT RSA/ECDSA |

## Connection

```
Okapi:   http://127.0.0.1:11435   (hKask's inference backend)
```

hKask connects to Okapi on port 11435. Russell connects to hKask via MCP with macaroon authentication.

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
| **Macaroon auth** | `Authorization: Bearer <macaroon>` header |

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
| Russell | ACP agent registered with hKask, holds macaroons |
| hKask | MCP server host, issues macaroons, routes inference requests |
| Okapi | Inference backend, verifies macaroons, enforces caveats |

**Configuration:**
- Russell connects to hKask via MCP with macaroon auth
- hKask connects to Okapi via HTTP with macaroon auth
- Russell inherits hKask's macaroon-based capability discovery

## Macaroon Caveats for Russell

| Caveat | Example | Purpose |
|--------|---------|---------|
| `iid` | `iid:russell-prod-1` | Identify Russell instance |
| `skill` | `skill:evolution-watcher` | Restrict to specific skill |
| `activity` | `activity:inference` | Allow inference operations |
| `endpoint` | `endpoint:/api/generate` | Restrict API endpoints |
| `model` | `model:qwen3:8b` | Restrict model access |
| `before` | `before:2026-05-21T00:00:00Z` | Expiration time |
| `quota` | `quota:1000000-tokens/day` | Token quota |

## Skill: okapi-watcher

The `okapi-watcher` skill monitors Okapi health with macaroon auth:

- `probe-health` — checks `/api/tags` with macaroon auth
- `probe-models` — lists loaded models with sizes
- `restart-okapi` — `systemctl --user restart okapi` (risk: low)

## Macaroon Client

Russell implements a macaroon client for hKask authentication:

- **Storage:** OS keychain (production) or encrypted file (development)
- **Auto-refresh:** 1 hour before expiry
- **Attenuation:** Per skill invocation
- **Discharge:** Automatic for Okapi access

See `macaroon-client.md` for detailed configuration and usage.

## Full Documentation

- Agent guide: `~/Clones/Okapi/AGENTS.md`
- API details: `~/Clones/Okapi/fork-docs/plans/KASK_INTEGRATION_POINTS.md`
- Macaroon spec: `~/Clones/Okapi/fork-docs/MACAROON_SPEC.md`
- Auth spec: `~/Clones/Okapi/fork-docs/AUTH_SPEC.md`
- Deployment: `~/Clones/Okapi/fork-docs/MACAROON_DEPLOYMENT.md`
- hKask issuer: `~/Clones/hKask/docs/integrations/macaroon-issuer.md`
- Russell client: `~/Clones/russell/docs/macaroon-client.md`
- Macaroon spec: `~/Clones/Okapi/fork-docs/MACAROON_SPEC.md`
- Auth spec: `~/Clones/Okapi/fork-docs/AUTH_SPEC.md`

---

*Okapi v0.22.0 — Macaroon-based multi-tenant inference engine*
