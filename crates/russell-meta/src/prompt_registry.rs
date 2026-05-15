// SPDX-License-Identifier: MIT OR Apache-2.0
//! Prompt registry — MiniJinja-based template loading, rendering,
//! inference hint extraction, and disk overrides.
//!
//! This module replaces the procedural `writeln!()` prompt assembly
//! with data-driven `.md.j2` templates. The architectural pattern
//! follows `stack-prompts` from the Kask ecosystem, adapted for
//! Russell's single-persona, single-cycle scope (JR-1: austere).
//!
//! ## Template format
//!
//! Templates are MiniJinja (Jinja2) files with an optional `[inference]`
//! TOML header that declares per-template LLM parameters:
//!
//! ```text
//! [inference]
//! temperature = 0.2
//! max_tokens = 4096
//!
//! # SOAP — russell help
//! {{ subjective }}
//! ```
//!
//! ## Disk overrides
//!
//! Operators can drop `.md.j2` files in `~/.config/harness/prompts/`
//! to override any compiled-in template without recompiling.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use minijinja::Environment;

use crate::error::Result;

// ─── Compiled-in templates ────────────────────────────────────────────────

/// SOAP one-shot template — used by `russell jack`.
const TEMPLATE_SOAP: &str = include_str!("../prompts/templates/soap.md.j2");

/// Chat objective template — used by `russell chat`.
const TEMPLATE_CHAT_OBJECTIVE: &str = include_str!("../prompts/templates/chat_objective.md.j2");

/// Workshop template — used by `russell workshop`.
const TEMPLATE_WORKSHOP: &str = include_str!("../prompts/templates/workshop.md.j2");

/// Known template names and their bodies for compile-time registration.
static DEFAULT_TEMPLATES: &[(&str, &str)] = &[
    ("soap", TEMPLATE_SOAP),
    ("chat_objective", TEMPLATE_CHAT_OBJECTIVE),
    ("workshop", TEMPLATE_WORKSHOP),
];

// ─── Inference hints ──────────────────────────────────────────────────────

/// LLM parameters extracted from a template's `[inference]` header.
#[derive(Debug, Clone, Default)]
pub struct InferenceHint {
    /// Sampling temperature (0.0–2.0).
    pub temperature: Option<f64>,
    /// Maximum tokens to generate.
    pub max_tokens: Option<u32>,
    /// Reasoning effort level (for models that support it).
    pub reasoning_effort: Option<String>,
}

impl InferenceHint {
    /// Whether any parameter is set.
    pub fn is_empty(&self) -> bool {
        self.temperature.is_none() && self.max_tokens.is_none() && self.reasoning_effort.is_none()
    }
}

/// Parse the `[inference]` TOML-style header from a template body.
///
/// Returns `(hint, stripped_body)`. If no header is present, hint is None
/// and the body is returned unchanged.
///
/// The `[inference]` block may be preceded by a Jinja comment `{# ... #}`.
/// The parser skips any such comment and leading whitespace before looking
/// for the `[inference]` tag.
fn parse_inference_header(body: &str) -> (Option<InferenceHint>, &str) {
    // Skip leading Jinja comment block if present.
    let mut search_start = body;
    let mut prefix_len = 0;
    let trimmed = body.trim_start();
    if trimmed.starts_with("{#") {
        if let Some(end) = trimmed.find("#}") {
            let after_comment = &trimmed[end + 2..];
            let after_trimmed = after_comment.trim_start();
            prefix_len = body.len() - after_trimmed.len();
            search_start = after_trimmed;
        }
    } else {
        prefix_len = body.len() - trimmed.len();
        search_start = trimmed;
    }

    if !search_start.starts_with("[inference]") {
        return (None, body);
    }

    // Find the end of the header block (first blank line after key=value lines).
    let mut hint = InferenceHint::default();
    let mut end_offset = prefix_len; // byte offset into `body` where content starts

    // Skip the "[inference]" line itself.
    let mut lines = search_start.lines();
    let first_line = lines.next().unwrap_or(""); // "[inference]"
    end_offset += first_line.len() + 1; // +1 for \n

    for line in lines {
        let trimmed_line = line.trim();
        if trimmed_line.is_empty() {
            // Blank line ends the header block.
            end_offset += line.len() + 1;
            break;
        }
        if let Some((key, value)) = trimmed_line.split_once('=') {
            let key = key.trim();
            let value = value.trim().trim_matches('"');
            match key {
                "temperature" => hint.temperature = value.parse().ok(),
                "max_tokens" => hint.max_tokens = value.parse().ok(),
                "reasoning_effort" => hint.reasoning_effort = Some(value.to_string()),
                _ => {} // ignore unknown keys
            }
        }
        end_offset += line.len() + 1;
    }

    let stripped = if end_offset <= body.len() {
        &body[end_offset..]
    } else {
        ""
    };

    (Some(hint), stripped)
}

// ─── Template metadata ────────────────────────────────────────────────────

/// Origin of a template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateSource {
    /// Compiled into the binary via `include_str!()`.
    CompiledIn,
    /// Loaded from an operator-provided file on disk.
    Disk {
        /// Path to the override file.
        path: PathBuf,
    },
}

/// Metadata for a registered template.
#[derive(Debug, Clone)]
pub struct TemplateInfo {
    /// Template name (e.g., "soap", "chat_objective").
    pub name: String,
    /// Inference hint parsed from the `[inference]` header.
    pub inference_hint: Option<InferenceHint>,
    /// Where the template was loaded from.
    pub source: TemplateSource,
}

// ─── Prompt registry ──────────────────────────────────────────────────────

/// The prompt registry — manages template loading, rendering, and
/// inference hint extraction.
///
/// Templates are loaded from compiled-in `.md.j2` files at startup,
/// with optional disk overrides from `~/.config/harness/prompts/`.
pub struct PromptRegistry {
    env: Environment<'static>,
    templates: HashMap<String, TemplateInfo>,
}

impl PromptRegistry {
    /// Create a registry with all compiled-in default templates.
    pub fn with_defaults() -> Result<Self> {
        let mut env = Environment::new();
        env.set_undefined_behavior(minijinja::UndefinedBehavior::Lenient);

        // Custom filters for prompt-specific transforms.
        env.add_filter("truncate_tokens", truncate_tokens_filter);

        let mut templates = HashMap::new();

        for &(name, body) in DEFAULT_TEMPLATES {
            let (hint, stripped) = parse_inference_header(body);
            env.add_template_owned(name.to_string(), stripped.to_string())
                .map_err(|e| {
                    crate::error::DoctorError::Prompt(format!(
                        "Failed to compile template '{name}': {e}"
                    ))
                })?;
            templates.insert(
                name.to_string(),
                TemplateInfo {
                    name: name.to_string(),
                    inference_hint: hint,
                    source: TemplateSource::CompiledIn,
                },
            );
        }

        Ok(Self { env, templates })
    }

    /// Load disk overrides from a directory, replacing matching templates.
    ///
    /// Only `.md.j2` files whose stem matches an existing template name
    /// are loaded. Returns the number of templates overridden.
    pub fn load_disk_overrides(&mut self, dir: &Path) -> Result<usize> {
        if !dir.is_dir() {
            return Ok(0);
        }
        let entries = std::fs::read_dir(dir).map_err(|e| {
            crate::error::DoctorError::Prompt(format!(
                "Failed to read prompt override directory '{}': {e}",
                dir.display()
            ))
        })?;

        let mut count = 0;
        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) if n.ends_with(".md.j2") => n.to_string(),
                _ => continue,
            };
            let template_name = file_name.trim_end_matches(".md.j2");
            if !self.templates.contains_key(template_name) {
                tracing::debug!(
                    file = %path.display(),
                    "Skipping disk template — no matching default"
                );
                continue;
            }

            let body = match std::fs::read_to_string(&path) {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!(
                        file = %path.display(),
                        error = %e,
                        "Failed to read disk template override"
                    );
                    continue;
                }
            };

            let (hint, stripped) = parse_inference_header(&body);
            match self
                .env
                .add_template_owned(template_name.to_string(), stripped.to_string())
            {
                Ok(()) => {
                    self.templates.insert(
                        template_name.to_string(),
                        TemplateInfo {
                            name: template_name.to_string(),
                            inference_hint: hint,
                            source: TemplateSource::Disk {
                                path: path.clone(),
                            },
                        },
                    );
                    tracing::info!(
                        template = %template_name,
                        path = %path.display(),
                        "Loaded disk template override"
                    );
                    count += 1;
                }
                Err(e) => {
                    tracing::warn!(
                        template = %template_name,
                        path = %path.display(),
                        error = %e,
                        "Failed to compile disk template override — using default"
                    );
                }
            }
        }
        Ok(count)
    }

    /// Render a template by name with the given context variables.
    ///
    /// Returns the rendered text. Use [`render_with_hint`] to also
    /// retrieve the inference hint.
    pub fn render(
        &self,
        template_name: &str,
        ctx: &HashMap<String, serde_json::Value>,
    ) -> Result<String> {
        let tmpl = self.env.get_template(template_name).map_err(|e| {
            crate::error::DoctorError::Prompt(format!(
                "Template '{template_name}' not found: {e}"
            ))
        })?;

        let jinja_ctx = minijinja::Value::from_serialize(ctx);
        let rendered = tmpl.render(jinja_ctx).map_err(|e| {
            crate::error::DoctorError::Prompt(format!(
                "Template '{template_name}' render error: {e}"
            ))
        })?;

        tracing::debug!(
            template = %template_name,
            chars = rendered.len(),
            tokens_est = rendered.len() / 4,
            "Prompt template rendered"
        );

        Ok(rendered)
    }

    /// Render a template and return both the rendered text and inference hint.
    pub fn render_with_hint(
        &self,
        template_name: &str,
        ctx: &HashMap<String, serde_json::Value>,
    ) -> Result<(String, Option<InferenceHint>)> {
        let rendered = self.render(template_name, ctx)?;
        let hint = self
            .templates
            .get(template_name)
            .and_then(|info| info.inference_hint.clone());
        Ok((rendered, hint))
    }

    /// Get the inference hint for a template without rendering.
    pub fn inference_hint(&self, template_name: &str) -> Option<&InferenceHint> {
        self.templates
            .get(template_name)
            .and_then(|info| info.inference_hint.as_ref())
    }

    /// List all registered template names.
    pub fn template_names(&self) -> Vec<&str> {
        self.templates.keys().map(|s| s.as_str()).collect()
    }

    /// Get metadata for a template.
    pub fn template_info(&self, name: &str) -> Option<&TemplateInfo> {
        self.templates.get(name)
    }

    /// Default path for disk overrides.
    pub fn default_overrides_path() -> Option<PathBuf> {
        dirs_path("prompts")
    }
}

/// Resolve the standard config path for a Russell subdirectory.
fn dirs_path(subdir: &str) -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(PathBuf::from(home).join(".config").join("harness").join(subdir))
}

/// MiniJinja filter: truncate text to approximately N tokens (4 bytes/token).
fn truncate_tokens_filter(s: String, limit: u32) -> String {
    let byte_limit = limit as usize * 4;
    if s.len() <= byte_limit {
        return s;
    }
    let mut end = byte_limit.saturating_sub(3);
    while end > 0 && !s.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    if end == 0 {
        return format!("{}...", &s[..byte_limit.min(s.len())]);
    }
    format!("{}...", &s[..end])
}

// ─── Prompt mode ──────────────────────────────────────────────────────────

/// Which prompt template to use, determined by invocation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptMode {
    /// One-shot SOAP health check (`russell jack`).
    Soap,
    /// Multi-turn chat REPL (`russell chat`).
    Chat,
    /// Skill workshop mode (`russell workshop`).
    Workshop,
}

impl PromptMode {
    /// The template name for this mode.
    pub fn template_name(self) -> &'static str {
        match self {
            Self::Soap => "soap",
            Self::Chat => "chat_objective",
            Self::Workshop => "workshop",
        }
    }
}

// ─── Knowledge budget ─────────────────────────────────────────────────────

/// A knowledge slot scored for relevance-based injection.
#[derive(Debug, Clone)]
pub struct KnowledgeSlot {
    /// Skill ID that owns this knowledge.
    pub skill_id: String,
    /// The KNOWLEDGE.md content.
    pub content: String,
    /// Relevance score 0.0–1.0 (based on symptom overlap with current state).
    pub relevance: f64,
    /// Estimated token count (~4 chars/token).
    pub token_estimate: usize,
}

/// Select knowledge slots that fit within the token budget, ordered
/// by relevance (highest first).
pub fn select_knowledge(slots: &mut [KnowledgeSlot], budget_tokens: usize) -> Vec<&KnowledgeSlot> {
    slots.sort_by(|a, b| b.relevance.partial_cmp(&a.relevance).unwrap_or(std::cmp::Ordering::Equal));
    let mut remaining = budget_tokens;
    let mut selected = Vec::new();
    for slot in slots.iter() {
        if slot.token_estimate > remaining {
            continue;
        }
        remaining -= slot.token_estimate;
        selected.push(slot);
    }
    selected
}

/// Score a knowledge skill's relevance based on symptom overlap
/// with the current alert state.
///
/// `skill_symptoms` — symptoms the knowledge skill covers (catalog entries
/// like "vram_oom", "llm_slow").
/// `active_symptoms` — signals derived from recent events (probe names,
/// keywords like "vram", "gpu", "swap", "timeout").
///
/// Matching is **keyword-based**: a skill symptom "vram_oom" matches if
/// any active symptom contains "vram" or "oom" as a substring, or vice versa.
/// This bridges the vocabulary gap between the formal symptom catalog and
/// the runtime signals extracted from journal events.
///
/// Returns 0.0 if no overlap, up to 1.0 for full coverage.
pub fn score_knowledge_relevance(
    skill_symptoms: &[String],
    active_symptoms: &[String],
) -> f64 {
    if skill_symptoms.is_empty() || active_symptoms.is_empty() {
        // Knowledge with no symptoms gets a base relevance (always somewhat useful).
        return 0.3;
    }
    let overlap = skill_symptoms
        .iter()
        .filter(|skill_sym| {
            // A skill symptom matches if ANY active symptom overlaps by keyword.
            active_symptoms.iter().any(|active| {
                // Exact match.
                if skill_sym.as_str() == active.as_str() {
                    return true;
                }
                // Keyword containment: "vram_oom" contains "vram", or "vram" is in "gpu_vram_used_pct".
                if skill_sym.contains(active.as_str()) || active.contains(skill_sym.as_str()) {
                    return true;
                }
                // Split skill symptom into keywords and check any match.
                skill_sym.split('_').any(|kw| kw.len() >= 3 && active.contains(kw))
            })
        })
        .count();
    if overlap == 0 {
        0.2 // base relevance for applicable knowledge
    } else {
        0.4 + 0.6 * (overlap as f64 / skill_symptoms.len().max(1) as f64)
    }
}

/// Runtime telemetry signals that modulate knowledge relevance.
///
/// These represent the **inter-session feedback loop**: past execution
/// outcomes influence future attention allocation.
#[derive(Debug, Clone, Default)]
pub struct SkillTelemetry {
    /// Freshness score 0.0–1.0 (from probe success rate). 0 = never run.
    pub freshness: f64,
    /// Total probe executions (higher = more battle-tested).
    pub probe_runs: u64,
    /// Recent probe failure count.
    pub recent_failures: u64,
    /// Total intervention executions.
    pub intervention_runs: u64,
    /// Recent intervention failures.
    pub recent_intervention_failures: u64,
}

/// Score knowledge relevance with inter-session telemetry feedback.
///
/// This is the **closed-loop** version of `score_knowledge_relevance`:
/// it incorporates runtime execution history to boost skills that have
/// been reliable and penalize those that have been failing.
///
/// Scoring formula:
/// ```text
/// final_score = symptom_score * reliability_modifier
/// ```
///
/// Where `reliability_modifier`:
/// - 1.0 if no telemetry (new skill, benefit of the doubt)
/// - 1.0 + 0.2 * freshness for battle-tested reliable skills (up to 1.2×)
/// - 1.0 - 0.3 * failure_rate for failing skills (down to 0.7×)
pub fn score_knowledge_relevance_with_telemetry(
    skill_symptoms: &[String],
    active_symptoms: &[String],
    telemetry: &SkillTelemetry,
) -> f64 {
    let base_score = score_knowledge_relevance(skill_symptoms, active_symptoms);

    // No telemetry → no modifier (benefit of the doubt for new skills).
    if telemetry.probe_runs == 0 && telemetry.intervention_runs == 0 {
        return base_score;
    }

    // Compute reliability modifier from execution history.
    let total_runs = telemetry.probe_runs + telemetry.intervention_runs;
    let total_failures = telemetry.recent_failures + telemetry.recent_intervention_failures;

    if total_runs == 0 {
        return base_score;
    }

    let failure_rate = total_failures as f64 / total_runs as f64;

    let modifier = if failure_rate < 0.1 {
        // Reliable skill: boost proportional to freshness (how active it's been).
        1.0 + 0.2 * telemetry.freshness
    } else if failure_rate > 0.5 {
        // Unreliable skill: significant penalty.
        0.7
    } else {
        // Middle ground: linear interpolation.
        1.0 - 0.3 * failure_rate
    };

    (base_score * modifier).clamp(0.0, 1.0)
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_inference_header_extracts_params() {
        let body = "[inference]\ntemperature = 0.2\nmax_tokens = 4096\n\n# Hello\n{{ var }}";
        let (hint, stripped) = parse_inference_header(body);
        let hint = hint.unwrap();
        assert_eq!(hint.temperature, Some(0.2));
        assert_eq!(hint.max_tokens, Some(4096));
        assert!(stripped.contains("# Hello"));
        assert!(!stripped.contains("[inference]"));
    }

    #[test]
    fn parse_inference_header_no_header() {
        let body = "# Just a template\n{{ var }}";
        let (hint, stripped) = parse_inference_header(body);
        assert!(hint.is_none());
        assert_eq!(stripped, body);
    }

    #[test]
    fn registry_loads_defaults() {
        let reg = PromptRegistry::with_defaults().unwrap();
        assert!(reg.templates.contains_key("soap"));
        assert!(reg.templates.contains_key("chat_objective"));
        assert!(reg.templates.contains_key("workshop"));
    }

    #[test]
    fn registry_renders_soap_template() {
        let reg = PromptRegistry::with_defaults().unwrap();
        let mut ctx = HashMap::new();
        ctx.insert("subjective".to_string(), serde_json::json!("test note"));
        ctx.insert("profile_block".to_string(), serde_json::json!("- os: linux"));
        ctx.insert(
            "severity_block".to_string(),
            serde_json::json!("- info: 5 | warn: 1 | alert: 0 | crit: 0"),
        );
        ctx.insert(
            "samples_table".to_string(),
            serde_json::json!("(no samples recorded)"),
        );
        ctx.insert(
            "freshness_block".to_string(),
            serde_json::json!("- Last sample 180 seconds ago."),
        );
        ctx.insert(
            "events_table".to_string(),
            serde_json::json!("- (no events recorded)"),
        );

        let (rendered, hint) = reg.render_with_hint("soap", &ctx).unwrap();
        assert!(rendered.contains("## Subjective"));
        assert!(rendered.contains("test note"));
        assert!(rendered.contains("## Assessment"));
        let hint = hint.unwrap();
        assert_eq!(hint.temperature, Some(0.2));
        assert_eq!(hint.max_tokens, Some(4096));
    }

    #[test]
    fn disk_override_replaces_template() {
        let tmp = tempfile::tempdir().unwrap();
        let override_path = tmp.path().join("soap.md.j2");
        std::fs::write(
            &override_path,
            "[inference]\ntemperature = 0.8\n\nCustom: {{ subjective }}",
        )
        .unwrap();

        let mut reg = PromptRegistry::with_defaults().unwrap();
        let count = reg.load_disk_overrides(tmp.path()).unwrap();
        assert_eq!(count, 1);

        let info = reg.template_info("soap").unwrap();
        assert_eq!(info.source, TemplateSource::Disk { path: override_path });
        assert_eq!(info.inference_hint.as_ref().unwrap().temperature, Some(0.8));

        let mut ctx = HashMap::new();
        ctx.insert("subjective".to_string(), serde_json::json!("hello"));
        let rendered = reg.render("soap", &ctx).unwrap();
        assert!(rendered.contains("Custom: hello"));
    }

    #[test]
    fn knowledge_relevance_scoring() {
        let skill_symptoms = vec!["llm_slow".to_string(), "gpu_fallback_to_cpu".to_string()];
        let active = vec!["llm_slow".to_string(), "swap_pressure".to_string()];
        let score = score_knowledge_relevance(&skill_symptoms, &active);
        // "llm_slow" exact match → 1 overlap out of 2 → 0.4 + 0.6*(1/2) = 0.7
        assert!((score - 0.7).abs() < 0.01, "expected ~0.7, got {score}");
    }

    #[test]
    fn knowledge_relevance_keyword_match() {
        // "vram_oom" should match active signal "vram" via keyword extraction.
        let skill_symptoms = vec!["vram_oom".to_string()];
        let active = vec!["vram".to_string(), "gpu".to_string()];
        let score = score_knowledge_relevance(&skill_symptoms, &active);
        // "vram_oom" contains "vram" → 1 overlap out of 1 → 0.4 + 0.6 = 1.0
        assert!((score - 1.0).abs() < 0.01, "expected 1.0, got {score}");
    }

    #[test]
    fn knowledge_relevance_no_active_symptoms() {
        let skill_symptoms = vec!["llm_slow".to_string()];
        let active: Vec<String> = vec![];
        let score = score_knowledge_relevance(&skill_symptoms, &active);
        // Empty active → base relevance 0.3 (always somewhat useful)
        assert!((score - 0.3).abs() < 0.01, "expected 0.3, got {score}");
    }

    #[test]
    fn knowledge_relevance_both_empty() {
        let skill_symptoms: Vec<String> = vec![];
        let active: Vec<String> = vec![];
        let score = score_knowledge_relevance(&skill_symptoms, &active);
        // Both empty → 0.3 base relevance
        assert!((score - 0.3).abs() < 0.01, "expected 0.3, got {score}");
    }

    #[test]
    fn knowledge_selection_respects_budget() {
        let mut slots = vec![
            KnowledgeSlot {
                skill_id: "a".to_string(),
                content: "big".to_string(),
                relevance: 0.9,
                token_estimate: 500,
            },
            KnowledgeSlot {
                skill_id: "b".to_string(),
                content: "small".to_string(),
                relevance: 0.8,
                token_estimate: 200,
            },
            KnowledgeSlot {
                skill_id: "c".to_string(),
                content: "medium".to_string(),
                relevance: 0.7,
                token_estimate: 400,
            },
        ];
        let selected = select_knowledge(&mut slots, 700);
        // Budget 700: 'a' (500, 0.9) fits, 'b' (200, 0.8) fits, 'c' (400) doesn't
        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].skill_id, "a");
        assert_eq!(selected[1].skill_id, "b");
    }

    #[test]
    fn telemetry_boosts_reliable_skill() {
        // Use no structural floor so base < 1.0, making the boost visible.
        let active = vec!["llm_slow".to_string()];
        let symptoms2 = vec!["llm_slow".to_string(), "gpu_fallback_to_cpu".to_string()];
        let base2 = score_knowledge_relevance(&symptoms2, &active);
        // Without structural floor: 0.4 + 0.6*(1/2) = 0.7

        let telemetry = SkillTelemetry {
            freshness: 1.0, // perfect freshness
            probe_runs: 100,
            recent_failures: 0,
            intervention_runs: 10,
            recent_intervention_failures: 0,
        };
        let boosted = score_knowledge_relevance_with_telemetry(&symptoms2, &active, &telemetry);
        // Reliable skill gets 1.2× multiplier: 0.7 * 1.2 = 0.84 > 0.7
        assert!(boosted > base2, "expected boosted ({boosted}) > base ({base2})");
    }

    #[test]
    fn telemetry_penalizes_failing_skill() {
        // Use no structural floor + 2 symptoms with 1 overlap for visible penalty.
        let symptoms = vec!["llm_slow".to_string(), "gpu_fallback_to_cpu".to_string()];
        let active = vec!["llm_slow".to_string()];
        let base = score_knowledge_relevance(&symptoms, &active);
        // base = 0.4 + 0.6*(1/2) = 0.7

        let telemetry = SkillTelemetry {
            freshness: 0.4,
            probe_runs: 100,
            recent_failures: 60,
            intervention_runs: 10,
            recent_intervention_failures: 8,
        };
        let penalized = score_knowledge_relevance_with_telemetry(&symptoms, &active, &telemetry);
        // Failing skill should be penalized below base (down to 0.7×)
        assert!(penalized < base, "expected penalized ({penalized}) < base ({base})");
    }

    #[test]
    fn telemetry_neutral_for_new_skill() {
        let symptoms = vec!["llm_slow".to_string(), "gpu_fallback_to_cpu".to_string()];
        let active = vec!["llm_slow".to_string()];
        let base = score_knowledge_relevance(&symptoms, &active);

        let telemetry = SkillTelemetry::default(); // no runs
        let scored = score_knowledge_relevance_with_telemetry(&symptoms, &active, &telemetry);
        // New skill gets no modifier
        assert!((scored - base).abs() < 0.001, "expected same as base, got {scored} vs {base}");
    }

    #[test]
    fn truncate_tokens_filter_short_passthrough() {
        let s = "hello world".to_string();
        assert_eq!(truncate_tokens_filter(s.clone(), 100), s);
    }

    #[test]
    fn truncate_tokens_filter_truncates() {
        let s = "a".repeat(500);
        let result = truncate_tokens_filter(s, 10); // 10 tokens = ~40 chars
        assert!(result.len() < 50);
        assert!(result.ends_with("..."));
    }
}
