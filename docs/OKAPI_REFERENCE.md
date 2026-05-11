# Okapi Reference — For Russell

Okapi is the local inference engine that Russell can use for LLM operations. It runs alongside Ollama on a separate port with additional capabilities.

## Connection

```
Okapi:   http://127.0.0.1:11435
Ollama:  http://127.0.0.1:11434  (standard, unchanged)
```

Set `OLLAMA_HOST=127.0.0.1:11435` to route requests to Okapi instead of Ollama.

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

## Models

Okapi shares the same model store as Ollama. Any model available to `ollama list` is also available to Okapi.

## When to Use Okapi vs Ollama

- Use **Okapi** when: adapter hot-swap is needed, token probabilities are needed, grammar constraints are needed, or metrics are needed.
- Use **Ollama** when: standard inference is sufficient, or when Okapi is not running.

Russell's Nurse module should prefer Okapi when available (check port 11435 responsiveness) and fall back to Ollama on 11434.

## Full Documentation

- Agent guide: `~/Clones/Okapi/AGENTS.md`
- API details: `~/Clones/Okapi/fork-docs/plans/KASK_INTEGRATION_POINTS.md`
