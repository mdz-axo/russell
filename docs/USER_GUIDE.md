---
title: "Russell User Guide"
audience: [operators]
last_updated: 2026-05-20
version: "1.0.0"
status: "Active"
---

# Russell User Guide

> *Though she be but little, she is fierce.*

This guide helps you operate Russell on your Linux workstation. Russell is a cybernetic health harness — he observes your machine, remembers what he sees, and cries for help when needed.

## 1. Quick Start

### 1.1 Installation

```bash
cd ~/Clones/russell
./packaging/bin/install.sh --release
```

See [`operations/INSTALL.md`](operations/INSTALL.md) for full installation instructions.

### 1.5 Essential Commands

| Command | Purpose |
|---|---|
| `russell status` | Current machine health summary |
| `russell list --limit 20` | Last 20 journal events |
| `russell digest --since-hours 168` | Weekly health report (7 days) |
| `russell jack --note "…"` | Ask Jack about a specific concern |
| `russell chat` | Interactive REPL with Jack |
| `russell skill list` | List installed skills |
| `russell skill run <id>` | Run a skill probe/intervention |

### 1.3 First Week with Russell

**Day 1:** Russell establishes baselines. Expect limited insights — he needs 30 days of data for full EWMA baselines.

**Day 7:** First weekly digest auto-generates (Sunday 09:00). Review with `russell digest --since-hours 168`.

**Day 30:** Baselines complete. Jack can now cite p95 (30-day percentile) values in his assessments.

## 2. Understanding Jack's Responses

Jack (the Nurse persona) speaks in three registers:

### 2.1 Observation Mode (Nothing Wrong)

> "Memory's fine. Swap's at 3.2 GB and climbing — who's eating it? Check `/proc/swaps` and get back to me."

Jack notices, doesn't diagnose. He points to evidence.

### 2.2 Alert Mode (Something Elevated)

> "Crit: NVMe media errors went from 0 to 3 in the last hour. That's not a blip. Open the evidence bundle."

Jack cites journal rows. He never hallucinates metrics.

### 2.3 Offline Mode (LLM Unreachable)

> "[offline fallback engaged — Ollama unreachable]
> 
> Nothing's wrong. I know you're worried. Go make coffee. I'm watching."

Jack always responds — LLM is optional.

## 3. Configuration

### 3.1 Environment Variables

Edit `~/.config/harness/russell.env`:

```bash
# Backend selection (default: okapi)
RUSSELL_DOCTOR_BACKEND=okapi

# Okapi (local Ollama)
OLLAMA_BASE_URL=http://127.0.0.1:11435/v1
OLLAMA_MODEL=deepseek-v4-pro:cloud

# OpenRouter (optional cloud fallback)
# OPENROUTER_API_KEY=sk-or-v1-…

# Rate limiting (default: 3 requests/minute)
# Override for faster local Okapi or slower cloud backends
RUSSELL_LLM_RATE_LIMIT=3
```

**Q8 Decision:** The rate limit can be overridden via `RUSSELL_LLM_RATE_LIMIT=N` environment variable. Default is 3 req/min. Example: `RUSLL_LLM_RATE_LIMIT=5` allows 5 requests/minute.

### 3.2 Backend Precedence

1. **Okapi (local Ollama)** — default, no API key needed
2. **OpenRouter** — requires `OPENROUTER_API_KEY`
3. **Offline fallback** — always available, rule-based summary

### 3.3 Consent Mode

Jack requires consent before running interventions:

```bash
# Strict mode (only /approve accepted)
# Add to ~/.config/harness/russell.env:
RUSSELL_CONSENT_MODE=strict

# Conversational mode (default — "ok", "yes", "go ahead" accepted)
RUSSELL_CONSENT_MODE=conversational
```

**Approval expiry:** All approvals expire after 5 minutes (300 seconds).

## 4. Security Features

### 4.1 Prompt Sanitization

Russell sanitizes all LLM input/output to prevent:

- **Prompt injection** — blocks phrases like "ignore previous instructions"
- **Secret exfiltration** — redacts `RUSSELL_*` env vars, API keys
- **ACTION injection** — validates skill IDs against registered manifests
- **Shell metacharacters** — strips `;|&$()` from input/output

**What you'll see:**

```
[LLM response was sanitized: redacted RUSSELL_* references]
```

This is normal — Jack is protecting your environment.

### 4.2 Capability Attenuation

Skills only receive declared environment variables:

```yaml
# Skill manifest (manifest.yaml)
safety:
  allowed_env_keys: ["HOME", "LANG", "PATH"]
  needs_network: false
```

Skills cannot access undeclared env vars (API keys, tokens, etc.).

### 4.3 Consent Expiry

All intervention approvals expire after **5 minutes**. This prevents:

- Stale approvals executing after context shifts
- Accidental execution of old proposals
- Session hijacking via delayed consent

**Error message:**
```
→ Approval expired; please re-confirm
```

## 5. Troubleshooting

### 5.1 Ollama Unreachable

**Symptom:** `russell jack` returns offline fallback

**Fix:**
```bash
# Check Ollama status
systemctl --user status ollama.service

# Start Ollama manually
ollama serve

# Pull required model
ollama pull deepseek-v4-pro:cloud
```

### 5.2 Skills Not Found

**Symptom:** `russell skill run <id>` fails with "skill not found"

**Fix:**
```bash
# List installed skills
russell skill list

# Skills live in ~/.local/share/harness/skills/
ls ~/.local/share/harness/skills/

# Reinstall skills
cp -r ~/Clones/russell/skills/* ~/.local/share/harness/skills/
```

### 5.3 Baselines Stale

**Symptom:** Jack warns "baseline freshness warning: X probe(s) have stale baselines"

**Cause:** Baselines not updated in >7 days

**Fix:**
```bash
# Trigger baseline refresh
cargo run -- sentinel-refresh-baselines

# Or wait for daily auto-refresh (03:00 local)
```

### 5.4 GPU Probes Return None

**Symptom:** `gpu_vram_used_pct` returns no data

**Cause:** Dynamic dGPU detection failed or sysfs paths missing

**Fix:**
```bash
# Check GPU sysfs
ls /sys/class/drm/card*/device/mem_info_vram_total

# Force card1 (legacy behavior)
# Edit crates/russell-sentinel/src/probes/gpu.rs if needed
```

## 6. Evidence Bundles

Every `russell jack` session creates an evidence bundle:

```
~/.local/state/harness/evidence/help/<session-id>/
  soap.md           # Prompt sent to LLM
  request.json      # Request metadata
  response.json     # Response metadata
  transcript.jsonl  # Full transcript
```

**Access bundles:**
```bash
# Latest session
russell jack --note "test"
# Bundle path printed in output

# Open evidence in browser (optional)
cat ~/.local/state/harness/evidence/help/<session-id>/transcript.jsonl
```

## 7. Regular Operations

### 7.1 Daily Checks

```bash
# Morning check
russell status

# Review overnight events
russell list --limit 10
```

### 7.2 Weekly Digest

Auto-generated Sunday 09:00. Manual trigger:

```bash
russell digest --since-hours 168
```

### 7.3 Monthly Review

```bash
# Full month summary
russell digest --since-hours 720

# Baseline status
russell proprio
```

## 8. Skills Reference

### 8.1 Built-in Skills

| Skill | Purpose |
|---|---|
| `okapi-watcher` | Monitor Ollama health |
| `scenario-tester` | Run test scenarios |
| `skill-manager` | Install/prune skills |
| `sysadmin` | System admin playbooks |
| `ubuntu-jack` | Ubuntu conventions |

### 8.2 Running Skills

```bash
# List skill probes
russell skill list okapi-watcher

# Run a probe
russell skill run okapi-watcher/probe-okapi

# Dry-run (print without executing)
russell skill run --dry-run <id>
```

## 9. Getting Help

| Resource | Contents |
|---|---|
| [`../AGENTS.md`](../AGENTS.md) | Binding rules, authority hierarchy |
| [`README.md`](README.md) | Documentation portal |
| [`operations/INSTALL.md`](operations/INSTALL.md) | Installation guide |
| [`reference/cli.md`](reference/cli.md) | Command reference |
| [`standards/safety.md`](standards/safety.md) | IDRS contract, risk bands |

## 10. Next Steps

1. **Week 1:** Run Russell daily, review digests
2. **Week 2:** Install additional skills from `skills/`
3. **Week 3:** Customize config (`~/.config/harness/russell.env`)
4. **Week 4:** Write your first skill (see `skills/templates/`)

---

*Russell is watching. Go make coffee.*