---
title: "Skill System — Hexagonal Ports & Adapters"
audience: [architects, developers, agents]
last_updated: 2026-05-15
togaf_phase: "C"
version: "1.0.0"
status: "Active"
---

<!-- TOGAF_DOMAIN: Application Architecture -->
<!-- VERSION: 1.0.0 -->
<!-- STATUS: Active -->
<!-- LAST_UPDATED: 2026-05-15 -->

# Skill System — Hexagonal Ports & Adapters

> Each port defined as a trait in `russell-core` (the port definition layer per cross-crate DAG).
> Adapters live in the crate closest to their dependency.
> Pattern follows `russell-proprio/src/lib.rs:54-76` — trait-as-port, struct-as-adapter.
> Version: 1.0.0 | 2026-05-15

---

## Port Inventory

| Port | Status | Crate | Trait Name |
|---|---|---|---|
| `SkillLoader` | **PROPOSED** | `russell-core` | `SkillLoader` |
| `SkillValidator` | **EXISTS IMPLICITLY** | `russell-core` | `SkillValidator` |
| `SkillIndex` | **EXISTS IMPLICITLY** | `russell-core` | `SkillIndex` |
| `SkillTelemetry` | **EXISTS IMPLICITLY** | `russell-core` | `SkillTelemetryRecorder` |
| `SkillEvaluator` | **PROPOSED** | `russell-core` | `SkillEvaluator` |
| `PromptAssembler` | **EXISTS IMPLICITLY** | `russell-core` | `PromptAssembler` |
| `ActionResolver` | **EXISTS IMPLICITLY** | `russell-core` | `ActionResolver` |
| `KnowledgeInjector` | **EXISTS IMPLICITLY** | `russell-core` | `KnowledgeInjector` |

**Status codes:**
- **EXISTS AS TRAIT**: `pub trait` already defined in codebase
- **EXISTS IMPLICITLY**: Concrete struct or free function only — no trait boundary
- **PROPOSED**: New trait, no implementation exists

> Note: These ports are defined in `russell-core` (the dependency-free anchor crate) following the cross-crate DAG pattern documented in `CODE_ANCHOR_GRAPH.md` §7. This prevents `russell-cli` from depending on implementation details of `russell-skills`.

---

## Port 1: `SkillLoader`

### Trait definition — `russell-core/src/port/skill_loader.rs`

```rust
use std::path::Path;

/// Loads skills from a source.
///
/// The port for discovering and loading skill manifests.
/// Adapters: FilesystemSkillLoader (current), RemoteRegistryLoader (future).
pub trait SkillLoader: Send + Sync {
    /// The type of error returned on load failure.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Load all skills from the configured source.
    /// Returns a collection of skill identity descriptors.
    fn load_all(&self) -> Result<Vec<SkillDescriptor>, Self::Error>;

    /// Load a single skill by ID.
    fn load_one(&self, id: &str) -> Result<Option<SkillDescriptor>, Self::Error>;
}

/// Minimal skill identity for the loader port — not the full Skill aggregate.
/// This avoids pulling russell-skills types into russell-core.
#[derive(Debug, Clone)]
pub struct SkillDescriptor {
    pub id: String,
    pub kind: SkillCapability,
    pub version: String,
    pub authored: String,
    pub symptoms: Vec<String>,
    pub probe_count: usize,
    pub intervention_count: usize,
    pub manifest_content: String,
    pub directory_path: std::path::PathBuf,
}

/// Capability flags for a skill (replaces SkillKind binary enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillCapability(u8);

impl SkillCapability {
    pub const CAN_PROBE: Self = Self(1 << 0);
    pub const CAN_INTERVENE: Self = Self(1 << 1);
    pub const HAS_KNOWLEDGE: Self = Self(1 << 2);
    pub const CAN_BE_TESTED: Self = Self(1 << 3);

    pub fn is_actionable(self) -> bool {
        self.0 & (Self::CAN_PROBE.0 | Self::CAN_INTERVENE.0) != 0
    }

    pub fn is_lens(self) -> bool {
        self.0 & (Self::CAN_PROBE.0 | Self::CAN_INTERVENE.0) == 0 && self.0 & Self::HAS_KNOWLEDGE.0 != 0
    }
}
```

### Adapter: `FilesystemSkillLoader` — `russell-skills/src/loader.rs`

```rust
/// Loads skills from the filesystem skills directory.
///
/// Mirrors the existing `load_all()` function at lib.rs:452 but
/// returns `SkillDescriptor` through the port contract.
pub struct FilesystemSkillLoader {
    skills_dir: PathBuf,
}

impl FilesystemSkillLoader {
    pub fn new(skills_dir: impl Into<PathBuf>) -> Self {
        Self { skills_dir: skills_dir.into() }
    }
}

impl SkillLoader for FilesystemSkillLoader {
    type Error = crate::LoadError;

    fn load_all(&self) -> Result<Vec<SkillDescriptor>, Self::Error> {
        crate::load_all_descriptors(&self.skills_dir)
    }

    fn load_one(&self, id: &str) -> Result<Option<SkillDescriptor>, Self::Error> {
        let dir = self.skills_dir.join(id);
        if !dir.exists() {
            return Ok(None);
        }
        crate::load_descriptor(&dir).map(Some)
    }
}
```

### Adapter: `RemoteRegistryLoader` — future (`russell-skills/src/remote/loader.rs`)

```rust
/// Loads skills from a remote registry (future — ADR-0025 §8 deferral lifted).
pub struct RemoteRegistryLoader {
    sources: RegistrySources,
    cache_dir: PathBuf,
}
impl SkillLoader for RemoteRegistryLoader { /* ... */ }
```

---

## Port 2: `SkillValidator`

### Trait definition — `russell-core/src/port/skill_validator.rs`

```rust
/// Validates a skill before installation or loading.
///
/// Adapters: ManifestValidator, SafetyScanner, PokaYokeValidator.
/// All three are chained via a composite validator.
pub trait SkillValidator: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Validate a skill descriptor. Returns Ok if all checks pass,
    /// or an error describing the first validation failure.
    fn validate(&self, skill: &SkillDescriptor) -> Result<(), Self::Error>;
}

/// Composite validator that chains multiple validators together.
pub struct ChainedValidator<V: SkillValidator> {
    validators: Vec<V>,
}

impl<V: SkillValidator> SkillValidator for ChainedValidator<V> {
    type Error = V::Error;
    fn validate(&self, skill: &SkillDescriptor) -> Result<(), Self::Error> {
        for validator in &self.validators {
            validator.validate(skill)?;
        }
        Ok(())
    }
}
```

### Adapter: `ManifestValidator` — `russell-skills/src/registry/safety.rs` (extracted)

```rust
/// Validates structural manifest completeness.
/// Current implementation: the parse_manifest() function at lib.rs:625
/// already performs this validation inline. This adapter extracts it.
pub struct ManifestValidator;

impl SkillValidator for ManifestValidator {
    type Error = crate::LoadError;
    fn validate(&self, skill: &SkillDescriptor) -> Result<(), Self::Error> {
        // Check required fields: id, version, authored, symptoms
        if skill.id.is_empty() {
            return Err(crate::LoadError::MissingField("id".into()));
        }
        if skill.version.is_empty() {
            return Err(crate::LoadError::MissingField("version".into()));
        }
        // ... existing parse_manifest validation logic
        Ok(())
    }
}
```

### Adapter: `PokaYokeValidator` — `russell-skills/src/registry/poka_yoke.rs` (extracted)

```rust
/// Validates that all symptoms referenced in the manifest exist in the catalog.
/// Current implementation: the check in load_all() at lib.rs:452 that
/// returns LoadError::UnknownSymptom.
pub struct PokaYokeValidator {
    catalog: Vec<String>,
}

impl SkillValidator for PokaYokeValidator {
    type Error = crate::LoadError;
    fn validate(&self, skill: &SkillDescriptor) -> Result<(), Self::Error> {
        for symptom in &skill.symptoms {
            if !self.catalog.contains(symptom) {
                return Err(crate::LoadError::UnknownSymptom {
                    symptom: symptom.clone(),
                    skill_id: skill.id.clone(),
                });
            }
        }
        Ok(())
    }
}
```

### Adapter: `SafetyScannerAdapter` — `russell-skills/src/registry/safety.rs`

```rust
/// Wraps the existing SafetyScan struct as a SkillValidator.
/// Current: SafetyScan is a concrete struct (safety.rs:13).
/// This adapter implements the SkillValidator trait.
pub struct SafetyScannerAdapter;

impl SkillValidator for SafetyScannerAdapter {
    type Error = SafetyValidationError;

    fn validate(&self, skill: &SkillDescriptor) -> Result<(), Self::Error> {
        let scan = SafetyScan::scan(&skill.manifest_content);
        if scan.has_blocks() {
            return Err(SafetyValidationError {
                skill_id: skill.id.clone(),
                findings: scan.into_findings(),
            });
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct SafetyValidationError {
    pub skill_id: String,
    pub findings: Vec<ScanFinding>,
}
```

---

## Port 3: `SkillIndex`

### Trait definition — `russell-core/src/port/skill_index.rs`

```rust
/// Index for skill lookup by symptom, status, or ID.
///
/// Adapters: RegistryCache (current, file-backed), InMemoryIndex (proposed, for sessions).
pub trait SkillIndex: Send + Sync {
    /// Look up installed skills addressing a given symptom.
    fn lookup_symptom(&self, symptom: &str) -> Vec<SkillRef>;

    /// Get all skills in a given lifecycle status.
    fn by_status(&self, status: LifecycleStatus) -> Vec<SkillRef>;

    /// Find all catalogued symptoms with no installed skill.
    fn coverage_gaps(&self, all_symptoms: &[String]) -> Vec<String>;

    /// Get a skill by ID.
    fn get(&self, skill_id: &str) -> Option<SkillRef>;
}

/// A lightweight reference to a skill in the index.
#[derive(Debug, Clone)]
pub struct SkillRef {
    pub id: String,
    pub status: LifecycleStatus,
    pub symptoms: Vec<String>,
    pub health: SkillHealthSummary,
}

#[derive(Debug, Clone, Default)]
pub struct SkillHealthSummary {
    pub quality_score: Option<f64>,
    pub freshness: f64,
    pub probe_runs: u64,
}
```

### Adapter: `RegistryCacheAdapter` — `russell-skills/src/registry/mod.rs` (refactored)

```rust
/// Wraps RegistryCache as a SkillIndex adapter.
/// Current: RegistryCache IS the index (mod.rs:38-41).
/// This adapter delegates all read operations.
impl SkillIndex for RegistryCache {
    fn lookup_symptom(&self, symptom: &str) -> Vec<SkillRef> {
        self.skills.values()
            .filter(|e| e.status.is_loadable() && e.symptoms.iter().any(|s| s == symptom))
            .map(|e| e.to_ref())
            .collect()
    }

    fn by_status(&self, status: LifecycleStatus) -> Vec<SkillRef> {
        self.skills.values()
            .filter(|e| e.status == status)
            .map(|e| e.to_ref())
            .collect()
    }

    fn coverage_gaps(&self, all_symptoms: &[String]) -> Vec<String> {
        // existing coverage_gaps logic from mod.rs:248
        todo!()
    }

    fn get(&self, skill_id: &str) -> Option<SkillRef> {
        self.skills.get(skill_id).map(|e| e.to_ref())
    }
}
```

### Adapter: `InMemoryIndex` — `russell-skills/src/registry/memory.rs` (proposed)

```rust
/// In-memory index for fast session-local lookups.
/// Bypasses file I/O — for chat sessions where the index is rebuilt
/// at session start and mutations are journaled inline.
pub struct InMemoryIndex {
    entries: BTreeMap<String, SkillRef>,
}

impl SkillIndex for InMemoryIndex {
    fn lookup_symptom(&self, symptom: &str) -> Vec<SkillRef> {
        self.entries.values()
            .filter(|e| e.symptoms.iter().any(|s| s == symptom))
            .cloned()
            .collect()
    }
    // ...
}
```

---

## Port 4: `SkillTelemetryRecorder`

### Trait definition — `russell-core/src/port/skill_telemetry.rs`

```rust
/// Records skill execution outcomes for health tracking.
///
/// Adapters: JournalTelemetry (current, wired for workshop only).
pub trait SkillTelemetryRecorder: Send + Sync {
    /// Record a probe execution outcome.
    fn record_probe(&mut self, skill_id: &str, success: bool, duration_ms: u64, error: Option<&str>);

    /// Record an intervention execution outcome.
    fn record_intervention(&mut self, skill_id: &str, success: bool, error: Option<&str>);

    /// Compute health for all tracked skills.
    fn compute_health(&self) -> BTreeMap<String, SkillHealthSummary>;

    /// Compute health for a single skill.
    fn compute_skill_health(&self, skill_id: &str) -> Option<SkillHealthSummary>;
}
```

### Adapter: `JournalTelemetryRecorder` — `russell-skills/src/registry/health.rs` (extended)

```rust
/// Records telemetry via RegistryEntry mutation and journal events.
/// Current: the record_execution() method on RegistryCache at mod.rs:279.
/// This adapter makes it a standalone port implementation.
pub struct JournalTelemetryRecorder {
    cache_path: PathBuf,
}

impl SkillTelemetryRecorder for JournalTelemetryRecorder {
    fn record_probe(&mut self, skill_id: &str, success: bool, duration_ms: u64, error: Option<&str>) {
        RegistryCache::with_update(&self.cache_path, |cache| {
            cache.record_execution(skill_id, success, duration_ms, error);
        }).ok(); // best-effort; telemetry is not critical path
    }

    fn record_intervention(&mut self, skill_id: &str, success: bool, error: Option<&str>) {
        RegistryCache::with_update(&self.cache_path, |cache| {
            cache.record_intervention(skill_id, success, error);
        }).ok();
    }

    fn compute_skill_health(&self, skill_id: &str) -> Option<SkillHealthSummary> {
        let cache = RegistryCache::load(&self.cache_path).ok()?;
        let entry = cache.skills.get(skill_id)?;
        Some(SkillHealthSummary {
            quality_score: entry.coverage_score,
            freshness: crate::registry::freshness_score(entry),
            probe_runs: entry.probe_runs,
        })
    }

    fn compute_health(&self) -> BTreeMap<String, SkillHealthSummary> {
        let cache = RegistryCache::load(&self.cache_path).unwrap_or_default();
        cache.skills.iter().map(|(id, entry)| {
            (id.clone(), SkillHealthSummary {
                quality_score: entry.coverage_score,
                freshness: crate::registry::freshness_score(entry),
                probe_runs: entry.probe_runs,
            })
        }).collect()
    }
}
```

---

## Port 5: `SkillEvaluator`

### Trait definition — `russell-core/src/port/skill_evaluator.rs`

```rust
/// Evaluates skill quality using multiple dimensions.
///
/// Adapters: QualityScorer (current, unwired), ScenarioTester (current, external).
pub trait SkillEvaluator: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Evaluate a skill and produce a health assessment.
    fn evaluate(&self, skill: &SkillDescriptor, telemetry: &SkillHealthSummary) -> Result<SkillHealth, Self::Error>;
}

/// Full skill health aggregate (see Task 6 for complete definition).
#[derive(Debug, Clone)]
pub struct SkillHealth {
    pub quality_score: f64,
    pub reliability: f64,
    pub latency_p95: Option<f64>,
    pub freshness_days: u32,
    pub safety_posture: SafetyPosture,
    pub staleness_days: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafetyPosture {
    Pass,
    Warn,
    Block,
}
```

### Adapter: `OkhSkillEvaluator` — `russell-skills/src/health/evaluator.rs` (proposed)

```rust
/// Evaluates skill health with OKH instrumentation.
/// Each dimension emits an okh.skill.eval.<dimension> tracing span.
pub struct OkhSkillEvaluator;

impl SkillEvaluator for OkhSkillEvaluator {
    type Error = EvaluationError;

    fn evaluate(&self, skill: &SkillDescriptor, telemetry: &SkillHealthSummary) -> Result<SkillHealth, Self::Error> {
        let quality = {
            let _span = tracing::info_span!("okh.skill.eval.quality", skill_id = %skill.id).entered();
            crate::registry::compute_quality_score_from_descriptor(skill)
        };
        // ... similar okh spans for reliability, latency, freshness, safety, staleness
        let _span = tracing::info_span!("okh.skill.eval.complete", skill_id = %skill.id, quality, reliability).entered();
        Ok(SkillHealth { quality_score: quality, /* ... */ })
    }
}
```

---

## Port 6: `PromptAssembler`

### Trait definition — `russell-core/src/port/prompt_assembler.rs`

```rust
/// Assembles the SOAP prompt from telemetry context and loaded skills.
///
/// Adapters: TemplatedPromptAssembler (current, active path target),
///           LegacyPromptAssembler (current, deprecated).
pub trait PromptAssembler: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Assemble a SOAP prompt from the given context.
    fn assemble(&self, context: PromptContext) -> Result<SoapPrompt, Self::Error>;
}

/// All data needed to build a prompt.
pub struct PromptContext<'a> {
    pub subjective: Option<&'a str>,
    pub profile: Option<&'a Profile>,
    pub journal_reader: &'a JournalReader,
    pub loaded_skills: &'a [SkillDescriptor],
    pub skills_base_dir: &'a Path,
    pub kask_tool_names: &'a [(String, Option<String>)],
    pub skill_registry: Option<&'a dyn SkillIndex>,
    pub telemetry: Option<&'a dyn SkillTelemetryRecorder>,
}
```

### Adapter: `TemplatedPromptAssembler` — `russell-meta/src/prompt.rs` (refactored)

```rust
/// Assembles prompts using the MiniJinja template registry.
/// Mirrors the existing compose_templated() function at prompt.rs:357
/// but takes PromptContext as a structured input rather than positional args.
pub struct TemplatedPromptAssembler {
    registry: PromptRegistry,
}

impl PromptAssembler for TemplatedPromptAssembler {
    type Error = crate::error::MetaError;

    fn assemble(&self, ctx: PromptContext) -> Result<SoapPrompt, Self::Error> {
        compose_templated(
            &self.registry,
            ctx.journal_reader,
            ctx.profile,
            ctx.subjective,
            ctx.loaded_skills_descriptors(), // convert from SkillDescriptor
            ctx.skills_base_dir,
            ctx.kask_tool_names,
            ctx.skill_registry.map(|r| convert_index(r)),
        )
    }
}
```

### Adapter: `LegacyPromptAssembler` — `russell-meta/src/prompt.rs` (deprecated, to be removed)

```rust
/// Legacy procedural prompt assembler.
/// Wraps compose_with_kask() at prompt.rs:62.
/// Marked #[deprecated] — will be removed in Task 7.
pub struct LegacyPromptAssembler;

impl PromptAssembler for LegacyPromptAssembler { /* ... */ }
```

---

## Port 7: `ActionResolver`

### Trait definition — `russell-core/src/port/action_resolver.rs`

```rust
/// Resolves ACTION: protocol lines into executable actions.
///
/// Adapters: LocalActionResolver (current), KaskMCPResolver (current via russell-mcp).
pub trait ActionResolver: Send + Sync {
    /// Resolve an ACTION: line from the LLM response.
    fn resolve(&self, response: &str) -> Option<Result<ResolvedAction, ActionError>>;
}
```

### Adapter: `LocalActionResolver` — `russell-meta/src/action.rs` (refactored)

```rust
/// Resolves local skill ACTIONS (ACTION: skill-id/probe-or-intervention-id).
/// Mirrors the existing resolve() function at action.rs:251.
pub struct LocalActionResolver {
    loaded_skills: Vec<SkillDescriptor>,
    kask_tools: Vec<KaskToolInfo>,
}

impl ActionResolver for LocalActionResolver {
    fn resolve(&self, response: &str) -> Option<Result<ResolvedAction, ActionError>> {
        resolve_with_kask(response, &self.loaded_skills_converted(), &self.kask_tools)
    }
}
```

---

## Port 8: `KnowledgeInjector`

### Trait definition — `russell-core/src/port/knowledge_injector.rs`

```rust
/// Injects KNOWLEDGE.md content into the system prompt, constrained by budget.
///
/// Adapters: RelevanceScoredInjector (current), FullContextInjector (legacy, to be removed).
pub trait KnowledgeInjector: Send + Sync {
    /// Append knowledge content to the system prompt.
    /// Returns the number of tokens injected.
    fn inject(&self, system_prompt: &mut String, context: &KnowledgeContext) -> usize;
}

pub struct KnowledgeContext<'a> {
    pub loaded_skills: &'a [SkillDescriptor],
    pub skills_base_dir: &'a Path,
    pub active_symptoms: &'a [String],
    pub budget_tokens: usize,
    pub telemetry: Option<&'a dyn SkillTelemetryRecorder>,
}
```

### Adapter: `RelevanceScoredInjector` — `russell-meta/src/prompt_registry.rs` (refactored)

```rust
/// Injects knowledge using relevance scoring and token budgeting.
/// Mirrors append_skill_knowledge_scored() at prompt.rs:575 plus
/// select_knowledge() at prompt_registry.rs:421.
pub struct RelevanceScoredInjector;

impl KnowledgeInjector for RelevanceScoredInjector {
    fn inject(&self, system_prompt: &mut String, ctx: &KnowledgeContext) -> usize {
        // Existing select_knowledge + append logic, generalized
        todo!()
    }
}
```

---

## Cross-Crate Dependency DAG

```
russell-core          ← Port definitions (traits, SkillDescriptor, SkillRef, etc.)
    ↑
    ├── russell-skills   ← Adapters: FilesystemSkillLoader, SafetyValidator, etc.
    ├── russell-meta     ← Adapters: TemplatedPromptAssembler, LocalActionResolver, etc.
    ├── russell-sentinel ← ProbeDescriptor port + adapters
    ├── russell-proprio  ← TimerSource port + adapters
    ├── russell-mcp      ← TokenProvider port + adapters
    └── russell-cli      ← Wiring (no implementations, just DI)
```

Following the established pattern: `russell-cli` depends on all crates but `russell-core` depends on nothing.

---

## Port Wiring in CLI Bootstrap

```rust
// crates/russell-cli/src/main.rs (pseudocode)
fn build_dependencies(config: &Config) -> Result<Dependencies> {
    // Ports
    let skill_loader: Box<dyn SkillLoader<Error = LoadError>> =
        Box::new(FilesystemSkillLoader::new(config.skills_dir.clone()));

    let skill_validator: Box<dyn SkillValidator<Error = LoadError>> =
        Box::new(ChainedValidator::new(vec![
            Box::new(ManifestValidator),
            Box::new(PokaYokeValidator::new(load_symptom_catalog())),
            Box::new(SafetyScannerAdapter),
        ]));

    let skill_index: Box<dyn SkillIndex> =
        Box::new(RegistryCache::load(&config.registry_cache_path)?);

    let telemetry: Box<dyn SkillTelemetryRecorder> =
        Box::new(JournalTelemetryRecorder::new(config.registry_cache_path.clone()));

    let evaluator: Box<dyn SkillEvaluator<Error = EvaluationError>> =
        Box::new(OkhSkillEvaluator);

    let prompt_assembler: Box<dyn PromptAssembler<Error = MetaError>> =
        Box::new(TemplatedPromptAssembler::new(PromptRegistry::with_defaults()?));

    let knowledge_injector: Box<dyn KnowledgeInjector> =
        Box::new(RelevanceScoredInjector);

    let action_resolver: Box<dyn ActionResolver> =
        Box::new(LocalActionResolver::new(loaded_skills, kask_tools));

    Ok(Dependencies { /* ... */ })
}
```
