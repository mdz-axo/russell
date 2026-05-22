# Russell — Cybernetic Health Harness for hKask

**Version:** 1.0.0  
**Status:** Active — Documentation Refresh Complete  
**Last Updated:** 2026-05-22  
**TOGAF Phase:** G — Governance

---

## What Russell Is

Russell is a **cybernetic health harness** for a single Linux AI/ML workstation, operating as an **ACP (Agent Client Protocol) agent** integrated with hKask (separate repository).

**Documentation Corpus:** 79 active files (2026-05-22 refresh)

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                         hKask Platform                          │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐ │
│  │ MCP Servers │  │   Agents    │  │  Bitemporal Store       │ │
│  │ (193 tools) │  │ (Ensemble)  │  │  (PostgreSQL/Redis)     │ │
│  └──────┬──────┘  └──────┬──────┘  └───────────┬─────────────┘ │
│         │                │                      │                │
│         └────────────────┼──────────────────────┘                │
│                          │ ACP (JSON-RPC)                        │
└──────────────────────────┼───────────────────────────────────────┘
                           │ stdio
┌──────────────────────────┼───────────────────────────────────────┐
│                    Russell ACP Server                            │
│  ┌────────────────────────┼────────────────────────────────┐    │
│  │  Jack Persona (LLLM)  │  Public Skills (8)             │    │
│  │  - Okapi backend      │  - journal-viewer              │    │
│  │  - SOAP prompts       │  - web-search                  │    │
│  │  - Nurse voice        │  - scenario-tester             │    │
│  │                       │  - package-checker             │    │
│  │  Session Manager      │  - ubuntu-jack                 │    │
│  │  - Multi-turn state   │  - journal-compactor           │    │
│  │  - Turn records       │  - pragmatic-*                 │    │
│  │                       │                                 │    │
│  │  Security Boundary    │  Private Skills (6)            │    │
│  │  - Macaroon auth      │  - okapi-watcher               │    │
│  │  - Rate limiter       │  - sysadmin                    │    │
│  │  - Visibility filter  │  - skill-*                     │    │
│  └────────────────────────┼────────────────────────────────┘    │
│                           │                                     │
│  ┌────────────────────────┴─────────────────────────────────┐   │
│  │              SQLite Journal (Local)                       │   │
│  │  - Host telemetry (5-min cadence)                         │   │
│  │  - Evidence bundles (IDRS)                                │   │
│  │  - Proprioception vitals (private)                        │   │
│  └───────────────────────────────────────────────────────────┘   │
└───────────────────────────────────────────────────────────────────┘
                           │
┌──────────────────────────┼───────────────────────────────────────┐
│              Russell Sentinel (systemd timer)                    │
│  - 5-minute probe cadence                                        │
│  - 23 host samples per cycle                                     │
│  - Threshold evaluation (RuleSet)                                │
│  - Journal writes (harness.event.v1)                             │
└───────────────────────────────────────────────────────────────────┘
```

### Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| **ACP primary interface** | hKask integration, bidirectional capability access |
| **Hybrid deployment** | ACP server (on-demand) + Sentinel timer (5-min) |
| **Visibility boundary** | 8 public skills (read-only) / 6 private (mutations) |
| **Independent journal** | Russell operates during hKask outages |
| **Proprioception private** | 5 self-vitals never exposed (security) |
| **Macaroon OCAP** | Fine-grained capability attenuation |

---

## Quick Start

### Prerequisites

- Rust toolchain (`cargo --version`)
- Okapi running (`systemctl --user status okapi`)
- hKask configured (optional, for ACP integration)

### Install

```bash
# Install binaries
./docs/deployment/install.sh

# Configure macaroon auth
./docs/deployment/macaroon-setup.sh
source ~/.bashrc

# Enable services
systemctl --user enable --now russell-sentinel.timer
systemctl --user enable --now russell-acp-server.service
```

### Verify

```bash
# Check services
systemctl --user status russell-acp-server.service
systemctl --user list-timers | grep russell

# Run tests
./docs/deployment/test-acp-integration.sh
```

---

## Documentation

### Core Documents

| Document | Purpose | TOGAF Phase |
|----------|---------|-------------|
| [AGENTS.md](../AGENTS.md) | Contributor orientation | Preliminary |
| [cybernetic-health-harness.md](../cybernetic-health-harness.md) | Canonical design | A — Vision |
| [MACHINE_PROFILE.md](../MACHINE_PROFILE.md) | Observed machine | G — Governance |
| [CONSOLIDATED-STATUS.md](status/CONSOLIDATED-STATUS.md) | Single source of truth | G — Governance |

### Architecture

| Document | Purpose | TOGAF Phase |
|----------|---------|-------------|
| [PRINCIPLES_CATALOG.md](architecture/PRINCIPLES_CATALOG.md) | JR-1 through JR-7 | Preliminary |
| [THE_JACK.md](architecture/THE_JACK.md) | Nurse persona spec | A — Vision |
| [overview.md](architecture/overview.md) | Architecture overview | C — Application |
| [ecosystem-integration.md](architecture/ecosystem-integration.md) | hKask integration | C — Application |
| [skill-self-management-strategy.md](architecture/skill-self-management-strategy.md) | Skill lifecycle design | C — Application |
| [ADR Index](adr/0001-scope-and-charter.md) | Architecture decisions | H — Change |

### Specifications

| Document | Purpose | TOGAF Phase |
|----------|---------|-------------|
| [MVP_SPEC.md](specifications/MVP_SPEC.md) | MVP boundary | Requirements |
| [PERSISTENCE_CATALOG.md](specifications/PERSISTENCE_CATALOG.md) | Data stores | C — Data |
| [disk-pkg-hygiene](specifications/disk-pkg-hygiene/00-semantic-decomposition.md) | Disk/package hygiene | C — Application |

### Standards

| Document | Purpose | TOGAF Phase |
|----------|---------|-------------|
| [DOCUMENTATION_STANDARDS.md](standards/DOCUMENTATION_STANDARDS.md) | Documentation governance | Preliminary |
| [TOGAF_LITE_FOR_OPEN_SOURCE.md](standards/TOGAF_LITE_FOR_OPEN_SOURCE.md) | TOGAF-Lite pattern | Preliminary |
| [WRITING_EXCELLENCE.md](standards/WRITING_EXCELLENCE.md) | Writing rubric | Preliminary |
| [VALIDATION_RUBRIC.md](standards/VALIDATION_RUBRIC.md) | Validation standard | Preliminary |
| [agent-operating-rules.md](standards/agent-operating-rules.md) | Agent rules | Preliminary |
| [coding-rust.md](standards/coding-rust.md) | Rust coding standard | Preliminary |
| [commits.md](standards/commits.md) | Commit standard | Preliminary |
| [hkask-integration.md](standards/hkask-integration.md) | hKask integration | Preliminary |
| [safety.md](standards/safety.md) | IDRS contract | Preliminary |
| [skill-building-rules.md](standards/skill-building-rules.md) | Skill development | Preliminary |

### Deployment & Operations

| Document | Purpose | TOGAF Phase |
|----------|---------|-------------|
| [QUICKSTART.md](deployment/QUICKSTART.md) | 5-minute setup | G — Governance |
| [INSTALL.md](deployment/INSTALL.md) | Full installation guide | G — Governance |
| [acp-integration.md](deployment/acp-integration.md) | hKask integration | G — Governance |
| [REUSE_MANIFEST.md](operations/REUSE_MANIFEST.md) | Reuse manifest | D — Technology |

### Reference

| Document | Purpose | TOGAF Phase |
|----------|---------|-------------|
| [cli.md](reference/cli.md) | CLI reference | G — Governance |

### Templates

| Document | Purpose | TOGAF Phase |
|----------|---------|-------------|
| [adr-template.md](templates/adr-template.md) | ADR template | H — Change |
| [daily-log.md](templates/daily-log.md) | Daily log template | G — Governance |
| [review-entry.md](templates/review-entry.md) | Review template | G — Governance |
| [soap-bundle.md](templates/soap-bundle.md) | SOAP template | G — Governance |

---

## Crate Structure

| Crate | Purpose | TOGAF Phase |
|-------|---------|-------------|
| `russell-acp-server` | ACP session interface | D — Technology |
| `russell-cli` | Local CLI (secondary) | D — Technology |
| `russell-core` | Base types, journal, rules | C — Application |
| `russell-mcp` | hKask MCP client | C — Application |
| `russell-meta` | Jack persona, LLM client | C — Application |
| `russell-proprio` | Proprioception (5 vitals) | C — Application |
| `russell-sentinel` | Host probe collection | C — Application |
| `russell-skills` | Skill manifest loader | C — Application |
| `russell-testing` | Test fixtures | G — Governance |

---

## Security Model

### Boundaries

| Boundary | Enforcement |
|----------|-------------|
| Public/Private skills | `visibility` field filter at dispatch |
| Proprioception | Never added to ACP capability registry |
| Interventions | Require upstream consent (hKask or CLI) |
| Macaroon auth | OCAP validation with caveats |
| Rate limiting | 100 calls/min/token |

### JR Principles

| Principle | Implementation |
|-----------|----------------|
| **JR-1** (Austere) | Minimal crate changes, delete before add |
| **JR-2** (Observe > Act) | Public skills read-only |
| **JR-3** (No LLM shell) | LLM ranks IDs, doesn't emit commands |
| **JR-4** (Nurse present) | Jack persona via ACP sessions |
| **JR-5** (Proprioception) | 5 vitals retained, never exposed |
| **JR-6** (Reuse) | Independent SQLite journal |
| **JR-7** (Auditable) | ACP calls logged to journal |

---

## Development

### Build

```bash
cargo check                    # Type check
cargo test                     # Run tests
cargo clippy -- -D warnings    # Lint
cargo fmt --check              # Format check
```

### Key Commands

```bash
cargo run -- sentinel-once     # Fire one observe cycle
cargo run -- verify-journal    # Audit hash chain
cargo run -- skill list        # List skills
cargo run -- chat              # Interactive REPL with Jack
```

### Adding Skills

See [Skill Building Rules](standards/skill-building-rules.md) for:
- Manifest structure
- Path validation rules
- IDRS contract requirements

---

## Status

| Component | Status | TOGAF Phase |
|-----------|--------|-------------|
| ACP Server | ✅ Deployed | G — Governance |
| Sentinel | ✅ 5-min cadence | G — Governance |
| Skills | ✅ 12 loaded | G — Governance |
| hKask Integration | ✅ Config ready | G — Governance |
| Tests | ✅ 218 passing | G — Governance |
| Documentation | ✅ 79 active files | G — Governance |

**Last Deployment:** 2026-05-22  
**Documentation Refresh:** 2026-05-22 (35 files archived, 79 retained)  
**Next Review:** After bidirectional ACP testing

---

## Archive

Superseded documents are archived per lifecycle policy. Git history is the canonical archive of record.

- **Archive location:** [`docs/archive/2026-05-22-documentation-refresh/`](archive/2026-05-22-documentation-refresh/README.md)
- **Archived:** 35 files (phase logs, analysis, superseded proposals)
- **Retained:** 79 files (active specifications, standards, architecture)

To recover an archived document:

```bash
# Find deletion commit
git log --diff-filter=D -- docs/<path>

# Restore from history
git checkout <sha> -- docs/<path>
```

---

## References

- [ACP Specification](https://agentclientprotocol.com)
- [Macaroons](https://github.com/macaroon-v2/spec)
- [TOGAF Standard](https://www.opengroup.org/togaf)
- [Rust Programming Language](https://www.rust-lang.org)
