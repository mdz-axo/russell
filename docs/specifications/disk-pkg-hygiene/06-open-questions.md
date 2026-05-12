---
title: "Disk & Package Hygiene — Task 6: Open Questions"
audience: [developers, architects, operators]
last_updated: 2026-05-06
togaf_phase: "Preliminary / Governance"
version: "0.1.0"
status: "Draft"
---

# Task 6 — Open Questions and Underspecified Aspects

The following aspects are explicitly deferred or require ADRs
before implementation proceeds.

---

## Question 1: Cadence Separation

**Problem.** Disk probes (statvfs, dir walks) fit the 5-minute
Sentinel cadence. Package probes (subprocess invocations) are
expensive — each shells out to apt/pip/npm/brew with a 5-second
timeout. Running 8 providers × 5s timeout = 40s worst case on
every 5-minute cycle is unacceptable.

**Options:**

| Option | Pros | Cons |
|---|---|---|
| **A: Second systemd timer** (`russell-pkg-sentinel.timer` at 1h/6h/24h) | Clean separation; independent failure domains; operator can disable pkg probes without affecting core Sentinel | Two timers to manage; two units to install; slightly more complex packaging |
| **B: Conditional in existing timer** (check elapsed time since last pkg run) | Single timer; simpler systemd unit; no new binary entry point | Conflates cadences in one code path; harder to reason about; Sentinel cycle may exceed 5 minutes if pkg probes run |
| **C: Separate binary** (`russell-pkg-sentinel`) | Maximum isolation; can have different resource limits | Violates JR-1 (one binary); more packaging complexity |

**Recommendation:** Option A. A second user-scope timer is the
cleanest separation. The existing `russell sentinel-once` verb
handles disk probes; a new `russell pkg-sentinel-once` verb (or
a flag: `russell sentinel-once --extended`) handles package probes.

**ADR required:** Yes. This changes the systemd unit topology.

---

## Question 2: Mutation Boundary (Observe → Recommend → Act)

**Problem.** The disk/package probes observe. But where does
"recommend" live?

| Phase | Capability | Surface |
|---|---|---|
| Phase 2 | **Observe** — collect metrics | Journal samples, `russell digest` |
| Phase 3 | **Recommend** — suggest actions | Jack's SOAP response (LLM connector) |
| Phase 4+ | **Act** — execute cleanup skills | IDRS-compliant skill dispatcher |

**Sub-question:** Should `russell digest` include recommendations
(e.g., "run `apt autoremove` to reclaim 2.3 GiB"), or should
recommendations be exclusively Jack's domain?

**Options:**

| Option | Pros | Cons |
|---|---|---|
| **A: Digest = facts only; Jack = recommendations** | Clean separation; digest is deterministic; Jack adds reasoning | Operator must run `russell jack` to get actionable advice |
| **B: Digest includes rule-based recommendations** | Immediate value without LLM; works offline | Digest becomes opinionated; recommendations without LLM reasoning may be wrong |

**Recommendation:** Option A for Phase 2. Digest reports facts
("cache_total_mib: 67,432"). Jack recommends actions ("Consider
running `pip cache purge` — your pip cache is 12 GiB and hasn't
been accessed in 45 days"). This maintains the tool/connector
separation: digest is a tool (formats data), Jack is a connector
(transfers data to LLM for assessment).

---

## Question 3: GitHub API Rate Limiting

**Problem.** Checking releases for curl-installed binaries hits
GitHub's API. Unauthenticated rate limit: 60 requests/hour.
With 4-5 tracked binaries checked daily, this is fine. But if
the operator tracks 20+ binaries, or checks more frequently,
rate limits become a concern.

**Options:**

| Option | Pros | Cons |
|---|---|---|
| **A: Conditional GET with ETags** | Efficient; cached responses don't count against limit | Requires persisting ETags (new state) |
| **B: Local cache with TTL (24h)** | Simple; no new persistence | Stale for up to 24h; still hits API once/day/artifact |
| **C: Defer entirely to daily batch** | Simplest; predictable API usage | No real-time drift detection |
| **D: Use authenticated token** | 5000 req/hour; effectively unlimited | Requires operator to provide GitHub token; security surface |

**Recommendation:** Option B for Phase 2 (simplest). The
provenance adapter checks each artifact at most once per 24 hours.
The "last checked" timestamp is stored as a journal sample
(`pkg_provenance_last_check_ts`). If the sample is <24h old,
skip the API call.

Phase 3 can upgrade to Option A (ETags) if operators track many
binaries.

---

## Question 4: Provider Discovery

**Problem.** Should Russell auto-detect which providers are
installed (by probing for `apt`, `pip`, `brew`, etc. in PATH),
or should the operator declare them?

**Options:**

| Option | Pros | Cons |
|---|---|---|
| **A: Auto-detect via PATH** | Zero config; ergonomic; adapts to system changes | Less auditable; may detect providers in unexpected locations |
| **B: Operator declares in profile.json** | Fully auditable (JR-7); explicit; no surprises | Manual maintenance; staleness risk |
| **C: Auto-detect with operator override** | Best of both; operator can disable noisy providers | More complex; two sources of truth |

**Recommendation:** Option A for Phase 2. Each provider adapter's
`is_available()` method checks PATH. This is the simplest approach
that satisfies JR-1. The provider registry is code-level (not
config-level), so adding a new provider requires a code change
anyway.

If operators need to suppress a provider (e.g., brew is installed
but they don't want Russell checking it), Phase 3 can add a
`disabled_providers` list in `russell.env` or `rules.d/`.

---

## Question 5: Cross-Provider Conflict Detection

**Problem.** Detecting that `numpy` is installed via both apt
(`python3-numpy`) and pip requires:
1. Enumerating apt Python packages (`dpkg -l 'python3-*'`)
2. Enumerating pip packages (`pip list --format=json`)
3. Normalizing names (`python3-numpy` ↔ `numpy`)
4. Comparing versions across providers
5. Determining which one "wins" in the import path

This is computationally expensive and semantically complex.

**Options:**

| Option | Pros | Cons |
|---|---|---|
| **A: Simple probe (returns count)** | Fits existing Sample model; cheap to store | Loses detail; operator can't act on a count alone |
| **B: Doctor-level assessment (structured finding)** | Rich detail; Jack can reason about it | Requires event (not sample); more complex |
| **C: Defer entirely** | Simplest; avoids false positives | Misses a real source of breakage |

**Recommendation:** Option B, deferred to Phase 3. Cross-provider
conflicts are Doctor territory — they require reasoning about
import paths, ABI compatibility, and resolution order. The probe
emits a count (`pkg_cross_provider_conflict_count`) as a signal,
but the detailed finding is an event with structured `outputs`
that Jack can reference.

This is where the **connector to Jack/Kask** adds the most value:
the LLM can reason about "numpy 1.24 via apt shadows numpy 1.26
via pip, which means your CUDA-enabled numpy is being overridden
by the system package" — something a simple threshold rule cannot
express.

---

## Question 6: Snap Revision Cleanup

**Problem.** Old snap revisions are the single largest source of
silent disk consumption on Ubuntu. `snap list --all | grep disabled`
identifies them, but removal requires `snap remove <name> --revision <rev>`.
This is a mutation.

**Phase mapping:**

| Phase | What Russell does |
|---|---|
| Phase 2 | **Observe:** `pkg_snap_held_revisions` count in journal |
| Phase 3 | **Recommend:** Jack says "you have 8 disabled snap revisions consuming ~4 GiB" |
| Phase 4 | **Act:** Skill `snap-prune` with IDRS compliance (risk: low) |

**Is snap revision removal safe enough for `risk: low`?**

Arguments for `low`:
- Disabled revisions are explicitly marked as unused by snapd
- `snap remove --revision` is idempotent
- Rollback = `snap revert` (snapd maintains the previous revision)
- No data loss (snap data dirs are separate from revision dirs)

Arguments for `medium`:
- Removes software from disk (even if unused)
- Edge case: operator might want to revert to a specific old revision

**Recommendation:** `risk: low` is appropriate. When skills land
in Phase 4, `snap-prune` satisfies IDRS:
- **I:** Running twice = same result (already-removed revisions are no-ops)
- **D:** `--dry-run` lists what would be removed
- **R:** `snap revert` restores the previous revision
- **S:** Event logged with revision IDs and space reclaimed

---

## Question 7: Threshold Calibration

**Problem.** What disk-used percentage triggers `warn` vs `alert`?
The 3.6 TB root partition means 90% used = 360 GiB free, which is
still generous. On a 256 GB laptop, 90% used = 25 GiB free, which
is critical.

**Options:**

| Option | Pros | Cons |
|---|---|---|
| **A: Absolute thresholds** (">90% used") | Simple; universal; easy to reason about | Doesn't account for partition size |
| **B: Relative to EWMA baselines** (">2σ above 30-day mean") | Adapts to the specific machine; detects anomalous growth | Requires EWMA infrastructure (Phase 2); cold-start problem |
| **C: Both** (absolute hard limits + relative anomaly detection) | Catches both gradual fill and sudden spikes | More complex rules |

**Recommendation:** Option A for Phase 2 (absolute thresholds in
`rules.d/disk-hygiene.toml`). Option C when EWMA baselines land.

For the 3.6 TB machine specifically:
- 80% used = 720 GiB free → `warn` (plenty of room, but trending)
- 90% used = 360 GiB free → `alert` (investigate)
- 95% used = 180 GiB free → `crit` (act now)

These are generous because the machine has generous storage. The
operator can tighten them in `rules.d/`.

---

## Question 8: Provenance Registry Ownership

**Problem.** The provenance registry (`provenance.toml`) is
operator-maintained. Is this the right ownership model?

**Options:**

| Option | Pros | Cons |
|---|---|---|
| **A: Operator-maintained only** | Clean ownership; auditable; no false positives | Staleness risk; operator forgets to update |
| **B: Russell auto-populates** (scan ~/.local/bin, attempt --version) | Always current; discovers new binaries | False positives; Russell WRITES (violates JR-2); noisy |
| **C: Russell proposes, operator confirms** (`russell provenance scan` → diff) | Best of both; operator stays in control | Requires new verb; more complex UX |

**Recommendation:** Option A for Phase 2 (operator-maintained).
Option C for Phase 3 (propose-and-confirm pattern, consistent
with observe > recommend > act).

The `russell provenance scan` verb would:
1. Scan `~/.local/bin` for executables not tracked by any provider
2. Attempt `<binary> --version` for each
3. Print proposed `[[artifact]]` entries to stdout
4. Operator copies desired entries into `provenance.toml`

This respects JR-2 (Russell doesn't write the file) while reducing
operator burden.

---

## Summary: ADR Requirements

| Question | ADR needed? | Blocking Phase 2? |
|---|---|---|
| 1. Cadence separation | Yes — **decided** ([ADR-0019](../../adr/0019-probe-cadence-and-okh.md)) | ~~Yes~~ Resolved |
| 2. Mutation boundary | No (follows existing JR-2 posture) | No |
| 3. GitHub rate limiting | No (implementation detail) | No |
| 4. Provider discovery | No (auto-detect is simplest) | No |
| 5. Cross-provider conflicts | No (deferred to Phase 3) | No |
| 6. Snap revision cleanup | No (deferred to Phase 4) | No |
| 7. Threshold calibration | No (absolute thresholds are default) | No |
| 8. Provenance ownership | No (operator-maintained is default) | No |

**Question 1 has been resolved by [ADR-0019](../../adr/0019-probe-cadence-and-okh.md).** No remaining ADR blockers for Phase 2 implementation.
