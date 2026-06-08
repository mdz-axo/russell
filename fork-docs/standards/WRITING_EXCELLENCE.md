---
title: "Russell Writing Excellence Protocol"
audience: [contributors, maintainers]
last_updated: 2026-05-25
version: "1.0.0"
status: "Active"
domain: "Cross-cutting"
ddmvss_categories: [curation]
---

# Russell Writing Excellence Protocol

**Purpose:** Define the quality standards for Russell's documentation.

**Axiom:** *Good documentation is discovered, not invented.*

---

## 1. Writing Principles

### 1.1 Clarity Over Cleverness

- Prefer simple words over complex ones
- Prefer short sentences over long ones
- Prefer concrete examples over abstract descriptions
- Avoid jargon unless defined in vocabulary

### 1.2 Precision Over Brevity

- Define terms explicitly
- Specify units (seconds, bytes, percent)
- Use exact values, not approximations
- Cite sources for claims

### 1.3 Consistency Over Creativity

- Use Russell's vocabulary consistently
- Follow frontmatter standard exactly
- Use consistent formatting for code, links, tables
- Match existing document structure

---

## 2. Document Structure

### 2.1 Required Sections

Every document MUST include:

1. **Frontmatter** — YAML metadata
2. **Purpose** — One-sentence summary
3. **Contents** — Table of contents (for documents > 100 lines)
4. **Body** — Main content
5. **References** — Citations and links

### 2.2 Optional Sections

Documents MAY include:

- **Axioms** — Fundamental assumptions
- **Focusing assumptions** — Simplifying constraints
- **Completeness checklist** — Verification criteria
- **Cross-references** — Links to related documents
- **Examples** — Concrete illustrations

### 2.3 Section Ordering

Follow this order when applicable:

1. Frontmatter
2. Purpose
3. Axioms (if any)
4. Contents
5. Main sections (numbered)
6. Focusing assumptions
7. Cross-references
8. Completeness checklist
9. References

---

## 3. Writing Quality Checks

### 3.1 Readability

- **Flesch-Kincaid grade level:** ≤ 12 (high school)
- **Sentence length:** ≤ 25 words average
- **Paragraph length:** ≤ 5 sentences
- **Passive voice:** ≤ 10% of sentences

### 3.2 Terminology

- **Vocabulary coverage:** All Russell terms defined
- **Consistency:** Same term used throughout (no synonyms)
- **First use:** Define term on first use if not in vocabulary

### 3.3 Code Examples

- **Language tags:** All code blocks tagged (```rust, ```yaml, ```bash)
- **Completeness:** Examples compile / run as written
- **Comments:** Explain non-obvious code
- **Formatting:** Consistent indentation and style

### 3.4 Links

- **Internal links:** Relative paths, verified to exist
- **External links:** Absolute URLs, verified to resolve
- **Anchor links:** Verified to exist in target document
- **No broken links:** All links checked before commit

---

## 4. Review Process

### 4.1 Self-Review Checklist

Before submitting a document for review:

- [ ] Frontmatter complete and valid
- [ ] Purpose statement clear and concise
- [ ] All sections present and ordered correctly
- [ ] Terminology consistent with vocabulary
- [ ] Code examples compile / run
- [ ] All links verified
- [ ] Diagrams have alignment metadata
- [ ] No spelling or grammar errors
- [ ] Readability metrics met

### 4.2 Peer Review Criteria

Reviewers should verify:

- **Accuracy:** Claims are correct and cited
- **Completeness:** No missing sections or information
- **Clarity:** Document is understandable to target audience
- **Consistency:** Follows standards and vocabulary
- **Correctness:** Code examples work as described

### 4.3 Approval Process

1. Author submits PR with self-review checklist
2. Reviewer verifies peer review criteria
3. Reviewer approves or requests changes
4. Author addresses feedback
5. Reviewer approves
6. PR merged

---

## 5. Common Patterns

### 5.1 Defining a Term

```markdown
**Term:** Definition.

Example usage in context.
```

### 5.2 Describing a Capability

```markdown
### Capability Name

**Purpose:** One-sentence description.

**Resource:** `resource:{id}`

**Action:** `verb`

**Risk band:** none | low | medium | high | critical

**IDRS:** Yes | No

**Example:**
```bash
russell command --args
```
```

### 5.3 Documenting a Schema

```markdown
### Schema Name

```sql
CREATE TABLE table_name (
  column1 TYPE NOT NULL,
  column2 TYPE,
  PRIMARY KEY (column1)
);
```

**Purpose:** What this table stores.

**Retention:** How long data is kept.

**Example:**
```json
{
  "column1": "value1",
  "column2": "value2"
}
```
```

### 5.4 Describing a Lifecycle

```markdown
### Lifecycle Name

```
State1 → State2 → State3
```

**State transitions:**
- **State1 → State2:** Trigger condition
- **State2 → State3:** Trigger condition

**Commands:**
- `russell command1` → State1 → State2
- `russell command2` → State2 → State3
```

---

## 6. Anti-Patterns

### 6.1 Avoid These Patterns

| Anti-Pattern | Problem | Solution |
|--------------|---------|----------|
| **Wall of text** | Hard to scan | Use headings, lists, tables |
| **Undefined jargon** | Confuses readers | Define or link to vocabulary |
| **Inconsistent terms** | Ambiguous | Use vocabulary consistently |
| **Broken links** | Dead ends | Verify all links |
| **Outdated examples** | Misleading | Update or remove |
| **Missing frontmatter** | No metadata | Add required fields |
| **No purpose statement** | Unclear intent | Add one-sentence summary |
| **Passive voice overuse** | Weak writing | Use active voice |

### 6.2 Common Mistakes

**Mistake:** "The journal is written by Russell."  
**Correction:** "Russell writes to the journal."

**Mistake:** "Probes are executed every 5 minutes."  
**Correction:** "Russell executes probes every 5 minutes."

**Mistake:** "The skill can be run using the `russell skill run` command."  
**Correction:** "Run the skill with `russell skill run <id>`."

---

## 7. Tools and Resources

### 7.1 Writing Tools

- **Grammar checker:** Grammarly, LanguageTool
- **Readability analyzer:** Hemingway Editor
- **Spell checker:** aspell, hunspell
- **Link checker:** markdown-link-check

### 7.2 Style Guides

- **Diátaxis:** https://diataxis.fr/
- **Google Developer Documentation Style Guide:** https://developers.google.com/style
- **Microsoft Writing Style Guide:** https://docs.microsoft.com/en-us/style-guide/

### 7.3 Russell Resources

- **Vocabulary:** `architecture/reference/vocabulary.md`
- **Examples:** Existing documents in corpus
- **Templates:** `standards/ADR_TEMPLATE.md`

---

## 8. Continuous Improvement

### 8.1 Feedback Loop

1. Reader identifies issue (typo, unclear, outdated)
2. Reader files issue or submits PR
3. Maintainer reviews and merges
4. Document improved

### 8.2 Metrics

Track these metrics quarterly:

- **Broken links:** Count of broken internal/external links
- **Stale documents:** Documents not updated in 180+ days
- **Orphaned documents:** Documents not linked from index
- **Vocabulary drift:** Terms used inconsistently

### 8.3 Retrospectives

Quarterly retrospective:
- What documentation worked well?
- What documentation caused confusion?
- What patterns should we adopt?
- What anti-patterns should we avoid?

---

## 9. References

- Writing Excellence (see standards/WRITING_EXCELLENCE.md)
- Diátaxis: https://diataxis.fr/
- Strunk & White. *The Elements of Style*.
