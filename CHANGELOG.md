# Changelog

All notable changes to Russell will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added — Documentation pivot (JR-1 austerity + UDQL-lite governance)

- **Principles catalog** — `docs/architecture/PRINCIPLES_CATALOG.md` —
  JR-1 through JR-7. *Though she be but little, she is fierce.*
- **Documentation standard** — `docs/standards/DOCUMENTATION_STANDARDS.md` —
  UDQL-derived governance (authority hierarchy, critical set,
  mandatory update gate, Mermaid `DIAGRAM_ALIGNMENT`, audience
  vocabulary, voice register, freshness tracking, Diataxis,
  TOGAF phase tags).
- **MVP spec** — `docs/specifications/MVP_SPEC.md` — pinned
  boundary: six read-only verbs, the help channel, the single
  proprioception vital.
- **Persistence catalog** — `docs/specifications/PERSISTENCE_CATALOG.md` —
  every byte Russell writes, named.
- **Reuse manifest** — `docs/operations/REUSE_MANIFEST.md` —
  register for copied files from `peripheral` and `slate/stack`.
- **TOGAF traceability matrix** — `docs/architecture/TOGAF_TRACEABILITY_MATRIX.md`.
- **Documentation portal** — `docs/README.md` — single
  navigation entrypoint + critical-set declaration.
- **Consolidated status** — `docs/status/CONSOLIDATED-STATUS.md` —
  Phase 0 done; Phase 1 planned.
- **THE JACK** — `docs/architecture/THE_JACK.md` — the persona
  design (Jack Russell Terrier × Jack McFarland × Rust/Linux/
  cybernetics).
- **Persona file** — `crates/russell-doctor/prompts/jack.md` —
  the LLM system prompt Jack speaks with.
- **Russell-native AGENTS.md** — the binding orientation
  document. Inherited Peripheral rules moved to
  `docs/standards/agent-operating-rules.md`.
- Directory READMEs for all `docs/` subdirectories.
- YAML frontmatter + TOGAF metadata on every authoritative
  document.

### Changed

- **Architecture scope retreated to JR-1 austerity.** Seven
  ADRs moved to `docs/adr/deferred/` (0003, 0005, 0007, 0009,
  0010, 0012, 0014); their subjects are outside the MVP
  boundary but remain **Accepted** — they ship this way when
  their phase opens.
- **MCP surface and full proprioception design archived** to
  `docs/archive/` with provenance entries in
  `docs/archive/README.md`. The active ADR-0015 preserves the
  one-vital MVP proprioception.

### Added — Phase 0 Rust skeleton (prior)

- Cargo workspace with seven crates per ADR-0013 (`russell-core`,
  `russell-sentinel`, `russell-skills`, `russell-doctor`,
  `russell-proprio`, `russell-mcp`, `russell-cli`). Non-core
  crates are Phase-0 placeholders.
- `russell-core` — `paths`, `event` (`harness.event.v1`),
  `profile` (`russell.profile.v1`), `journal` (SQLite + WAL +
  numbered migrations), `telemetry`, `time`, `error`.
- `russell-sentinel` — three `/proc`-based probes
  (`mem_available_mib`, `swap_used_mib`, `loadavg_1m`).
- `russell-cli` — five read-only verbs: `status`, `list`,
  `profile [--init]`, `digest [--since-hours]`, `sentinel-once`.
- 22 unit tests passing. `cargo fmt --check`,
  `cargo clippy -- -D warnings`, `cargo test` all green.

### Added — Documentation scaffold (prior)

- Foundational ADRs (0001–0015), AGENTS.md, CONTRIBUTING.md,
  architecture overview, MCP surface (since archived), proprioception
  design (since archived), templates.

### Next

- **Phase 1 — MVP Doctor.** Implement `russell help` by copying
  `stack-llm` per `REUSE_MANIFEST.md` §4.1, authoring ADR-0016
  (Doctor and LLM router) and ADR-0017 (Reuse-over-dependency),
  adding migration `0002_help_sessions.sql`, wiring the env
  loader, and writing the offline fallback. Then `cargo test`
  through the mock backend.

[Unreleased]: https://example.invalid/russell/compare/HEAD...HEAD
