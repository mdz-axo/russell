# Okapi Integration Test Guide

**Version:** 1.0.0  
**Date:** 2026-05-20  
**Status:** Ready for manual execution

---

## Overview

This guide documents how to test the Russell okapi-watcher capability probing enhancement against a running Okapi instance.

---

## Prerequisites

1. **Okapi running** on `127.0.0.1:11435`
2. **Russell binary** built and accessible
3. **jq** installed for JSON parsing

---

## Installation

### Step 1: Build Russell

```bash
cd /home/mdz-axolotl/Clones/russell
cargo build --release
```

### Step 2: Install okapi-watcher Skill

```bash
mkdir -p ~/.local/share/russell/skills
cp -r /home/mdz-axolotl/Clones/russell/skills/okapi-watcher ~/.local/share/russell/skills/
```

### Step 3: Install Russell Binary (Optional)

```bash
cp /home/mdz-axolotl/Clones/russell/target/release/russell ~/.local/bin/
# Or with sudo:
sudo cp /home/mdz-axolotl/Clones/russell/target/release/russell /usr/local/bin/
```

---

## Manual Testing

### Test 1: Probe Capabilities Script

```bash
# Ensure Okapi is running
OLLAMA_HOST=127.0.0.1:11435 okapi list

# Run the probe script directly
OLLAMA_HOST=127.0.0.1:11435 bash ~/.local/share/russell/skills/okapi-watcher/scripts/probe-capabilities.sh
```

**Expected Output:**
```json
{
  "runner_type": "ollamarunner",
  "model_loaded": true,
  "model_name": "qwen3:8b",
  "capabilities": {
    "lora_hot_swap": true,
    "token_probs": true,
    "full_metrics": true,
    "advanced_sampling": true,
    "grammar_native": true,
    "speculative_decoding": true,
    "dry_sampler": true,
    "xtc_sampler": true,
    "min_keep": true,
    "chunked_prefill": true,
    "moe_observability": true
  },
  "degraded_mode": false,
  "feature_count": 11,
  "timestamp": "2026-05-20T09:00:00Z"
}
```

### Test 2: Russell okapi-probe Command

```bash
# Run with dry-run (no auto-actions)
russell okapi-probe

# Run with auto-apply (will trigger model load if needed)
russell okapi-probe --auto-apply --default-model qwen3:8b
```

### Test 3: Verify Journal Entries

```bash
# Query journal for capability events
russell journal query --scope okapi/capabilities

# Query journal for cycle events
russell journal query --module okapi/cycle
```

**Expected Journal Entry:**
```json
{
  "event_type": "observe",
  "tier": "okapi",
  "module": "okapi/capabilities",
  "summary": "runner_type=ollamarunner, degraded_mode=false, feature_count=11",
  "outputs": {
    "runner_type": "ollamarunner",
    "degraded_mode": false,
    "feature_count": 11
  }
}
```

---

## Verification Checklist

- [ ] `probe-capabilities.sh` returns valid JSON with all 11 capability flags
- [ ] `degraded_mode` is `false` when using ollamarunner
- [ ] `feature_count` matches number of `true` capability flags
- [ ] Russell journal contains `okapi/capabilities` events
- [ ] Russell journal contains `okapi/cycle` events with capability summary

---

## Troubleshooting

### Okapi Not Reachable

```bash
# Check Okapi is running
pgrep -f okapi
curl http://127.0.0.1:11435/api/version

# Restart Okapi if needed
systemctl --user restart okapi
# Or manually:
OLLAMA_HOST=127.0.0.1:11435 OKAPI_SIMPLE_ENGINE=1 okapi serve
```

### Probe Script Returns Error

```bash
# Debug the script
bash -x ~/.local/share/russell/skills/okapi-watcher/scripts/probe-capabilities.sh

# Check jq is installed
which jq

# Test curl directly
curl -s http://127.0.0.1:11435/api/engine/status | jq .
```

### Journal Entries Missing

```bash
# Check Russell paths
russell paths

# Verify journal directory exists
ls -la ~/.local/share/russell/journal/

# Check rules directory
ls -la ~/.local/share/russell/rules/
```

---

## Expected Behavior by Runner Type

### ollamarunner (Full Features)

| Capability | Expected |
|------------|----------|
| `lora_hot_swap` | `true` |
| `token_probs` | `true` |
| `full_metrics` | `true` |
| `advanced_sampling` | `true` |
| `grammar_native` | `true` |
| `speculative_decoding` | `true` |
| `dry_sampler` | `true` |
| `xtc_sampler` | `true` |
| `min_keep` | `true` |
| `chunked_prefill` | `true` |
| `moe_observability` | `true` |
| `degraded_mode` | `false` |
| `feature_count` | `11` |

### llamarunner (Fallback)

| Capability | Expected |
|------------|----------|
| `lora_hot_swap` | `false` |
| `token_probs` | `false` |
| `full_metrics` | `false` |
| `advanced_sampling` | `false` |
| `grammar_native` | `false` |
| `speculative_decoding` | `false` |
| `dry_sampler` | `false` |
| `xtc_sampler` | `false` |
| `min_keep` | `false` |
| `chunked_prefill` | `false` |
| `moe_observability` | `true` |
| `degraded_mode` | `true` |
| `feature_count` | `1` |

---

## References

- [Okapi Capabilities](../../okapi/server/capabilities.go)
- [Russell Okapi-Watcher](../../russell/skills/okapi-watcher/manifest.yaml)
- [Probe Script](../../russell/skills/okapi-watcher/scripts/probe-capabilities.sh)
- [Okapi Probe Command](../../russell/crates/russell-cli/src/commands/okapi_probe.rs)

---

*Okapi — Inference engine for hKask — v0.21.0*
*Russell — Host infrastructure curator — v0.20.0*
