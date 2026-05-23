# Phase 3: Template Crate Skills — Progress Report

**Date:** 2026-05-22  
**Status:** Infrastructure Complete, First Skill Converted  
**Next:** Convert remaining 12 skills

---

## Summary

Phase 3 template infrastructure is complete. The `russell-skills` crate now supports:
- Jinja2 template rendering via `minijinja`
- Template context with probes, journal, host telemetry, and skill metadata
- Template crate structure with `templates/`, `agent_persona.yaml`, `hlexicon.yaml`
- First skill converted: `okapi-watcher`

---

## What Was Built

### 1. Template Module (`crates/russell-skills/src/templates.rs`)

**Core Types:**
- `TemplateEngine` — MiniJinja wrapper with skill-specific helpers
- `TemplateContext` — Rendering context (probes, journal, params, skill, host)
- `TemplateCrate` — Template crate loader and manager
- `TemplateError` — Error enumeration

**Features:**
- Load templates from `templates/*.j2` files
- Render with context (probe results, journal state, host telemetry)
- Built-in filters: `default`, `round`
- Context serialization for debugging

**Tests:** 4 passing
- `test_template_context_serialization`
- `test_template_render_basic`
- `test_template_render_with_params`
- `test_load_okapi_watcher_templates` (integration test)

---

### 2. Okapi-Watcher Template Crate (`skills/okapi-watcher/`)

**New Files:**
```
skills/okapi-watcher/
├── Cargo.toml              # Rust package metadata
├── agent_persona.yaml      # Skill agent identity
├── hlexicon.yaml           # Human-readable terms
└── templates/
    ├── selector.j2         # Template selection logic
    ├── health-ok.j2        # Healthy status report
    ├── health-critical.j2  # Critical health alert
    ├── gpu-fallback.j2     # GPU fallback warning
    └── no-models.j2        # Empty model inventory alert
```

**Existing Files (unchanged):**
- `manifest.yaml` — Probe/intervention definitions
- `scripts/` — Bash probe scripts
- `SKILL.md` — Documentation
- `skill.json` — Metadata

---

## Template Examples

### Selector Template (`selector.j2`)

```jinja2
{% if probes["probe-health"] == "unhealthy" %}
  {% set response_template = "health-critical" %}
{% elif probes["probe-gpu-libs"] == "missing" %}
  {% set response_template = "gpu-fallback" %}
{% elif probes["probe-models"] == "empty" %}
  {% set response_template = "no-models" %}
{% else %}
  {% set response_template = "health-ok" %}
{% endif %}

{{ response_template }}
```

### Health OK Template (`health-ok.j2`)

```jinja2
## Okapi Status: Healthy ✓

**Instance:** {{ params.instance | default("local") }}
**Last Check:** {{ probes["probe-health-timestamp"] | default("unknown") }}

### Model Inventory
{% if probes["probe-models-output"] %}
{{ probes["probe-models-output"] }}
{% else %}
No models detected.
{% endif %}

### GPU Acceleration
{% if probes["probe-gpu-libs"] == "available" %}
✓ GPU acceleration active
{% elif probes["probe-gpu-libs"] == "missing" %}
⚠ GPU libraries missing - falling back to CPU
{% else %}
? GPU status unknown
{% endif %}
```

---

## Usage

### Load Template Crate

```rust
use russell_skills::templates::{TemplateCrate, TemplateEngine, TemplateContext};

// Load template crate
let crate_path = PathBuf::from("~/.local/share/harness/skills/okapi-watcher");
let template_crate = TemplateCrate::load(&crate_path)?;

// Create rendering context
let mut ctx = TemplateContext::default();
ctx.skill.id = "okapi-watcher".to_string();
ctx.probes.insert("probe-health".to_string(), "healthy".to_string());
ctx.params.insert("instance".to_string(), serde_json::json!("local"));

// Render template
let engine = TemplateEngine::new();
let template_path = template_crate.template_path("health-ok");
let rendered = engine.render_file(&template_path, &ctx)?;
```

### Integration with Dispatch

Templates integrate with the existing dispatch system:
1. Probes run (bash scripts)
2. Results populate `TemplateContext.probes`
3. Selector template chooses response template
4. Response template renders with full context
5. Rendered output sent to LLM (Jack) or operator

---

## Remaining Work

### Skills to Convert (12 remaining)

| Skill | Priority | Complexity | Notes |
|-------|----------|------------|-------|
| `skill-manager` | High | Medium | Self-management meta-skill |
| `skill-workshop` | High | Medium | Interactive REPL |
| `skill-discovery` | High | High | Registry search |
| `skill-maintenance` | Medium | Medium | Lifecycle ops |
| `sysadmin` | Medium | Low | Basic sysadmin probes |
| `journal-compactor` | Low | Low | Simple maintenance |
| `pragmatic-semantics` | Medium | High | Memory layer |
| `pragmatic-cybernetics` | Medium | High | Cybernetics logic |
| `scenario-tester` | Low | Medium | Test scenarios |
| `web-search` | Medium | Medium | Brave API integration |
| `package-checker` | Low | Low | Package validation |
| `ubuntu-jack` | Low | Low | Ubuntu-specific probes |

### Template Patterns Needed

1. **Probe Result Patterns**
   - Success/failure branching
   - Numeric threshold comparisons
   - List iteration (model lists, process lists)

2. **Context Enrichment**
   - Journal state (breach counts, severity distribution)
   - Host telemetry (CPU, memory, disk, GPU)
   - Skill metadata (version, dispatch ID)

3. **Response Patterns**
   - Status reports (healthy/degraded/critical)
   - Intervention proposals (with risk levels)
   - Diagnostic summaries (with recommended actions)

---

## Testing

### Unit Tests (4 passing)
- Context serialization
- Basic template rendering
- Parameter injection
- Template crate loading

### Integration Tests (needed)
- Full probe → render → dispatch pipeline
- Template selection logic
- Error handling (missing templates, render failures)

---

## Dependencies

**Added:**
- `minijinja = "2"` (workspace dependency, already present)

**No Breaking Changes:**
- Existing bash-based skills continue to work
- Template support is opt-in per skill
- `manifest.yaml` format unchanged

---

## Next Steps

1. **Convert 3 high-priority skills** (skill-manager, skill-workshop, skill-discovery)
2. **Add template helpers** (custom filters for common operations)
3. **Integrate with dispatch** (wire templates into probe execution pipeline)
4. **Document template patterns** (create template catalog)
5. **Add CLI commands** (`russell skill render <id> <template>`)

---

## Effort Tracking

| Task | Estimated | Actual | Status |
|------|-----------|--------|--------|
| Template module | 4h | 2h | ✅ Complete |
| Okapi-watcher conversion | 2h | 1.5h | ✅ Complete |
| Remaining 12 skills | 12h | - | 🔄 Pending |
| Dispatch integration | 4h | - | 🔄 Pending |
| Documentation | 2h | - | 🔄 Pending |
| **Total** | **24h** | **3.5h** | **15% complete** |

---

**References:**
- [`docs/AGENT-POD-REFACTORING-PLAN.md`](../docs/AGENT-POD-REFACTORING-PLAN.md) — Full refactoring plan
- [`crates/russell-skills/src/templates.rs`](../crates/russell-skills/src/templates.rs) — Template module
- [`skills/okapi-watcher/templates/`](../skills/okapi-watcher/templates/) — First template crate
