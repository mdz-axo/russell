# FlowDef Converter — Jack's hKask-to-Russell Bridge

> **A note from Jack about converting FlowDef skills:** hKask and I are
> family — we share the same operator, the same machine, and the same
> commitment to keeping things running. But we speak different dialects.
> hKask orchestrates multi-step processes with template rendering, energy
> budgets, and CNS spans. I inject knowledge into my chat REPL and run
> bash probes. Same methodology, different execution model. This skill
> translates between them. I read a FlowDef manifest and its Jinja2
> templates, extract the knowledge and logic, and produce a Russell skill
> that preserves the intent while adapting the form.
>
> **Source:** This knowledge file. Paired with conversion scripts in `scripts/`.
> **Bridge:** `~/Clones/hKask/registry/` → `~/.local/share/harness/skills/`

---

## 1. The Conversion Model

hKask FlowDef and Russell skills solve the same problem differently:

| Aspect | hKask FlowDef | Russell Skill |
|---|---|---|
| **Execution** | Orchestrated process steps (ordinals) | Knowledge injection + chat REPL |
| **Templates** | Jinja2 (.j2) rendered by inference engine | Embedded in KNOWLEDGE.md as prompt guidance |
| **State** | Step-to-step via input_mapping + output_schema | Conversation context in chat REPL |
| **Energy** | Energy caps per step, cost_per_token | No energy model (single-host, no token budget) |
| **Observability** | CNS spans, algedonic signals | Journal events, proprioception |
| **Consent** | OCAP capability delegation | Consent gate (operator approves) |
| **Inputs** | Declared inputs with types | Symptoms + environment variables |

### The key insight

A FlowDef step that does **cognitive work** (renders a template, evaluates
answers, makes decisions) converts to a **KNOWLEDGE.md section** — Jack
reads the prompt guidance and does the cognitive work in the chat REPL.

A FlowDef step that does **deterministic work** (validates schemas, checks
state, computes metrics) converts to a **probe script** — a bash script
that produces structured output without LLM involvement.

### Conversion mapping

```
FlowDef manifest.id          → manifest.yaml id
FlowDef manifest.description → KNOWLEDGE.md purpose section
FlowDef inputs               → symptoms + env vars
FlowDef levels               → KNOWLEDGE.md taxonomy section
FlowDef steps (cognitive)    → KNOWLEDGE.md methodology sections
FlowDef steps (deterministic)→ probe/intervention scripts
FlowDef escalation rules     → KNOWLEDGE.md decision rules
FlowDef error_handling       → script error handling
FlowDef template content     → KNOWLEDGE.md prompt guidance
FlowDef energy caps          → DROPPED (not applicable)
FlowDef OCAP/CNS/audit       → DROPPED (Russell has its own)
```

---

## 2. Step Classification Algorithm

When converting a FlowDef step, I classify it as cognitive or deterministic:

### Cognitive steps (→ KNOWLEDGE.md)

- `action: populate` with a `template_ref` → The step renders a prompt
  template and gets LLM output. This is cognitive work → convert the
  template content into a KNOWLEDGE.md section that teaches Jack the
  methodology.

- `action: select` with a `template_ref` → The step makes a decision
  (escalate/hold/reprobe). Convert the decision rules into KNOWLEDGE.md
  decision criteria.

- `action: feedback` → The step emits CNS events. In Russell, this is
  handled by the journal. Drop the step and note the CNS span namespace
  in the references section.

### Deterministic steps (→ probe/intervention scripts)

- `action: validate` with `validation_rules` → The step checks constraints
  (acyclic graphs, sum checks). Convert to a probe script that checks
  the same constraints.

- `action: populate` with `renderer: null` and `template_ref: null` →
  The step does deterministic work (storage, state updates). Convert to
  an intervention script if it mutates state, or a probe if read-only.

### Ambiguous steps

- `action: populate` with `model_tier: fast_local` → Could be either.
  If the template is short and structured (JSON output), it's likely
  deterministic-style work delegated to the LLM. Convert to KNOWLEDGE.md
  with a note about the original model tier.

---

## 3. Template Extraction

Jinja2 templates have two parts: a configuration header (in `[inference]`
blocks) and the prompt body. I extract both:

### Configuration → metadata notes

```
[inference]
temperature = 0.4
reasoning_effort = "high"
```

These inform Jack's approach: "This step was designed for high-reasoning
effort. Take your time and think carefully."

### Prompt body → KNOWLEDGE.md content

The Jinja2 variables (`{{ topic }}`, `{{ current_level }}`) become
placeholder references in KNOWLEDGE.md that Jack fills from the
conversation context:

```
Original: "You are conducting an examination on **{{ topic }}**."
Russell:  "When grilling, the topic comes from the conversation context.
          The operator specified it when they said 'grill me about X'."
```

### Jinja2 control flow → conditional guidance

```
{% if mode == "calibrate" %}
Original calibration instructions
{% elif mode == "interrogate" %}
Original interrogation instructions
{% endif %}
```

Becomes:

```
### Calibration phase
When starting a session, I calibrate by asking 1-2 quick recall
questions to gauge baseline knowledge.

### Interrogation phase
After calibration, I begin round-based interrogation...
```

---

## 4. Converting the grill-me FlowDef (Example)

The hKask grill-me FlowDef has 5 steps:

| Ordinal | Action | Russell Conversion |
|---------|--------|-------------------|
| 1 | select (escalation decision) | KNOWLEDGE.md §3 (Escalation Logic) |
| 2 | populate (first round, calibrate mode) | KNOWLEDGE.md §2 (Round Structure, calibration) |
| 3 | populate (subsequent rounds, interrogate mode) | KNOWLEDGE.md §2 (Round Structure, interrogation) |
| 4 | populate (final assessment synthesis) | KNOWLEDGE.md §5 (Final Gap Analysis) |
| 5 | feedback (CNS events) | DROPPED (Russell has journal) |

The escalation rules from step 1 become the decision rules in §3.
The template from steps 2-3 become the methodology in §2.
The template from step 4 becomes the assessment template in §5.
The energy caps and OCAP constraints are dropped.
The CNS span namespace is noted in references.

---

## 5. Conversion Workflow

When the operator asks me to convert a FlowDef skill:

1. **Analyze** — Run `flowdef-converter/analyze <manifest_path>` to see
   the structure, steps, and templates.

2. **Check templates** — Run `flowdef-converter/check-templates <manifest_path>`
   to verify all referenced templates exist.

3. **Dry-run** — Run `flowdef-converter/dry-run <manifest_path>` (which
   calls `convert.sh` with `RUSSELL_DRY_RUN=1`) to preview the output
   without writing files.

4. **Convert** — Run `flowdef-converter/convert <manifest_path> <output_dir>`
   to produce the Russell skill files:
   - `<output_dir>/manifest.yaml`
   - `<output_dir>/KNOWLEDGE.md`
   - `<output_dir>/scripts/` (any probe/intervention scripts)

5. **Install** — Use `skill-manager/install <skill-name>` to register
   the converted skill.

6. **Verify** — Run the skill's `health` probe to verify integrity.

---

## 6. What Gets Lost in Translation

Some FlowDef concepts have no Russell equivalent:

| FlowDef Concept | Russell Equivalent | Notes |
|---|---|---|
| Energy caps | None | Russell is single-host, no token budget |
| OCAP capabilities | Consent gate | Different security model |
| CNS spans | Journal events | Different observability model |
| Template rendering | KNOWLEDGE.md injection | Different execution model |
| Step ordinals | Conversation turns | Implicit ordering in chat |
| Input/output schemas | KNOWLEDGE.md format guidance | Suggested, not enforced |
| model_tier | Okapi model selection | Jack uses whatever model is loaded |
| mcp endpoint | Direct script execution | No MCP layer in Russell |

These aren't failures — they're design differences. The conversion preserves
the **methodology** (what to do and why) while adapting the **mechanism**
(how to do it).

---

## 7. Safety and Scope

### What the converter will convert

- Process manifests (FlowDef type) with steps and templates
- Knowledge manifests (semantic memory)
- Skill manifests with probes and interventions

### What the converter refuses to convert

- Manifests with `action: execute` steps (arbitrary code execution)
- Manifests with unresolvable template references
- Manifests that require network access (Russell skills are local-only)

### Output validation

After conversion, the converter validates:
1. `manifest.yaml` has all required fields (id, version, symptoms, probes)
2. `KNOWLEDGE.md` is non-empty
3. All referenced scripts exist and are executable
4. No `russell` or `bash` command references in script `cmd` fields
   (poka-yoke: skill commands must use explicit paths)

---

## 8. How This Connects to Other Skills

### workspace

The converter uses `workspace/write-file` and `workspace/create-directory`
to write the converted skill files to disk.

### skill-manager

After conversion, `skill-manager/install` registers the new skill.
`skill-manager/build` creates the skeleton if the converter is used
iteratively.

### grill-me

The first converted skill — grill-me was the proof of concept. Its
KNOWLEDGE.md was hand-crafted from the FlowDef templates. The converter
automates this process for future FlowDef skills.

---

**Version:** 1.0.0
**Last updated:** 2026-06-03
**Prerequisite skills:** workspace (for file output), skill-manager (for installation)
**Source registry:** ~/Clones/hKask/registry/