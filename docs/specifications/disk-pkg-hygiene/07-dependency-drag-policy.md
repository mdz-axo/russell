---
title: "Disk & Package Hygiene — Policy: Dependency Drag"
audience: [operators, developers, agents]
last_updated: 2026-05-07
togaf_phase: "H — Change Management"
version: "1.0.0"
status: "Active"
---

# Dependency Drag Policy

## Definition

A **dependency dragger** is an installed package whose declared
version constraints conflict with the currently-installed versions
of its dependencies. It is holding back the ecosystem — preventing
other packages from updating, or forcing pip to downgrade packages
when it resolves dependencies.

Observable signal: `pip check` reports the conflict. The package
either:
1. Pins an exact old version (`==X.Y.Z`) of a dependency that
   has moved on
2. Sets an upper bound (`<N.0`) that the dependency has exceeded
3. Requires a package that has been uninstalled

## Severity Classification

| Category | Definition | Action |
|---|---|---|
| **Dead drag** | Package pins ≥3 dependencies behind, hasn't released in >90 days | Uninstall |
| **Stale drag** | Package pins 1-2 dependencies behind, last release >30 days ago | Flag for review; uninstall if not actively used |
| **Active drag** | Package pins dependencies but released within 30 days | Hold — upstream is likely working on it |
| **Torch-class drag** | Major framework (torch, tensorflow) pins CUDA/build deps | Accept — these are intentional ABI pins |
| **Orphan** | Package requires something that was uninstalled as a dragger | Uninstall (cascade) |

## The Three Questions

When Russell detects a dependency dragger, the operator (or Jack)
asks:

1. **Is this package actively used?** If not → uninstall.
2. **Is the upstream actively maintained?** Check last release
   date and issue tracker. If abandoned → uninstall.
3. **Is the pin intentional and load-bearing?** (e.g., torch
   pinning CUDA libs for ABI safety) If yes → accept and
   document.

## Probe Design

### `pkg_dependency_drag_count`

**Source:** `pip check` output, parsed for conflict lines.

**Cadence:** Daily (same as package ecosystem probes).

**Tool/Connector decomposition:**
- **Connector:** `Command::new("pip").args(["check"]).output()` with 10s timeout
- **Tool:** Parse output lines matching `X requires Y, which is not installed` and `X has requirement Y, but you have Z`; group by blocker package; count unique blockers

**Samples emitted:**

| Probe | Unit | Description |
|---|---|---|
| `pkg_drag_total_conflicts` | count | Total number of dependency conflicts |
| `pkg_drag_blocker_count` | count | Number of distinct packages causing conflicts |
| `pkg_drag_dead_count` | count | Blockers with ≥3 conflicts (dead drag) |
| `pkg_drag_orphan_count` | count | Packages requiring uninstalled dependencies |

**Severity thresholds:**

| Metric | Warn | Alert |
|---|---|---|
| `pkg_drag_total_conflicts` | ≥ 5 | ≥ 15 |
| `pkg_drag_blocker_count` | ≥ 3 | ≥ 8 |
| `pkg_drag_dead_count` | ≥ 1 | ≥ 3 |

### `pkg_drag_detail` (structured event)

When `pkg_drag_dead_count` > 0, Russell emits a `harness.event.v1`
record with structured `outputs`:

```json
{
  "action": "observe",
  "module": "pkg_ecosystem/dependency_drag",
  "severity": "warn",
  "outputs": {
    "dead_draggers": [
      {
        "package": "ontolearner",
        "version": "1.4.11",
        "conflicts": 7,
        "pins": ["pydantic==2.11.3", "rdflib==7.1.1", "transformers<5.0.0"]
      }
    ],
    "orphans": [
      {
        "package": "funowl",
        "requires": "rdflib-shim",
        "status": "not_installed"
      }
    ]
  }
}
```

## Process: Automated Detection, Human Decision

Russell's role is **observe and recommend** (JR-2). The process:

```
1. Sentinel (daily) runs `pip check`
2. Tool parses output → classifies draggers by severity
3. Samples written to journal
4. If dead_count > 0:
   a. Event emitted with structured detail
   b. `russell digest` shows "Dependency Drag" section
   c. `russell jack` includes drag detail in SOAP Objective
5. Jack recommends: "ontolearner is pinning 7 packages behind.
   Last release: 2025-11-01. Consider: pip uninstall ontolearner"
6. Operator decides and acts (or doesn't)
```

Russell does NOT uninstall packages. That's a Phase 4+ skill
requiring IDRS compliance.

## Accepted Exceptions

The following patterns are NOT dependency drag — they are
intentional engineering decisions:

1. **Framework CUDA pins** — `torch` pinning exact CUDA toolkit
   and triton versions for ABI compatibility. These update only
   when torch releases a new version.

2. **OpenTelemetry lockstep** — the otel ecosystem releases all
   packages in lockstep. A package pinning `otel-sdk<1.41` is
   waiting for its own release to bump, not dead.

3. **CalVer packages** — packages using calendar versioning
   (attrs 25.x → 26.x, cattrs, certifi) where "major bumps"
   are routine and backward-compatible.

4. **System packages** — anything in `/usr/lib/python3/dist-packages`
   is apt-owned. pip reports them as outdated but they must NOT
   be touched via pip.

Russell's tool layer encodes these exceptions:

```rust
/// Tool: classify whether a conflict is "real drag" or "accepted exception"
fn classify_conflict(blocker: &str, dep: &str, constraint: &str) -> DragClass {
    // Torch pinning CUDA/triton/setuptools → Accepted
    if blocker.starts_with("torch") && 
       (dep.starts_with("nvidia-") || dep == "triton" || dep == "setuptools") {
        return DragClass::Accepted("torch ABI pin");
    }
    // OpenTelemetry lockstep
    if blocker.starts_with("opentelemetry-") && dep.starts_with("opentelemetry-") {
        return DragClass::Accepted("otel lockstep");
    }
    // System package
    if is_system_package(blocker) {
        return DragClass::Accepted("system package");
    }
    // Default: real drag
    DragClass::Drag
}
```

## Cascade Rule

When a dragger is uninstalled, Russell re-runs `pip check` to
detect **orphans** — packages that required the removed package.
These are flagged for cascade removal. The operator confirms each
cascade step.

Example cascade from the 2026-05-07 cleanup:
```
ontolearner removed
  → (no orphans, it was a leaf)

oaklib removed (required sssom which required linkml)
  → kgcl-rdflib orphaned → removed
  → (clean)

rdflib-shim removed (pinned rdflib-jsonld==0.6.1)
  → funowl orphaned → removed
  → sparqlslurper orphaned → removed
  → pyshex orphaned → removed
  → pyshexc orphaned → removed
  → PyJSG orphaned → removed
  → ShExJSG orphaned → removed
```

## Integration with Russell Infrastructure

| Component | Role |
|---|---|
| `probes/packages.rs` | `DragDetector` adapter implementing `ProviderHealth` |
| Journal `samples` table | Stores `pkg_drag_*` counts per cycle |
| Journal `events` table | Stores structured drag detail when dead_count > 0 |
| `russell digest` | "Dependency Drag" section when counts > 0 |
| `russell jack` | SOAP Objective includes drag detail; Jack recommends removal |
| `rules.d/pkg-ecosystem.toml` | Thresholds for drag counts |

## Lessons Learned (2026-05-07 Cleanup)

Packages removed for dependency drag:

| Package | Conflicts | Pattern |
|---|---|---|
| `ontolearner` 1.4.11 | 7 | Dead: exact-pins 5+ deps, months behind |
| `unsloth-zoo` 2026.5.1 | 4 | Stale: actively developed but can't keep up with transformers 5.x |
| `fast-agent-mcp` 0.7.0 | 4 | Dead: exact-pins huggingface-hub, rich, a2a-sdk |
| `multilspy` 0.0.15 | 2 | Dead: exact-pins jedi-language-server, requests |
| `linkml` 1.10.0 | 1 | Stale: pins click<8.2 |
| `oaklib` 0.6.23 | 1 | Cascade: required removed sssom |
| `pydantic-yaml` 1.6.0 | 1 | Dead: pins ruamel.yaml<0.19 |
| `mcp-agent` 0.2.6 | 1 | Cascade: required removed pydantic-yaml |
| `mcp-decision-system` 1.0.0 | 2 | Cascade: required removed linkml + mcp-agent |
| `sssom` / `sssom-schema` | 1 | Cascade: required removed linkml |
| `rdflib-shim` 1.0.3 | 1 | Dead: exact-pins rdflib-jsonld==0.6.1 |
| Ontology chain (6 pkgs) | — | Cascade: all required removed rdflib-shim |
| `otel-distro` / `otel-exporter-prometheus` / `otel-instrumentation-fastapi` / `otel-instrumentation-asgi` | circular | Stale: pin old otel versions, blocking the rest |

**Key insight:** The worst offenders are packages that use **exact version pins** (`==X.Y.Z`) on fast-moving dependencies. This is an anti-pattern in the Python ecosystem. Packages that use **compatible release** pins (`~=X.Y` or `>=X.Y,<X+1`) cause far fewer conflicts.

## Future: Proactive Detection

Phase 3 enhancement: before `pip install --upgrade <pkg>`, Russell
could dry-run the resolution and predict which packages will become
draggers. This is the "would this upgrade break anything?" question.

Implementation: `pip install --dry-run --report - <pkg>` (pip 23+)
produces a JSON resolution report that can be parsed to detect
incoming conflicts before they happen.
