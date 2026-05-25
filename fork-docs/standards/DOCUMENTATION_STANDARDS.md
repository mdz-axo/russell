---
title: "Russell Documentation Standards"
audience: [contributors, maintainers]
last_updated: 2026-05-25
version: "1.0.0"
status: "Active"
domain: "Cross-cutting"
ddmvss_categories: [curation]
---

# Russell Documentation Standards

**Purpose:** Define how Russell's documentation is structured, maintained, and curated.

**Axiom:** *Documentation is code. Treat it accordingly.*

---

## 1. Documentation Structure

### 1.1 DDMVSS Corpus (19 Documents)

Russell's documentation follows hKask's DDMVSS framework:

**Framework documents (4):**
- `architecture/russell-architecture-master.md` — Index
- `architecture/DDMVSS.md` — Framework taxonomy
- `architecture/PRINCIPLES.md` — JR-1 through JR-7
- `architecture/magna-carta.md` — Operator sovereignty

**Spec documents (4):**
- `architecture/domain-and-capability.md` — Bounded contexts, verbs
- `architecture/interface-and-composition.md` — CLI/ACP/systemd, skills
- `architecture/trust-security-observability.md` — IDRS, risk bands, journal
- `architecture/persistence-and-lifecycle.md` — Storage, lifecycle

**Standards documents (4):**
- `standards/DOCUMENTATION_STANDARDS.md` — This document
- `standards/WRITING_EXCELLENCE.md` — Writing quality protocol
- `standards/DEPENDENCY_POLICY.md` — Dependency policy
- `standards/ADR_TEMPLATE.md` — ADR template

**Operational documents (7):**
- `DDMVSS_SCAFFOLD.md` — DDMVSS scaffold
- `OPEN_QUESTIONS.md` — Unresolved aspects
- `CI-CD-GUIDE.md` — CI/CD guide
- `DEPLOYMENT.md` — Deployment guide
- `plans/TODO.md` — Open work
- `status/PROJECT_STATUS.md` — Single source of truth
- `specifications/REQUIREMENTS.md` — Goal specifications

### 1.2 Reference Documents

Located in `architecture/reference/`:
- `russell-erd.md` — Entity relationship diagrams
- `subsystem-erds.md` — Per-subsystem ERDs
- `vocabulary.md` — Russell vocabulary
- `ports-inventory.md` — Hexagonal port inventory
- `jack-persona.md` — Jack persona specification
- `okapi-integration.md` — Okapi LLM integration
- `hkask-integration.md` — hKask ACP integration

### 1.3 Decision Records

Located in `decisions/`:
- Active ADRs: `decisions/NNNN-*.md`
- Deferred ADRs: `decisions/deferred/NNNN-*.md`

---

## 2. Frontmatter Standard

Every document MUST include YAML frontmatter:

```yaml
---
title: "Document Title"
audience: [architects, developers, agents]
last_updated: YYYY-MM-DD
version: "MAJOR.MINOR.PATCH"
status: "Active | Draft | Deprecated | Superseded"
domain: "Cross-cutting | specific domain"
ddmvss_categories: [category1, category2, ...]
---
```

**Valid DDMVSS categories:**
- `domain` — Bounded contexts, vocabulary
- `capability` — Verbs, grants
- `interface` — CLI, ACP, systemd
- `composition` — Skills, manifests
- `trust` — IDRS, risk bands, security
- `observability` — Journal, proprioception, CNS spans
- `persistence` — Storage, schemas
- `lifecycle` — Bootstrap, evolution, deprecation
- `curation` — Documentation standards, writing quality

---

## 3. Document Lifecycle

### 3.1 Status Transitions

```
Draft → Active → Deprecated → Superseded → Removed
```

- **Draft:** Document is under review
- **Active:** Document is current and authoritative
- **Deprecated:** Document is outdated but retained for reference
- **Superseded:** New document replaces this one
- **Removed:** Document deleted from repository

### 3.2 Versioning

Documentation follows SemVer:
- **MAJOR:** Breaking changes (restructured, removed sections)
- **MINOR:** Additive changes (new sections, new content)
- **PATCH:** Corrective changes (typos, clarifications)

### 3.3 Update Process

1. Edit document
2. Update `last_updated` field
3. Bump `version` field
4. Commit with message: `docs: update <document-name>`
5. PR review required for Active documents

---

## 4. Diagram Alignment

Every Mermaid diagram MUST include alignment metadata:

```markdown
<!-- DIAGRAM_ALIGNMENT
id: DIAG-<TOPIC>-<LABEL>-<NNN>
type: flowchart | sequenceDiagram | stateDiagram-v2 | gantt | erDiagram
verified_date: YYYY-MM-DD
verified_against: path/to/source
reference_sources: Author (Year) | upstream ref
status: VERIFIED | STALE | DEPRECATED
-->
```

**ID prefix convention:**
- `DIAG-ARCH-*` — Architecture diagrams
- `DIAG-DDMVSS-*` — DDMVSS diagrams
- `DIAG-PRINCIPLES-*` — Principles diagrams
- `DIAG-JOURNAL-*` — Journal diagrams
- `DIAG-SENTINEL-*` — Sentinel diagrams

---

## 5. Writing Standards

### 5.1 Voice and Tone

- **Active voice:** "Russell observes the host" not "The host is observed by Russell"
- **Present tense:** "Russell writes to the journal" not "Russell wrote to the journal"
- **Second person for operators:** "You can run `russell status`" not "The operator can run..."
- **Third person for Russell:** "Russell monitors himself" not "I monitor myself"

### 5.2 Terminology

Use Russell's vocabulary consistently:
- `sentinel` — Continuous telemetry collector
- `journal` — SQLite database with hash chain
- `jack` — Persona that consults LLM
- `skill` — YAML manifest + scripts
- `proprioception` — Self-observation
- `IDRS` — Idempotent / Dry-run / Rollback / Structured-log
- `risk-band` — none / low / medium / high / critical
- `consent` — Operator approval for interventions

See `architecture/reference/vocabulary.md` for complete vocabulary.

### 5.3 Code Examples

- Use fenced code blocks with language tags
- Prefer Rust for implementation examples
- Prefer YAML for configuration examples
- Prefer bash for CLI examples

### 5.4 Links

- Use relative links for internal documents: `[PRINCIPLES.md](PRINCIPLES.md)`
- Use absolute links for external resources: `[hKask DDMVSS](https://github.com/hkask/hkask/blob/main/docs/architecture/DDMVSS.md)`
- Use anchor links for sections: `[§3 Completeness Predicates](#3-completeness-predicates)`

---

## 6. Curation Process

### 6.1 Curation Decisions

Every document update requires a curation decision:

| Decision | Description |
|----------|-------------|
| **Merge** | Document accepted into corpus |
| **Revise** | Document returned for revision with rationale |
| **Defer** | Decision postponed — needs more information |
| **Discard** | Document rejected — does not serve corpus |

### 6.2 Coherence Metric

Documentation coherence is measured by:
- **Vocabulary coverage:** % of Russell vocabulary terms defined
- **Cross-reference saturation:** % of cross-references satisfied
- **Completeness:** % of DDMVSS categories with Active documents

**Threshold:** Coherence score ≥ 0.7

### 6.3 Curation Cadence

- **Weekly:** Review OPEN_QUESTIONS.md
- **Monthly:** Review PROJECT_STATUS.md
- **Quarterly:** Review all Active documents for staleness

---

## 7. Archival Policy

### 7.1 Git as Archive

Git history is the canonical archive. Superseded and removed documents are recoverable via:

```bash
git log --diff-filter=D -- <path>
git show <sha>:<path>
```

### 7.2 No Archive Directory

Do not create `docs/archive/` or similar. Git history is sufficient.

### 7.3 ADR Archival

Superseded ADRs remain in `decisions/` with updated frontmatter:
```yaml
status: "Superseded"
superseded_by: "ADR-MMMM"
```

---

## 8. Tooling

### 8.1 Link Checker

```bash
# Check all internal links
find docs -name '*.md' -exec grep -l '\[.*\](.*)' {} \; | xargs -I {} sh -c 'echo "Checking {}"; grep -oP '\[.*?\]\(\K[^)]+' {} | while read link; do if [[ ! "$link" =~ ^http ]]; then target=$(dirname {})/$link; if [ ! -f "$target" ]; then echo "BROKEN: $link in {}"; fi; fi; done'
```

### 8.2 Frontmatter Validator

```bash
# Validate frontmatter on all documents
for f in $(find docs -name '*.md'); do
  head -20 "$f" | grep -q 'ddmvss_categories:' || echo "MISSING ddmvss_categories: $f"
done
```

### 8.3 Diagram Validator

```bash
# Check all diagrams have alignment metadata
for f in $(find docs -name '*.md'); do
  if grep -q '```mermaid' "$f"; then
    grep -q 'DIAGRAM_ALIGNMENT' "$f" || echo "MISSING DIAGRAM_ALIGNMENT: $f"
  fi
done
```

---

## 9. References

- hKask DDMVSS: `~/Clones/hKask/docs/architecture/DDMVSS.md`
- hKask Documentation Standards: `~/Clones/hKask/docs/standards/DOCUMENTATION_STANDARDS.md`
- Diátaxis: https://diataxis.fr/
