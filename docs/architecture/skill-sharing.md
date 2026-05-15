---
title: "Skill Sharing — Minimum Viable Architecture"
audience: [architects, developers]
last_updated: 2026-05-15
togaf_phase: "C"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Application Architecture -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-15 -->

# Skill Sharing — Minimum Viable Architecture

> Preserves JR-1 (austerity), JR-6 (reuse-don't-depend), and ADR-0025 §8 (remote deferral is liftable with justification).
> Version: 1.0.0 | 2026-05-15

---

## SkillBundle Format

A `.rsk.tar.gz` archive is the canonical unit of sharing. It is both the import and export format — single archive, single skill.

### Archive Layout

```
<skill-id>.rsk.tar.gz
  ├── manifest.yaml       ← YAML skill manifest (validated, poka-yoke enforced)
  ├── KNOWLEDGE.md        ← domain knowledge (optional, content-scanned)
  ├── scripts/            ← executable scripts referenced in manifest
  │   ├── probe-*.sh
  │   └── intervention-*.sh
  ├── provenance.json     ← upstream git SHA + build timestamp
  └── REUSE_MANIFEST.md   ← copy-with-provenance header (JR-6)
```

### Provenance Chain (`provenance.json`)

```json
{
  "skill_id": "okapi-watcher",
  "version": "0.1.0",
  "upstream_repo": "https://github.com/russell-harness/skills",
  "upstream_sha": "67a13834d8af...",
  "exported_at": "2026-05-15T09:00:00",
  "exported_by": "workstation-7",
  "russell_version": "0.2.0"
}
```

The `REUSE_MANIFEST.md` in each bundle carries the upstream provenance line
so JR-6 (reuse-don't-depend) is traceable across operator boundaries:

```markdown
| Russell skill | Upstream | Upstream commit | Changes |
|---|---|---|---|
| okapi-watcher | https://github.com/russell-harness/skills | 67a13834d8af... | exported by workstation-7 |
```

---

## Visibility Discriminant

Each skill carries a `visibility` tag controlling its shareability:

| Visibility | Exportable? | Registry Publish? | Description |
|---|---|---|---|
| `Local` | **No** | No | Never leaves the machine. Default for workshop-created skills. |
| `Shared` | **Yes** | No | Bundled for operator-curated distribution (USB, email, local network). |
| `Published` | **Yes** | Deferred | Tagged for potential registry push when remote registries are implemented. |

### Setting visibility

```
# Workshop: create skill with visibility
russell workshop
> create my-skill --visibility shared

# CLI: change visibility
russell skill set-visibility my-skill shared

# Default: workshop skills are Local; bundled skills are Shared.
```

---

## Import Pipeline

```
.rsk.tar.gz → tar extraction → SafetyScan::scan()
    ├── Block? → reject (ImportError::SafetyBlock)
    ├── Already exists? → ImportError::AlreadyExists (unless --force)
    └── Pass → write to skills/ directory
              → upsert RegistryCache entry with source=Manual
              → journal transition: skill.installed
```

### CLI

```bash
russell skill import okapi-watcher.rsk.tar.gz
russell skill import okapi-watcher.rsk.tar.gz --force   # overwrite existing
```

---

## Export Pipeline

```
russell skill export <id>
    ├── Visibility::Local? → reject (ExportError::NotExportable)
    ├── Skill not found? → ExportError::NotFound
    └── Read manifest + KNOWLEDGE.md + scripts/
        → Build Provenance from REUSE_MANIFEST.md + env
        → Pack as .rsk.tar.gz
        → Output to ~/.local/share/harness/exports/<id>.rsk.tar.gz (default)
```

### CLI

```bash
russell skill export okapi-watcher
russell skill export okapi-watcher --output /tmp/okapi-watcher.rsk.tar.gz
```

---

## Privacy Boundaries

1. **Local visibility is the default.** Workshop-created and manually installed skills default to `Local`. Operator must explicitly opt into sharing.
2. **No automatic export.** `russell sentinel` and `russell chat` never export bundles. Export is a manual CLI verb.
3. **No telemetry in bundles.** `RegistryEntry` telemetry (`probe_runs`, `avg_probe_duration_ms`) is NOT included in exported bundles. Bundles carry only structural content (manifest, knowledge, scripts, provenance).
4. **Provenance is metadata-only.** The `provenance.json` contains only export-time metadata and upstream source information. No host telemetry, no journal data, no profile data.
5. **Shared bundles carry a REUSE_MANIFEST.md.** This makes the provenance chain visible to the importing operator, satisfying JR-6 audit requirements without exposing internal telemetry.

---

## Cross-Ecosystem Compatibility

A `SkillBundle` produced by Russell's `russell skill export` can be imported by
any Kask ecosystem tool that understands the manifest schema:

```
Russell (export .rsk.tar.gz) → Kask (import via equivalent loader)
Kask (export .rsk.tar.gz)    → Russell (import via FilesystemSkillLoader)
```

The contract is the manifest schema (YAML with `id`, `version`, `authored`,
`symptoms`, `probes`, `interventions`, `safety`, and `evaluation` sections).
The bundle format (tar.gz) is a transport mechanism, not a protocol boundary.
