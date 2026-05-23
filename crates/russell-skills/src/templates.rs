// SPDX-License-Identifier: MIT OR Apache-2.0
//! Jinja2 template support for skill prompts.
//!
//! Template crates extend skills with LLM-powered prompt generation.
//! Each template lives under `templates/*.j2` and is rendered with
//! context from probe results, journal state, and operator input.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use minijinja::{Environment, Error as JinjaError};
use serde::Serialize;

/// Template rendering errors.
#[derive(Debug, thiserror::Error)]
pub enum TemplateError {
    /// Template file not found.
    #[error("template not found: {0}")]
    NotFound(String),
    
    /// Failed to read template file.
    #[error("cannot read template {path}: {source}")]
    ReadFailed {
        /// Template path.
        path: String,
        /// Underlying IO error.
        #[source]
        source: std::io::Error,
    },
    
    /// Jinja2 rendering error.
    #[error("template render failed: {0}")]
    RenderFailed(#[from] JinjaError),
    
    /// Invalid template syntax.
    #[error("invalid template syntax in {path}: {message}")]
    InvalidSyntax {
        /// Template path.
        path: String,
        /// Error message.
        message: String,
    },
}

/// Template rendering context.
///
/// Passed to Jinja2 templates as the `context` variable.
#[derive(Debug, Clone, Serialize)]
pub struct TemplateContext {
    /// Probe results (probe_id -> output).
    #[serde(default)]
    pub probes: BTreeMap<String, String>,
    
    /// Journal state summary.
    #[serde(default)]
    pub journal: JournalContext,
    
    /// Operator-provided parameters.
    #[serde(default)]
    pub params: BTreeMap<String, serde_json::Value>,
    
    /// Skill metadata.
    #[serde(default)]
    pub skill: SkillContext,
    
    /// Host telemetry (if available).
    #[serde(default)]
    pub host: HostContext,
}

impl Default for TemplateContext {
    fn default() -> Self {
        Self {
            probes: BTreeMap::new(),
            journal: JournalContext::default(),
            params: BTreeMap::new(),
            skill: SkillContext::default(),
            host: HostContext::default(),
        }
    }
}

/// Journal context for templates.
#[derive(Debug, Clone, Default, Serialize)]
pub struct JournalContext {
    /// Total event count.
    pub event_count: u64,
    /// Last sample timestamp (ISO 8601).
    pub last_sample: Option<String>,
    /// Threshold breaches in last 24h.
    pub breaches_24h: u64,
    /// Recent severity distribution.
    pub severity_dist: BTreeMap<String, u64>,
}

/// Skill metadata context.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SkillContext {
    /// Skill ID.
    pub id: String,
    /// Skill version.
    pub version: String,
    /// Current dispatch ID (if any).
    pub dispatch_id: Option<String>,
}

/// Host telemetry context.
#[derive(Debug, Clone, Default, Serialize)]
pub struct HostContext {
    /// CPU usage percentage (0-100).
    pub cpu_pct: Option<f64>,
    /// Memory usage percentage (0-100).
    pub mem_pct: Option<f64>,
    /// Disk usage percentage (0-100).
    pub disk_pct: Option<f64>,
    /// GPU VRAM usage percentage (0-100).
    pub gpu_vram_pct: Option<f64>,
    /// GPU temperature in Celsius.
    pub gpu_temp_c: Option<f64>,
}

/// Template engine for skill prompts.
///
/// Wraps MiniJinja environment with skill-specific helpers.
pub struct TemplateEngine {
    env: Environment<'static>,
}

impl TemplateEngine {
    /// Create a new template engine.
    pub fn new() -> Self {
        let env = Environment::new();
        Self { env }
    }
    
    /// Create engine with skill-specific helpers.
    pub fn with_helpers() -> Self {
        let mut env = Environment::new();
        
        // Add default filter
        env.add_filter("default", |value: Option<String>, default: String| {
            Ok(value.unwrap_or(default))
        });
        
        // Add round filter
        env.add_filter("round", |value: f64, decimals: u8| {
            let factor = 10_f64.powi(decimals as i32);
            Ok((value * factor).round() / factor)
        });
        
        // Add upper filter
        env.add_filter("upper", |s: String| Ok(s.to_uppercase()));
        
        // Add lower filter
        env.add_filter("lower", |s: String| Ok(s.to_lowercase()));
        
        Self { env }
    }
    
    /// Load a template from file.
    pub fn load_template(&self, template_path: &Path) -> Result<String, TemplateError> {
        if !template_path.exists() {
            return Err(TemplateError::NotFound(
                template_path.display().to_string(),
            ));
        }
        
        std::fs::read_to_string(template_path).map_err(|e| TemplateError::ReadFailed {
            path: template_path.display().to_string(),
            source: e,
        })
    }
    
    /// Render a template with context.
    pub fn render(&self, template: &str, context: &TemplateContext) -> Result<String, TemplateError> {
        let rendered = self.env.render_str(template, context)?;
        Ok(rendered)
    }
    
    /// Render a template from file with context.
    pub fn render_file(&self, template_path: &Path, context: &TemplateContext) -> Result<String, TemplateError> {
        let template = self.load_template(template_path)?;
        self.render(&template, context)
    }
}

impl Default for TemplateEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Template crate structure.
///
/// Represents a skill with template support.
#[derive(Debug, Clone)]
pub struct TemplateCrate {
    /// Base directory path.
    pub base_dir: PathBuf,
    /// Templates directory.
    pub templates_dir: PathBuf,
    /// Available template names.
    pub templates: Vec<String>,
}

impl TemplateCrate {
    /// Load template crate from directory.
    pub fn load(base_dir: &Path) -> Result<Self, TemplateError> {
        let templates_dir = base_dir.join("templates");
        
        if !templates_dir.exists() {
            return Err(TemplateError::NotFound(
                templates_dir.display().to_string(),
            ));
        }
        
        let mut templates = Vec::new();
        
        if let Ok(entries) = std::fs::read_dir(&templates_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "j2") {
                    if let Some(name) = path.file_stem() {
                        templates.push(name.to_string_lossy().to_string());
                    }
                }
            }
        }
        
        templates.sort();
        
        Ok(Self {
            base_dir: base_dir.to_path_buf(),
            templates_dir,
            templates,
        })
    }
    
    /// Get template path by name.
    pub fn template_path(&self, name: &str) -> PathBuf {
        self.templates_dir.join(format!("{}.j2", name))
    }
    
    /// Check if template exists.
    pub fn has_template(&self, name: &str) -> bool {
        self.templates.contains(&name.to_string())
    }
}

/// Dispatch integration: render templates with probe/intervention results.
///
/// This function integrates templates with the dispatch pipeline.
pub fn render_dispatch_result(
    skill_id: &str,
    skill_dir: &Path,
    step_id: &str,
    step_type: StepType,
    stdout: &str,
    params: &serde_json::Value,
) -> Result<String, TemplateError> {
    // Load template crate
    let template_crate = TemplateCrate::load(skill_dir)?;
    
    // Build context
    let mut ctx = TemplateContext::default();
    ctx.skill.id = skill_id.to_string();
    ctx.params.insert("step_id".to_string(), serde_json::json!(step_id));
    ctx.params.insert("step_type".to_string(), serde_json::json!(step_type.as_str()));
    
    // Parse params if provided
    if let Some(p) = params.as_object() {
        for (k, v) in p {
            ctx.params.insert(k.clone(), v.clone());
        }
    }
    
    // Capture stdout as probe result
    ctx.probes.insert("stdout".to_string(), stdout.to_string());
    
    // Select template
    let _template_name = match step_type {
        StepType::Probe => format!("probe-{}", step_id),
        StepType::Intervention => format!("intervention-{}", step_id),
    };
    
    // Render with selector
    let selector_path = template_crate.template_path("selector");
    let engine = TemplateEngine::with_helpers();
    
    // First render selector to get response template
    let selector_result = engine.render_file(&selector_path, &ctx)?;
    let response_template = selector_result.trim();
    
    // Render response template
    let response_path = template_crate.template_path(response_template);
    engine.render_file(&response_path, &ctx)
}

/// Step type enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepType {
    /// Probe step (read-only).
    Probe,
    /// Intervention step (mutating).
    Intervention,
}

impl StepType {
    /// Convert to string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Probe => "probe",
            Self::Intervention => "intervention",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_template_context_serialization() {
        let mut ctx = TemplateContext::default();
        ctx.skill.id = "test-skill".to_string();
        ctx.skill.version = "1.0.0".to_string();
        ctx.params.insert("limit".to_string(), serde_json::json!(20));
        
        let json = serde_json::to_string(&ctx).unwrap();
        assert!(json.contains("test-skill"));
        assert!(json.contains("limit"));
    }
    
    #[test]
    fn test_template_render_basic() {
        let engine = TemplateEngine::new();
        let template = "Hello, {{ skill.id }}!";
        
        let mut ctx = TemplateContext::default();
        ctx.skill.id = "world".to_string();
        
        let result = engine.render(template, &ctx).unwrap();
        assert_eq!(result, "Hello, world!");
    }
    
    #[test]
    fn test_template_render_with_params() {
        let engine = TemplateEngine::new();
        let template = "Limit: {{ params.limit }}";
        
        let mut ctx = TemplateContext::default();
        ctx.params.insert("limit".to_string(), serde_json::json!(20));
        
        let result = engine.render(template, &ctx).unwrap();
        assert_eq!(result, "Limit: 20");
    }
    
    #[test]
    fn test_load_okapi_watcher_templates() {
        let crate_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../skills/okapi-watcher");
        
        if crate_path.exists() {
            let template_crate = TemplateCrate::load(&crate_path).expect("Failed to load template crate");
            assert!(template_crate.has_template("selector"));
            assert!(template_crate.has_template("health-ok"));
            
            let engine = TemplateEngine::with_helpers();
            let template_path = template_crate.template_path("health-ok");
            let mut ctx = TemplateContext::default();
            ctx.skill.id = "okapi-watcher".to_string();
            
            let result = engine.render_file(&template_path, &ctx);
            assert!(result.is_ok());
        }
    }
    
    #[test]
    fn test_helpers() {
        let engine = TemplateEngine::with_helpers();
        
        // Test default filter
        let result = engine.render("{{ value | default('fallback') }}", &TemplateContext::default()).unwrap();
        assert_eq!(result, "fallback");
        
        // Test round filter
        let mut ctx = TemplateContext::default();
        ctx.params.insert("num".to_string(), serde_json::json!(3.14159));
        let result = engine.render("{{ params.num | round(2) }}", &ctx).unwrap();
        assert!(result.contains("3.14"));
    }
}
