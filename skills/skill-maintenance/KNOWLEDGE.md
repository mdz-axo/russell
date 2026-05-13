# Skill Maintenance — Jack's Lifecycle Auditor

> **A note from Jack about maintaining skills:** Skills are like terriers — they
> don't age gracefully on their own. A probe written for Ubuntu 24.10 might
> reference paths that moved in 25.10. An intervention that worked on kernel
> 6.8 might be harmless or harmful on 6.14. A symptom that fired constantly
> six months ago might have been fixed by a package update — and the skill
> that watched for it is now dead weight. I don't let skills rot. I check them,
> score them, and when they're done, I retire them with a note. That's loyalty
> applied to tools.
>
> **Source:** This knowledge file. Activated during `russell skill workshop`.
> **Requires:** `skill-discovery` (manifest format knowledge), the symptom
> catalog (`SYMPTOMS`), and the registry cache.
> **Pairs with:** `skill-workshop` (composition and adaptation).

---

## 1. The Skill Lifecycle

Every skill moves through six states:

```
  discovered → evaluated → installed → active → stale_warning → deprecated → retired
```

| State | Meaning | How it got there |
|---|---|---|
| **discovered** | Found via search, not yet inspected | `search` command found it in a remote registry |
| **evaluated** | Manifest reviewed, safety scanned | `evaluate` command ran, operator read results |
| **installed** | On disk at `~/.local/share/harness/skills/<id>/` | `build` or `fetch` completed, poka-yoke passed |
| **active** | Loaded by harness, used in Jack sessions | Appears in `russell skill list` with probes |
| **stale_warning** | Authored > 6mo or `valid_until` passed | `check` command detected staleness |
| **deprecated** | Superseded, no longer relevant, still on disk | `prune` command moved it here |
| **retired** | Removed from skills directory | Operator deleted the directory |

Transitions:
- discovered → evaluated: operator runs `evaluate <slug>`
- evaluated → installed: operator consents, files are copied
- installed → active: automatic (poka-yoke passed = active)
- active → stale_warning: `check` detects age > 6mo
- stale_warning → active: operator updates `authored` date (skill refreshed)
- stale_warning → deprecated: operator runs `prune <slug>`
- deprecated → retired: operator deletes the directory
- Any state → evaluated: operator runs `evaluate <slug>` again

---

## 2. The `check` Audit — What I Look For

When the operator runs `check` (or `/check`), I audit every installed skill
against these criteria:

### 2.1 Staleness (age checks)

```
For each installed skill:
  authored_date = parse(skill.manifest.authored)
  age_days = today - authored_date

  if age_days > 180:
    → "skill-name is 8 months old. Review needed."

  if skill.manifest.valid_until exists and valid_until < today:
    → "skill-name validity expired on {valid_until}. Needs update or retirement."
```

A stale skill isn't broken. It might be perfectly fine. But it hasn't been
reviewed recently, and the world it was written for may have changed. I flag
it so the operator decides.

### 2.2 Symptom coverage gaps

```
catalogued_symptoms = set(SYMPTOMS)
covered_symptoms = union(all skills' symptoms)
uncovered = catalogued_symptoms - covered_symptoms

For each uncovered symptom:
  → "No skill covers {symptom}. Want me to search for one?"
```

This is the registry as a decision-support tool: which known failure modes
have no watcher?

### 2.3 Unused symptoms (skills watching nothing)

```
For each installed skill:
  if none of the skill's symptoms have fired in the last 30 days:
    → "skill-name covers {symptoms} but none have fired in 30 days.
       Is this still relevant?"
```

A skill that watches for symptoms that never fire is candidate for
deprecation. But it might just be that nothing has gone wrong — some skills
are insurance, not alarms.

### 2.4 Quality scoring

I score each skill on a 0.0–1.0 scale:

| Factor | Weight | Check |
|---|---|---|
| Manifest completeness | 0.20 | Has version, authored, symptoms, probes |
| Probe coverage | 0.25 | Probes exist for declared symptoms |
| Intervention coverage | 0.20 | Interventions exist for probe-detected issues |
| Rollback quality | 0.15 | Every intervention has valid rollback |
| Script quality | 0.10 | Scripts are well-formed, have shebangs, error handling |
| Documentation | 0.10 | KNOWLEDGE.md present or symptoms are self-documenting |

Qualitative output:
```
coverage scores:
  okapi-watcher        0.85  (probes: 3, interventions: 1, rollback: ok)
  web-search           1.00  (knowledge-only, complete)
  memory-guard         0.65  (probes: 1, no intervention yet — add one?)
  emergency-oom        0.40  (one probe, no version, no authored date)
```

---

## 3. Coverage Gap Analysis in Detail

The symptom catalog has 85 entries (and growing). Coverage is a key metric:

### Current coverage (at time of writing)

I compute coverage from the registry cache:

```
symptoms_covered = extract from registry-cache skills[*].symptoms
symptoms_total = len(SYMPTOMS)
coverage_pct = len(symptoms_covered) / symptoms_total * 100
```

I report gaps by category:

```
Coverage by category:
  Hardware/GPU:  6/7 covered  (missing: rocm_unreachable)
  System:        4/5 covered  (missing: loadavg_high)
  Ubuntu:       12/12 covered ✓
  Cybernetic:   10/10 covered ✓
  Semantic:      7/7 covered  ✓
  Sysadmin:      8/8 covered  ✓
  Web search:    9/9 covered  ✓
  Skill lifecycle: 9/9 covered ✓
```

### How I help fill gaps

When I find an uncovered symptom:
1. I search the registry: "Does any remote skill cover {symptom}?"
2. If yes: "There's a skill for that at {url}. Want me to fetch it?"
3. If no: "No existing skill covers {symptom}. Want me to help build one?"

---

## 4. Staleness — When and Why Skills Rot

### Things that make a skill stale

| Change | Which skills break | Example |
|---|---|---|
| Ubuntu upgrade (24.10→25.10) | Skills with hardcoded paths | `apt` commands, `snap` paths |
| Kernel update (6.8→6.14) | Skills reading `/proc` | `/proc/pressure/memory` field names changed |
| Package removal | Skills depending on CLI tools | `rocm-smi` removed, GPU probes fail |
| Hardware change | Skills targeting specific devices | GPU replaced, old `card1` path wrong |
| Service deprecation | Skills watching service units | Systemd unit renamed or removed |
| ROCm version bump | GPU skills | AMDGPU ring hang recovery API changed |

### How I detect staleness

| Signal | Detection |
|---|---|
| `authored` date > 6 months | `check` flags it |
| `valid_until` in manifest has passed | `check` flags it |
| Probe failed in last 3 runs | Registry recorded failure |
| Symptom hasn't fired in 90 days | Journal query (if available) |
| Operator reports: "this skill doesn't work" | Direct feedback |

### Staleness report format

```
Staleness audit — 2026-05-13:
  ✓ okapi-watcher        authored 2026-05-09 (4d ago) — fresh
  ⚠ ubuntu-jack          valid_until 2026-07-09 (57d remaining) — review soon
  ⚠ pragmatic-cybernetics authored 2026-05-10 (3d ago) — fresh but no valid_until
  ✗ emergency-oom        authored 2026-04-01 (42d ago) — approaching stale threshold
  ✗ custom-gpu-monitor   authored 2025-12-15 (149d ago) — due for review
  ✗ legacy-cpu-check     probe failures in last 3 runs — may be broken
```

---

## 5. Deprecation and Retirement

### When to deprecate

A skill should be deprecated when:
1. It's superseded by a better skill (e.g., `gpu-doctor` replaces `custom-gpu-monitor`)
2. The monitored service/hardware is no longer present
3. Its symptoms never fire and likely never will
4. It's been stale > 12 months with no review
5. The operator says "I don't need this anymore"

### Deprecation process

```
→ prune custom-gpu-monitor

"custom-gpu-monitor (v0.1.0, authored 2025-12-15):
  Symptoms: amdgpu_ring_hang, vram_oom
  Reason for deprecation: ___ (operator provides)

  This will:
  - Mark the skill as 'deprecated' in the registry
  - Keep files on disk (not deleted — JR-7: persistence is auditable)
  - Remove it from russell skill list (no longer loaded)
  - The journal retains all past probe results

  Deprecate? /approve"
```

### Retirement

After deprecation, the operator can retire a skill by deleting its directory:

```bash
rm -rf ~/.local/share/harness/skills/custom-gpu-monitor/
```

The registry retains the historical record (skill was here, covered these
symptoms, was deprecated on this date). Jack doesn't forget — JR-7.

### Merge/Replace

When a new skill supersedes an old one:

```
"gpu-doctor v0.2.0 replaces custom-gpu-monitor v0.1.0.
gpu-doctor covers: amdgpu_ring_hang, vram_oom, gpu_temp_high, rocm_unreachable
custom-gpu-monitor covered: amdgpu_ring_hang, vram_oom

  → Deprecate custom-gpu-monitor (superseded by gpu-doctor)
  → Registry notes: superseded_by: gpu-doctor
  → custom-gpu-monitor symptoms now covered by gpu-doctor"
```

---

## 6. The Registry as Decision-Support

The registry cache (`local-cache.yaml`) is not just a catalogue — it's a
lookup table for answering operational questions:

### "Which skill do I run for this symptom?"

```
/lookup vram_oom
→ okapi-watcher (probes: probe-health, probe-models)
→ gpu-doctor (probes: check-vram, check-temp; interventions: reset-gpu)
```

### "What symptoms am I not watching?"

```
/gaps
→ rocm_unreachable — no installed skill
→ loadavg_high — no installed skill
→ gpu_fallback_to_cpu — covered by okapi-watcher but no direct probe
```

### "Is this skill worth keeping?"

```
/check emergency-oom
→ Score: 0.40 (below threshold)
→ Authored: 2026-04-01 (42d ago)
→ Probes: 1 (check-ooms)
→ Interventions: 0
→ Recommendation: replace with memory-guard or add interventions
```

---

## 7. Maintenance Schedule

| Frequency | What to check |
|---|---|
| **Weekly** | `russell skill workshop` → `/check` (quick staleness pass) |
| **Monthly** | Full audit: staleness, coverage gaps, probe failure rates |
| **After Ubuntu upgrade** | All skills — paths, package names, kernel interfaces |
| **After hardware change** | Hardware-specific skills (GPU, disk, network) |
| **After new service deployed** | Coverage gap analysis for the new service |

---

## 8. Self-Maintenance — When Jack Flags Himself

I should proactively flag skill issues during normal `russell jack`/`chat`
sessions when I notice:

- A probe returned unexpected output (format changed)
- A symptom the operator asked about has no installed skill ("Want me to
  check the registry for a `disk_iopressure_high` skill?")
- A skill's `valid_until` date is approaching ("ubuntu-jack expires in
  30 days — want me to search for an update?")

I don't do a full audit in every conversation (token budget). But I mention
it when it's relevant. A nurse who notices the gauze is running low.

---

**Version:** 1.0.0
**Last updated:** 2026-05-13
**Depends on:** skill-discovery (manifest format), symptom catalog
**Pairs with:** skill-workshop (composition and adaptation)
