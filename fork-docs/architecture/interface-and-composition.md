---
title: "Russell Interface and Composition"
audience: [architects, developers, agents]
last_updated: 2026-05-25
version: "1.0.0"
status: "Active"
domain: "Cross-cutting"
ddmvss_categories: [interface, composition]
---

# Russell Interface and Composition

**Purpose:** Define Russell's interface surfaces (CLI, ACP, systemd) and composition mechanisms (skill manifests, dispatcher).

**Focusing assumption:** `CLI ≡ ACP ≡ systemd` — three projections of one core.

---

## 1. Interface Surfaces

### 1.1 Surface Inventory

| Surface | Technology | Use Case |
|---------|-----------|----------|
| `CLI` | Rust binary (`russell`) | Operator interaction, scripting |
| `ACP` | JSON-RPC 2.0 over stdio | Agent integration (hKask) |
| `systemd` | User units | Background operation |

### 1.2 Interface Equivalence Matrix

| Capability | CLI | ACP | systemd |
|------------|-----|-----|---------|
| Run sentinel | `russell sentinel-once` | `acp/probe/run` | `russell-sentinel.timer` |
| Query journal | `russell list` | `acp/journal/query` | N/A |
| Run skill | `russell skill run <id>` | `acp/skill/run` | N/A |
| Install skill | `russell skill install <id>` | N/A | N/A |
| Prune skill | `russell skill prune <id>` | N/A | N/A |
| Retire skill | `russell skill retire <id>` | N/A | N/A |
| Self-triage | `russell self-triage` | N/A | N/A |
| Consult Jack | `russell jack` | `acp/jack/consult` | N/A |
| Chat with Jack | `russell chat` | `acp/session/create` | N/A |
| Export evidence | `russell digest` | N/A | N/A |

**Focusing assumption:** All three surfaces exercise the same functional core. No surface has exclusive capabilities (except where noted).

---

## 2. CLI Interface

### 2.1 Command Inventory

| Command | Purpose | Risk Band |
|---------|---------|-----------|
| `russell status` | Show current state | none |
| `russell list` | Query journal | none |
| `russell sentinel-once` | Run sentinel cycle | none |
| `russell jack` | Consult Jack (LLM) | none |
| `russell chat` | Interactive Jack session | none |
| `russell skill list` | List installed skills | none |
| `russell skill run <id>` | Run skill | varies |
| `russell skill install <id>` | Install skill | low |
| `russell skill prune <id>` | Prune skill | low |
| `russell skill retire <id>` | Retire skill | medium |
| `russell self-triage` | Self-diagnosis | none |
| `russell digest` | Export evidence | none |
| `russell verify-journal` | Verify hash chain | none |

### 2.2 CLI Architecture

```
russell-cli (binary)
├── commands/
│   ├── status.rs
│   ├── list.rs
│   ├── sentinel_once.rs
│   ├── jack.rs
│   ├── chat.rs
│   ├── skill/
│   │   ├── list.rs
│   │   ├── run.rs
│   │   ├── install.rs
│   │   ├── prune.rs
│   │   └── retire.rs
│   ├── self_triage.rs
│   ├── digest.rs
│   └── verify_journal.rs
└── main.rs
```

**Dependency:** `russell-cli` depends on all other crates. No other crate depends on `russell-cli`.

---

## 3. ACP Interface

### 3.1 Method Inventory

| Method | Purpose | Risk Band |
|--------|---------|-----------|
| `acp/capabilities` | List capabilities | none |
| `acp/probe/run` | Run probe | none |
| `acp/journal/query` | Query journal | none |
| `acp/skill/run` | Run skill | varies |
| `acp/jack/consult` | Consult Jack | none |
| `acp/session/create` | Create chat session | none |
| `acp/session/message` | Send message | none |
| `acp/session/close` | Close session | none |
| `acp/consent/respond` | Respond to consent request | none |

### 3.2 ACP Architecture

```
russell-acp-server (binary)
├── handler.rs (JSON-RPC dispatcher)
├── session.rs (session management)
├── auth.rs (macaroon validation)
└── main.rs
```

**Transport:** JSON-RPC 2.0 over stdio.

**Authentication:** Macaroon-based OCAP tokens.

---

## 4. systemd Interface

### 4.1 Unit Inventory

| Unit | Type | Purpose |
|------|------|---------|
| `russell-sentinel.timer` | Timer | Trigger sentinel every 5 minutes |
| `russell-sentinel.service` | Service | Run sentinel cycle |
| `russell-digest.timer` | Timer | Trigger weekly digest |
| `russell-digest.service` | Service | Generate weekly digest |
| `russell-failure@.service` | Template | Capture failure context |
| `russell-acp-server.service` | Service | Run ACP server |

### 4.2 systemd Architecture

```
~/.config/systemd/user/
├── russell-sentinel.timer
├── russell-sentinel.service
├── russell-digest.timer
├── russell-digest.service
├── russell-failure@.service
└── russell-acp-server.service
```

**Activation:** `systemctl --user enable --now russell-sentinel.timer`

**Logs:** `journalctl --user -u russell-sentinel.service`

---

## 5. Composition Mechanisms

### 5.1 Skill Manifest Schema

```yaml
id: <skill-id>
version: <semver>
authored: <ISO 8601>
symptoms:
  - <symptom-class>
applies_when:
  os_family: linux
probes:
  - id: <probe-id>
    cmd: ["bash", "./scripts/<probe-id>.sh"]
    risk: none
    timeout: 30s
interventions:
  - id: <intervention-id>
    cmd: ["bash", "./scripts/<intervention-id>.sh"]
    risk: low
    idempotent: true
    rollback: none_needed
safety:
  max_auto_risk: low
  allowed_env_keys: ["HOME", "LANG", "PATH"]
  needs_network: false
```

### 5.2 Skill Composition Rules

| Rule | Description |
|------|-------------|
| **No skill invocation** | Skills cannot invoke other skills |
| **No code sharing** | Each skill is self-contained |
| **Manifest-only** | Composition is declarative, not imperative |
| **IDRS compliance** | All interventions satisfy IDRS contract |

**Rationale:** Skill composition is intentionally limited to prevent cascading failures and maintain auditability.

### 5.3 Dispatcher Rules

| Rule | Description |
|------|-------------|
| **Manifest validation** | Reject manifests that fail schema validation |
| **Risk enforcement** | Block interventions above `max_auto_risk` without consent |
| **IDRS enforcement** | Require IDRS compliance for all interventions |
| **Timeout enforcement** | Kill processes that exceed timeout |
| **Rollback enforcement** | Execute rollback on intervention failure |

---

## 6. Focusing Assumptions

### FA-I1: CLI ≡ ACP ≡ systemd — Three Projections of One Core

**Statement:** All three surfaces exercise the same functional core.

**Rationale:** Collapses entire UX specification dimension. No surface has exclusive capabilities (except where noted).

**Evidence:** See §1.2 Interface Equivalence Matrix.

---

### FA-Co1: One Registry, No Composition

**Statement:** Skills are registered in a flat registry. Skills cannot compose with other skills.

**Rationale:** Prevents cascading failures and maintains auditability. Composition is declarative (manifest), not imperative (code).

**Evidence:** See §5.2 Skill Composition Rules.

---

## 7. Cross-References

| Category | Relation |
|----------|----------|
| Capability | Interfaces surface capabilities |
| Composition | Registry discoverable through all surfaces |
| Trust | Interfaces enforce risk bands |
| Observability | Interfaces emit CNS spans |

---

## 8. Completeness Checklist

- [x] Every capability has all three surface entries
- [x] Equivalence matrix covers all verbs
- [x] Auth model consistent across surfaces
- [x] Registry schema defined
- [x] Composition rules documented
- [x] Dispatcher rules documented

---

## References

- hKask DDMVSS: `~/Clones/hKask/docs/architecture/DDMVSS.md` §5.3, §5.4
- ADR-0003 (MCP transport)
- ADR-0027 (ACP integration)
- ADR-0049 (three-surface interaction)
