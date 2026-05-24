---
title: "Writing Excellence Protocol"
audience: [contributors, developers, agents]
last_updated: 2026-05-24
togaf_phase: "Preliminary"
version: "1.0.0"
status: "Active"
---

# Writing Excellence Protocol

<!-- TOGAF_DOMAIN: Cross-cutting -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-24 -->

**Audience:** All contributors authoring documentation; AI agents generating or editing prose
**TOGAF Phase:** Preliminary — Framework and Principles

---

## 1. Purpose

This protocol defines the voice, style, and discipline standards for the
Russell documentation corpus. It is not an abstract style guide — it is
grounded in the work and philosophy of four women whose contributions to
technical communication define what excellence means in our field. Their
stories are not decoration; they are the operating principles that govern
how we write, what we demand of ourselves, and why clarity is a moral
obligation rather than a stylistic preference.[^schriver-readers]

---

## 2. Exemplars

### 2.1 Grace Hopper (1906–1992) — Clarity as Access

Rear Admiral Grace Hopper wrote the first computer manual — the 561-page
*A Manual of Operation for the Automatic Sequence Controlled Calculator*
(1946) — and spent her career demanding that technical systems speak the
language of their users rather than forcing users to speak the language of
machines.[^hopper-yale]

When Hopper proposed writing programs in English words rather than
mathematical symbols in 1953, she was told her idea would not work. She
built it anyway. FLOW-MATIC (1956) and its descendant COBOL proved that
accessibility does not require sacrificing precision — it requires the
writer to do the harder intellectual work of translation.[^hopper-britannica]

Hopper's standard for herself was absolute: *"I've come to feel that there
is no use doing anything unless you can communicate it."*[^hopper-communicate]

**What Hopper demands of us:**

| Principle | Operational Rule |
|-----------|-----------------|
| Write for the reader's vocabulary, not the author's | Agent-facing documentation uses ACP method names and JSON schemas. Operator documentation uses CLI commands and configuration paths. Never assume the reader shares your mental model. |
| If the audience cannot understand it, the writer has failed — not the reader | Every `describe()` output, every README quick-start section, every error message must be comprehensible on first reading without requiring prior context. |
| Build the bridge others said was impossible | When a concept seems "too complex to document clearly," that is precisely when documentation matters most. The reactive surface pipeline, the bitemporal memory semantics, the policy engine — these demand Hopper-level clarity. |

### 2.2 Ada Lovelace (1815–1852) — Precision as Vision

Ada Lovelace's Notes on the Analytical Engine (1843) are three times
longer than the article they annotate. She did not merely translate
Menabrea's paper — she saw what Babbage himself had not fully articulated:
that the machine could manipulate symbols beyond arithmetic, that it could
"weave algebraic patterns, just as the Jacquard-loom weaves flowers and
leaves."[^lovelace-notes]

Her annotations included the first algorithm ever written for machine
execution (computing Bernoulli numbers), and when Babbage offered to write
one section himself, Lovelace returned it with a correction — she had
found a grave error in his work.[^lovelace-babbage]

What makes Lovelace's contribution singular is not that she wrote code but
that she documented a machine that did not yet exist with such precision
that her specification remains verifiable 180 years later. She wrote for
a future reader she would never meet.

**What Lovelace demands of us:**

| Principle | Operational Rule |
|-----------|-----------------|
| Document with enough precision that the specification is independently verifiable | Every data model, every API contract, every state transition must be stated with sufficient precision that a reader can write a test from the documentation alone — without consulting source code. |
| See beyond the immediate implementation to the implications | The Russell pod is not merely a process wrapper; it is an organizing principle for agent identity and lifecycle. Documentation must articulate *why* a design exists, not merely *what* it does. The ACP session interface is not a feature — it is a statement about how agents interact safely with host observation. |
| Annotate with more depth than the original | When documenting a design decision, the annotation (rationale, context, consequences, alternatives considered) must exceed the decision statement itself in length and value. An ADR with a one-line "Context" section is a failure of Lovelacian duty. |

### 2.3 Karen Schriver (b. 1955) — Design for the Reader

Karen Schriver's *Dynamics in Document Design* (1997) is the first
research-based portrait of what readers actually need from technical
documents — not what writers assume they need. Named a "legend in
technical communication" by the Society for Technical Communication and
the first recipient of STC's Ken Rainey Award for Excellence in Research,
Schriver demonstrated through empirical study that document quality is
measurable by reader outcomes, not author intent.[^schriver-dynamics]

Her guiding philosophy is direct: *"It's all about your readers."*

Schriver proved that typography, layout, and visual-verbal integration are
not cosmetic — they are functional components of comprehension. A diagram
placed correctly reduces cognitive load measurably. A heading that
accurately previews its content enables selective reading. A citation that
traces an idea to its origin enables trust.[^schriver-attw]

**What Schriver demands of us:**

| Principle | Operational Rule |
|-----------|-----------------|
| Design documents for how readers actually read, not how authors wish they would | Readers scan before they read. Every document must have: a navigation table, scannable headings that preview content, and diagrams placed at the point of use — not in appendices. |
| Integrate word and image as a single communication unit | A Mermaid diagram and its surrounding prose are not independent artifacts — they are a single explanation rendered in two modalities. The prose must reference the diagram explicitly ("as shown in DIAG-APP-005 above") and the diagram must be captioned with its TOGAF concern. Neither should be comprehensible alone. |
| Measure quality by reader outcomes, not author effort | If a reader cannot find the answer to their question within 30 seconds of arriving at the right document, the document has failed — regardless of how thorough its content may be. The navigation hierarchy, cross-references, and section structure exist to serve retrieval, not taxonomic completeness. |

### 2.4 Anne Gentle (b. 1971) — Documentation as Living System

Anne Gentle's *Docs Like Code* (2017, now in 3rd edition) codified what
open-source communities had been discovering through practice: that
documentation is not a deliverable produced after code ships — it is a
living system that shares the code's lifecycle, tools, and quality
gates.[^gentle-docs]

As Principal Engineer at Rackspace and Director of Developer Experience at
Cisco DevNet, Gentle led OpenStack's community documentation effort — 130
git repositories with over 130 active contributors — proving that
documentation scales only when it is subject to the same discipline as
code: version control, continuous integration, automated testing, peer
review, and contributor workflows.[^gentle-openstack]

Her formulation is operational: *"Treating docs like code means
collaborating with contributors efficiently by keeping docs close to code
or in the same system as code, with a source file concept and an output
for deliverables."*[^gentle-about]

What makes Gentle's contribution singular for Russell is that in an
agent-native system, documentation is not a companion to code — it *is*
the code. When an AI agent reads `AGENTS.md` to understand what
vocabulary governs the system, or parses `overview.md` to determine
which crate owns a feature, or follows the data model in
`PERSISTENCE_CATALOG.md` to validate a schema — the documentation is
executing as specification. The TOGAF-aligned architecture documents
in this repository are consumed by AI agents as operational truth, not
by humans as reading material. Gentle's insight that docs must share
the code's lifecycle, testing, and CI discipline is not merely best
practice here — it is a correctness requirement. A stale document in an
agent-native system is not an inconvenience; it is a bug that produces
incorrect agent behavior.

This is the Gentle Principle at its most consequential: **in a system
where AI agents consume documentation as their primary interface to
architecture, the documentation *is* the runtime specification.** Drift
between docs and code is not a quality problem — it is a functional
defect.

**What Gentle demands of us:**

| Principle | Operational Rule |
|-----------|-----------------|
| Documentation lives in the same repository and shares the same review process as code | All Russell documentation lives under `docs/` in the same git repository. Changes to documentation go through the same PR review, CI checks, and merge process as code changes. Documentation and code changes for the same feature belong in the same commit. |
| Automate quality gates; do not rely on human vigilance alone | Link checking (`check_links.sh`), package count verification (`cargo metadata`), stale-name detection (`grep`), and diagram metadata presence must be automated gates — not manual checklist items that humans forget under deadline pressure. |
| Documentation must accept contributions from the people who know the system best — the developers writing the code | The documentation format (Markdown), tooling (git, grep, standard CLI), and conventions (DIAGRAM_ALIGNMENT metadata, footnote format) are deliberately chosen to be native to developers. No proprietary toolchain, no CMS, no separate publishing system. A developer who can write a `cargo test` can write a documentation section. |
| Continuous integration for docs means broken docs block the build, just as broken tests do | The `check_links.sh` script and the knowledge graph regeneration are CI-equivalent gates. A stale package count is a documentation bug with the same severity as a compilation error — it means the system description no longer matches the system. |

---

## 3. Voice and Style Standards

These standards synthesize the principles above into operational rules
for every document in the corpus.

### 3.1 Voice

| Dimension | Standard | Exemplar Origin |
|-----------|----------|-----------------|
| **Register** | Formal-technical. No slang, no hedging, no filler. | Lovelace: precision without apology |
| **Person** | Third person for specifications; second person for operator guides. Never first person plural ("we"). | Hopper: the system speaks; the reader acts |
| **Tense** | Present tense for current-state descriptions; past tense only for historical provenance notes. | Schriver: readers expect documents to describe *now* |
| **Confidence** | Assertions are definite. Use "must", "shall", "does" — never "should probably", "might", "could potentially". Where uncertainty exists, state it as an explicit open question, not as a weakened assertion. | Hopper: "It is easier to ask forgiveness than to get permission." Commit to the statement. |

### 3.2 Sentence Construction

- Maximum sentence length: 35 words. If a sentence exceeds this, split it.
- One idea per sentence. One claim per paragraph.
- Active voice in all cases. Passive voice is permitted only in citations
  describing what *a source* states.
- Technical terms are used once without definition only if they are
  defined in the project glossary or in a preceding section of the same
  document. Otherwise, define on first use.

### 3.3 Structural Discipline

Every document section follows this invariant structure:

1. **Statement** — what is true, in one sentence
2. **Evidence** — code path, command, or external citation
3. **Diagram** — visual rendering of the structure or flow (if applicable)
4. **Implications** — what the reader should do with this knowledge

Sections that lack evidence are drafts. Sections that lack implications
are reference material (acceptable in specifications, not in architecture
narratives).

### 3.4 Citation Density Requirements

| Document type | Minimum citations per `##` section |
|---------------|-----------------------------------|
| Architecture (A-D) | 1 external source |
| Specifications | 1 external source or 1 code-path verification |
| Standards | 1 external source |
| Operations | 0 (commands are self-verifying) |
| Plans | 0 (plans describe future work) |
| Status | 1 verification command per claim |

---

## 4. Quality Enforcement Process

The four tests below represent four independent dimensions of
documentation quality. They are not a conjunctive checklist — demanding
perfection across all four simultaneously is unrealistic and would
paralyze publication. Instead, they form a scoring rubric:[^rubric]

| Score | Meaning | Publication Decision |
|-------|---------|---------------------|
| **1 of 4 passes** | Poor quality | Do not publish. Fundamental rework required. |
| **2 of 4 pass** | Passing | Acceptable for publication with noted gaps. |
| **3 of 4 pass** | Excellent | Publish confidently. The remaining dimension is a documented improvement target. |
| **4 of 4 pass** | Exceptional | Rare. Celebrate and use as a reference exemplar. |

**The goal is 3 out of 4.** Different document types naturally emphasize
different dimensions: an operations quick-start guide will ace Hopper and
Schriver but may not reach Lovelace-level precision; a data architecture
ER specification will ace Lovelace and Gentle but may not achieve Hopper's
zero-context accessibility for non-technical readers. This is acceptable.
What is not acceptable is passing only one.

### 4.1 The Hopper Test (Accessibility)

*Can a reader with zero prior context accomplish the task described in
this document by following only what is written?*

Strongest fit: operations guides, READMEs, quick-start sections, error
messages, `describe()` outputs.

### 4.2 The Lovelace Test (Precision)

*Could a reader write a correct implementation or test from this
specification alone, without consulting source code?*

Strongest fit: architecture specifications, API contract tables, data
model ER diagrams, state transition diagrams, ADR consequence sections.

### 4.3 The Schriver Test (Findability)

*Can a reader find the answer to their specific question within 30
seconds of arriving at this document?*

Strongest fit: portal READMEs, navigation hubs, traceability matrices,
any document exceeding 200 lines.

### 4.4 The Gentle Test (Agent-Correctness)

*If an AI agent consumed this document as its sole source of truth about
the system, would it behave correctly?*

Strongest fit: architecture documents (A–D), the knowledge graph,
AGENTS.md, status documents, any document that agents parse to determine
system structure or capability.

This test treats documentation accuracy as a functional correctness
property. A package count wrong by one, a removed skill still named
in a diagram, a stale tool name in a contract table — these are not
cosmetic issues. They are defects that produce incorrect agent behavior
in a system where documentation is the agent's primary interface to
architecture.

---

## 5. Recommended Reading

These works form the intellectual foundation of this protocol. Every
contributor to the documentation corpus is encouraged to read them.

| Work | Author | Contribution to this protocol |
|------|--------|-------------------------------|
| *A Manual of Operation for the Automatic Sequence Controlled Calculator* (1946) | Grace Hopper | The first computer manual; establishes that complex systems require comprehensive, accessible documentation |
| Notes on the Analytical Engine (1843) | Ada Lovelace | Demonstrates that specification precision enables independent verification across time and context |
| *Dynamics in Document Design* (1997) | Karen Schriver | Establishes that document quality is measurable by reader outcomes; provides research-based design principles |
| *Docs Like Code* (2017, 3rd ed. 2023) | Anne Gentle | Documentation shares code's lifecycle, CI, review, and contributor workflows — essential for agent-native systems where docs are the specification |
| *Information Development* (2006) | JoAnn Hackos | Defines process maturity for documentation organizations (IPMM); cited in our verification checklist |
| *Managing Your Documentation Projects* (1994) | JoAnn Hackos | The operational discipline of documentation project management |

---

## References

[^hopper-yale]: Office of the President, Yale University. (2017). *Biography of Grace Murray Hopper*. <https://president.yale.edu/biography-grace-murray-hopper>. Primary biographical source for Hopper's career and documentation contributions.

[^hopper-britannica]: Britannica, T. Editors of Encyclopaedia. (2024). *Grace Hopper*. <https://www.britannica.com/biography/Grace-Hopper>. Cited for the first compiler development and FLOW-MATIC chronology.

[^hopper-communicate]: Hopper, G. M. (1980). Interview. As quoted in Beyer, K. W. (2009). *Grace Hopper and the Invention of the Information Age*. MIT Press. Cited for the communication imperative.

[^lovelace-notes]: Lovelace, A. A. (1843). Notes on L. F. Menabrea's "Sketch of the Analytical Engine Invented by Charles Babbage." *Scientific Memoirs*, 3. Cited for the "weaves algebraic patterns" formulation and the first published algorithm.

[^lovelace-babbage]: Babbage, C. (1864). *Passages from the Life of a Philosopher*. Longman, Green. Cited for Babbage's testimony regarding Lovelace's correction of his error and the independence of her analytical work.

[^schriver-dynamics]: Schriver, K. A. (1997). *Dynamics in Document Design: Creating Text for Readers*. Wiley. The foundational text for reader-centered document design; cited for the empirical basis of the reader-outcome quality measure.

[^schriver-readers]: Schriver, K. A. (2012). What do technical communicators need to know about information design? In J. Johnson-Eilola & S. A. Selber (Eds.), *Solving Problems in Technical Communication*. University of Chicago Press. Cited for the "it's all about your readers" principle.

[^schriver-attw]: Association of Teachers of Technical Writing. (2015). *2015 ATTW Fellow: Karen Schriver*. <https://attw.org/about-attw/attw-fellows/2015-karen-schriver/>. Cited for Schriver's research standard and boundary-spanning contributions.

[^rubric]: Stevens, D. D., & Levi, A. J. (2013). *Introduction to Rubrics* (2nd ed.). Stylus Publishing. Cited for the multi-dimensional scoring approach where dimensions are independent and not all equally weighted for every artifact type.

[^gentle-docs]: Gentle, A. (2017). *Docs Like Code: Collaborate and Automate to Improve Technical Documentation*. Just Write Click. <https://www.docslikecode.com/book/>. The foundational text for docs-as-code methodology; cited for the principle that documentation shares code's lifecycle and quality gates.

[^gentle-openstack]: Gentle, A. (2016). Git and GitHub for open source documentation. *OpenSource.com*. <https://opensource.com/article/16/4/git-and-github-open-source-documentation>. Cited for the scaling demonstration: 130 git repos, 130+ contributors, automated builds.

[^gentle-about]: Gentle, A. (2024). About Docs Like Code. <https://www.docslikecode.com/about/>. Cited for the operational definition of docs-as-code: "source file concept and an output for deliverables."

[^matc-women]: Bogue, M. (2025). Women Who Shaped Technical Writing: A History of Progress, Struggles, and Successes. MATC Group. <https://www.matcgroup.com/technical-writing/women-who-shaped-technical-writing-a-history-of-progress-struggles-and-successes/>. Cited for historical context on women's contributions to technical communication.
