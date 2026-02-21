//! TOML configuration structs, parsing, and validation.
//!
//! A project TOML defines the full campaign: identity, targets, competitive
//! context, strategy (manual phases or auto-generated), infrastructure
//! requirements, budget limits, and fleet sizing.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::warn;

// ── TOML Configuration Structs ──────────────────────────────────

/// Top-level project configuration parsed from TOML files.
///
/// Maps directly to the `[project]`, `[target]`, `[competitive]`, `[strategy]`,
/// `[infrastructure]`, `[budget]`, and `[workers]` sections of a project TOML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub project: ProjectMeta,
    #[serde(default)]
    pub target: TargetConfig,
    pub competitive: Option<CompetitiveConfig>,
    #[serde(default)]
    pub strategy: StrategyConfig,
    pub infrastructure: Option<InfrastructureConfig>,
    pub budget: Option<BudgetConfig>,
    pub workers: Option<WorkerConfig>,
}

/// The `[project]` section: identity and classification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMeta {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub objective: Objective,
    pub form: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Project objective type — determines default strategy and success criteria.
///
/// - **Record**: hunt for a new world record in digit count.
/// - **Survey**: systematically enumerate a range for completeness.
/// - **Verification**: re-verify existing results.
/// - **Custom**: user-defined phases with no built-in strategy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Objective {
    Record,
    Survey,
    Verification,
    Custom,
}

impl std::fmt::Display for Objective {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Objective::Record => write!(f, "record"),
            Objective::Survey => write!(f, "survey"),
            Objective::Verification => write!(f, "verification"),
            Objective::Custom => write!(f, "custom"),
        }
    }
}

/// The `[target]` section: what the project aims to achieve.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TargetConfig {
    pub target_digits: Option<u64>,
    pub range_start: Option<u64>,
    pub range_end: Option<u64>,
}

/// The `[competitive]` section: current world record for comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompetitiveConfig {
    pub current_record_expression: Option<String>,
    pub current_record_digits: Option<u64>,
    pub current_record_holder: Option<String>,
    pub oeis_sequence: Option<String>,
    pub reference_urls: Option<Vec<String>>,
}

/// The `[strategy]` section: manual or auto-generated phase definitions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StrategyConfig {
    #[serde(default)]
    pub auto_strategy: bool,
    #[serde(default)]
    pub phases: Vec<PhaseConfig>,
}

/// A single phase within the strategy: a self-contained search job with
/// dependencies and activation conditions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseConfig {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub search_params: serde_json::Value,
    pub block_size: Option<i64>,
    pub depends_on: Option<Vec<String>>,
    pub activation_condition: Option<String>,
    #[serde(default = "default_completion")]
    pub completion: String,
}

fn default_completion() -> String {
    "all_blocks_done".to_string()
}

/// The `[infrastructure]` section: hardware requirements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfrastructureConfig {
    pub min_ram_gb: Option<u32>,
    pub min_cores: Option<u32>,
    pub recommended_cores: Option<u32>,
    #[serde(default)]
    pub required_tools: Vec<String>,
    #[serde(default)]
    pub preferred_tools: Vec<String>,
}

/// The `[budget]` section: cost limits and cloud pricing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetConfig {
    pub max_cost_usd: Option<f64>,
    pub cost_alert_threshold_usd: Option<f64>,
    #[serde(default = "default_cloud_rate")]
    pub cloud_rate_usd_per_core_hour: f64,
}

fn default_cloud_rate() -> f64 {
    0.04
}

/// The `[workers]` section: fleet sizing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
    pub min_workers: Option<u32>,
    pub max_workers: Option<u32>,
    pub recommended_workers: Option<u32>,
}

// ── TOML Parsing ────────────────────────────────────────────────

/// Parse a project configuration from a TOML string.
pub fn parse_toml(content: &str) -> Result<ProjectConfig> {
    let config: ProjectConfig = toml::from_str(content)?;
    validate_config(&config)?;
    Ok(config)
}

/// Parse a project configuration from a TOML file path.
pub fn parse_toml_file(path: &std::path::Path) -> Result<ProjectConfig> {
    let content = std::fs::read_to_string(path)?;
    parse_toml(&content)
}

/// Validate a project configuration for logical consistency.
fn validate_config(config: &ProjectConfig) -> Result<()> {
    if config.project.name.is_empty() {
        anyhow::bail!("project.name is required");
    }
    if config.project.form.is_empty() {
        anyhow::bail!("project.form is required");
    }

    // Validate form name
    let valid_forms = [
        "factorial",
        "primorial",
        "kbn",
        "palindromic",
        "near_repdigit",
        "cullen_woodall",
        "wagstaff",
        "carol_kynea",
        "twin",
        "sophie_germain",
        "repunit",
        "gen_fermat",
    ];
    if !valid_forms.contains(&config.project.form.as_str()) {
        anyhow::bail!(
            "Unknown form '{}'. Valid forms: {}",
            config.project.form,
            valid_forms.join(", ")
        );
    }

    // For record objective, target_digits is expected
    if config.project.objective == Objective::Record && config.target.target_digits.is_none() {
        warn!("record objective without target_digits — will use world record as target");
    }

    // For survey objective, range is expected
    if config.project.objective == Objective::Survey
        && config.target.range_start.is_none()
        && config.strategy.phases.is_empty()
        && !config.strategy.auto_strategy
    {
        anyhow::bail!("Survey objective requires target.range_start/range_end or strategy.phases");
    }

    // Validate phase dependency graph (skip for auto_strategy — phases generated later)
    if !config.strategy.phases.is_empty() {
        validate_phase_graph(&config.strategy.phases)?;
    }

    Ok(())
}

/// Validate the phase dependency graph: all `depends_on` references must
/// point to phases in the same project, and the graph must be acyclic
/// (no circular dependencies).
pub(crate) fn validate_phase_graph(phases: &[PhaseConfig]) -> Result<()> {
    use std::collections::{HashMap, HashSet};

    let names: HashSet<&str> = phases.iter().map(|p| p.name.as_str()).collect();

    // Check all depends_on references exist
    for phase in phases {
        for dep in phase.depends_on.as_deref().unwrap_or_default() {
            if !names.contains(dep.as_str()) {
                anyhow::bail!(
                    "Phase '{}' depends on unknown phase '{}'. Known phases: {}",
                    phase.name,
                    dep,
                    names.into_iter().collect::<Vec<_>>().join(", ")
                );
            }
        }
    }

    // Detect circular dependencies via topological sort (Kahn's algorithm)
    let mut in_degree: HashMap<&str, usize> = HashMap::new();
    let mut successors: HashMap<&str, Vec<&str>> = HashMap::new();
    for phase in phases {
        in_degree.entry(phase.name.as_str()).or_insert(0);
        successors.entry(phase.name.as_str()).or_default();
        for dep in phase.depends_on.as_deref().unwrap_or_default() {
            *in_degree.entry(phase.name.as_str()).or_insert(0) += 1;
            successors
                .entry(dep.as_str())
                .or_default()
                .push(phase.name.as_str());
        }
    }

    let mut queue: Vec<&str> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(&name, _)| name)
        .collect();
    let mut visited = 0usize;

    while let Some(node) = queue.pop() {
        visited += 1;
        for &succ in successors.get(node).unwrap_or(&vec![]) {
            let deg = in_degree.get_mut(succ).unwrap();
            *deg -= 1;
            if *deg == 0 {
                queue.push(succ);
            }
        }
    }

    if visited != phases.len() {
        anyhow::bail!(
            "Circular dependency detected among phases. {} phases could not be ordered.",
            phases.len() - visited
        );
    }

    Ok(())
}

/// Generate a URL-safe slug from a project name.
pub fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── TOML parsing roundtrip ──────────────────────────────────

    #[test]
    fn toml_roundtrip_minimal() {
        let toml_str = r#"
[project]
name = "roundtrip-test"
objective = "survey"
form = "factorial"

[target]
range_start = 1
range_end = 1000

[[strategy.phases]]
name = "sweep"
search_params = { search_type = "factorial", start = 1, end = 1000 }
"#;
        let config = parse_toml(toml_str).unwrap();
        let serialized = toml::to_string(&config).unwrap();
        let reparsed: ProjectConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(reparsed.project.name, config.project.name);
        assert_eq!(reparsed.project.form, config.project.form);
        assert_eq!(reparsed.project.objective, config.project.objective);
        assert_eq!(reparsed.target.range_start, config.target.range_start);
        assert_eq!(reparsed.target.range_end, config.target.range_end);
        assert_eq!(reparsed.strategy.phases.len(), config.strategy.phases.len());
    }

    #[test]
    fn toml_roundtrip_with_all_sections() {
        let toml_str = r#"
[project]
name = "full-roundtrip"
description = "A comprehensive test"
objective = "record"
form = "kbn"
author = "tester"
tags = ["kbn", "record"]

[target]
target_digits = 100000
range_start = 100
range_end = 50000

[competitive]
current_record_expression = "3*2^100000-1"
current_record_digits = 30103

[strategy]
auto_strategy = false

[[strategy.phases]]
name = "sweep"
description = "First pass"
search_params = { search_type = "kbn", k = 3, base = 2, min_n = 100, max_n = 50000 }
block_size = 10000
completion = "all_blocks_done"

[infrastructure]
min_ram_gb = 8
min_cores = 4
recommended_cores = 16
preferred_tools = ["pfgw"]

[budget]
max_cost_usd = 100.0
cost_alert_threshold_usd = 50.0
cloud_rate_usd_per_core_hour = 0.05

[workers]
min_workers = 1
max_workers = 4
recommended_workers = 2
"#;
        let config = parse_toml(toml_str).unwrap();
        let serialized = toml::to_string(&config).unwrap();
        let reparsed: ProjectConfig = toml::from_str(&serialized).unwrap();

        assert_eq!(reparsed.project.name, "full-roundtrip");
        assert_eq!(reparsed.project.author, "tester");
        assert_eq!(reparsed.project.tags, vec!["kbn", "record"]);
        assert_eq!(reparsed.target.target_digits, Some(100000));
        assert!(reparsed.competitive.is_some());
        assert_eq!(reparsed.competitive.as_ref().unwrap().current_record_digits, Some(30103));
        assert!(reparsed.infrastructure.is_some());
        assert_eq!(reparsed.infrastructure.as_ref().unwrap().min_ram_gb, Some(8));
        assert!(reparsed.budget.is_some());
        assert_eq!(reparsed.budget.as_ref().unwrap().max_cost_usd, Some(100.0));
        assert!(reparsed.workers.is_some());
        assert_eq!(reparsed.workers.as_ref().unwrap().recommended_workers, Some(2));
    }

    // ── Slug generation ─────────────────────────────────────────

    #[test]
    fn slugify_simple_name() {
        assert_eq!(slugify("my-project"), "my-project");
    }

    #[test]
    fn slugify_mixed_case() {
        assert_eq!(slugify("MyProject"), "myproject");
    }

    #[test]
    fn slugify_spaces_to_hyphens() {
        assert_eq!(slugify("my project"), "my-project");
    }

    #[test]
    fn slugify_special_characters() {
        assert_eq!(slugify("test!@#$%^&*()"), "test");
    }

    #[test]
    fn slugify_multiple_spaces() {
        assert_eq!(slugify("hello   world"), "hello-world");
    }

    #[test]
    fn slugify_leading_trailing_spaces() {
        assert_eq!(slugify("  hello world  "), "hello-world");
    }

    #[test]
    fn slugify_numbers_preserved() {
        assert_eq!(slugify("project-2026"), "project-2026");
    }

    #[test]
    fn slugify_parentheses_and_equals() {
        assert_eq!(slugify("factorial survey (n=1..1000)"), "factorial-survey-n-1-1000");
    }

    #[test]
    fn slugify_already_valid() {
        assert_eq!(slugify("wagstaff-record-2026"), "wagstaff-record-2026");
    }

    #[test]
    fn slugify_empty_string() {
        assert_eq!(slugify(""), "");
    }

    #[test]
    fn slugify_only_special_chars() {
        assert_eq!(slugify("!@#$%"), "");
    }

    // ── Objective enum parsing ──────────────────────────────────

    #[test]
    fn objective_record_from_toml() {
        let toml_str = r#"
[project]
name = "test"
objective = "record"
form = "factorial"

[target]
target_digits = 1000000
"#;
        let config = parse_toml(toml_str).unwrap();
        assert_eq!(config.project.objective, Objective::Record);
    }

    #[test]
    fn objective_survey_from_toml() {
        let toml_str = r#"
[project]
name = "test"
objective = "survey"
form = "factorial"

[target]
range_start = 1
range_end = 100

[[strategy.phases]]
name = "s"
search_params = { start = 1, end = 100 }
"#;
        let config = parse_toml(toml_str).unwrap();
        assert_eq!(config.project.objective, Objective::Survey);
    }

    #[test]
    fn objective_verification_from_toml() {
        let toml_str = r#"
[project]
name = "test"
objective = "verification"
form = "kbn"
"#;
        let config = parse_toml(toml_str).unwrap();
        assert_eq!(config.project.objective, Objective::Verification);
    }

    #[test]
    fn objective_custom_from_toml() {
        let toml_str = r#"
[project]
name = "test"
objective = "custom"
form = "kbn"
"#;
        let config = parse_toml(toml_str).unwrap();
        assert_eq!(config.project.objective, Objective::Custom);
    }

    #[test]
    fn objective_display() {
        assert_eq!(Objective::Record.to_string(), "record");
        assert_eq!(Objective::Survey.to_string(), "survey");
        assert_eq!(Objective::Verification.to_string(), "verification");
        assert_eq!(Objective::Custom.to_string(), "custom");
    }

    #[test]
    fn objective_invalid_rejected() {
        let toml_str = r#"
[project]
name = "test"
objective = "invalid_objective"
form = "factorial"
"#;
        assert!(parse_toml(toml_str).is_err());
    }

    // ── Validation ──────────────────────────────────────────────

    #[test]
    fn validate_empty_name_rejected() {
        let toml_str = r#"
[project]
name = ""
objective = "survey"
form = "factorial"

[target]
range_start = 1
range_end = 100

[[strategy.phases]]
name = "s"
search_params = {}
"#;
        let err = parse_toml(toml_str).unwrap_err();
        assert!(err.to_string().contains("name"), "Expected name validation error, got: {}", err);
    }

    #[test]
    fn validate_empty_form_rejected() {
        let toml_str = r#"
[project]
name = "test"
objective = "survey"
form = ""

[target]
range_start = 1
range_end = 100
"#;
        let err = parse_toml(toml_str).unwrap_err();
        assert!(err.to_string().contains("form"), "Expected form validation error, got: {}", err);
    }

    #[test]
    fn validate_invalid_form_rejected() {
        let toml_str = r#"
[project]
name = "test"
objective = "survey"
form = "nonexistent_form"
"#;
        let err = parse_toml(toml_str).unwrap_err();
        assert!(err.to_string().contains("Unknown form"));
    }

    #[test]
    fn validate_all_valid_forms_accepted() {
        let valid_forms = [
            "factorial", "primorial", "kbn", "palindromic", "near_repdigit",
            "cullen_woodall", "wagstaff", "carol_kynea", "twin",
            "sophie_germain", "repunit", "gen_fermat",
        ];
        for form in &valid_forms {
            let toml_str = format!(
                r#"
[project]
name = "test-{}"
objective = "custom"
form = "{}"
"#,
                form, form
            );
            let result = parse_toml(&toml_str);
            assert!(result.is_ok(), "Form '{}' should be valid, got: {:?}", form, result.err());
        }
    }

    #[test]
    fn validate_survey_without_range_or_phases_rejected() {
        let toml_str = r#"
[project]
name = "test"
objective = "survey"
form = "factorial"
"#;
        let err = parse_toml(toml_str).unwrap_err();
        assert!(err.to_string().contains("Survey"));
    }

    #[test]
    fn validate_survey_with_auto_strategy_accepted() {
        let toml_str = r#"
[project]
name = "test"
objective = "survey"
form = "factorial"

[strategy]
auto_strategy = true
"#;
        // auto_strategy bypasses the range/phases requirement
        assert!(parse_toml(toml_str).is_ok());
    }

    #[test]
    fn validate_survey_with_range_accepted() {
        let toml_str = r#"
[project]
name = "test"
objective = "survey"
form = "factorial"

[target]
range_start = 1
range_end = 100

[[strategy.phases]]
name = "sweep"
search_params = { start = 1, end = 100 }
"#;
        assert!(parse_toml(toml_str).is_ok());
    }

    #[test]
    fn validate_circular_dependency_rejected() {
        let toml_str = r#"
[project]
name = "test"
objective = "custom"
form = "factorial"

[[strategy.phases]]
name = "a"
search_params = {}
depends_on = ["b"]

[[strategy.phases]]
name = "b"
search_params = {}
depends_on = ["a"]
"#;
        let err = parse_toml(toml_str).unwrap_err();
        assert!(err.to_string().contains("Circular dependency"));
    }

    #[test]
    fn validate_unknown_dependency_rejected() {
        let toml_str = r#"
[project]
name = "test"
objective = "custom"
form = "factorial"

[[strategy.phases]]
name = "sweep"
search_params = {}
depends_on = ["nonexistent"]
"#;
        let err = parse_toml(toml_str).unwrap_err();
        assert!(err.to_string().contains("unknown phase"));
    }

    #[test]
    fn validate_valid_dependency_chain() {
        let phases = vec![
            PhaseConfig {
                name: "a".into(),
                description: String::new(),
                search_params: serde_json::json!({}),
                block_size: None,
                depends_on: None,
                activation_condition: None,
                completion: "all_blocks_done".into(),
            },
            PhaseConfig {
                name: "b".into(),
                description: String::new(),
                search_params: serde_json::json!({}),
                block_size: None,
                depends_on: Some(vec!["a".into()]),
                activation_condition: None,
                completion: "all_blocks_done".into(),
            },
            PhaseConfig {
                name: "c".into(),
                description: String::new(),
                search_params: serde_json::json!({}),
                block_size: None,
                depends_on: Some(vec!["a".into(), "b".into()]),
                activation_condition: None,
                completion: "all_blocks_done".into(),
            },
        ];
        assert!(validate_phase_graph(&phases).is_ok());
    }

    #[test]
    fn validate_three_node_cycle_rejected() {
        let phases = vec![
            PhaseConfig {
                name: "a".into(),
                description: String::new(),
                search_params: serde_json::json!({}),
                block_size: None,
                depends_on: Some(vec!["c".into()]),
                activation_condition: None,
                completion: "all_blocks_done".into(),
            },
            PhaseConfig {
                name: "b".into(),
                description: String::new(),
                search_params: serde_json::json!({}),
                block_size: None,
                depends_on: Some(vec!["a".into()]),
                activation_condition: None,
                completion: "all_blocks_done".into(),
            },
            PhaseConfig {
                name: "c".into(),
                description: String::new(),
                search_params: serde_json::json!({}),
                block_size: None,
                depends_on: Some(vec!["b".into()]),
                activation_condition: None,
                completion: "all_blocks_done".into(),
            },
        ];
        let err = validate_phase_graph(&phases).unwrap_err();
        assert!(err.to_string().contains("Circular dependency"));
    }

    // ── Default values ──────────────────────────────────────────

    #[test]
    fn default_completion_is_all_blocks_done() {
        let toml_str = r#"
[project]
name = "test"
objective = "custom"
form = "factorial"

[[strategy.phases]]
name = "sweep"
search_params = {}
"#;
        let config = parse_toml(toml_str).unwrap();
        assert_eq!(config.strategy.phases[0].completion, "all_blocks_done");
    }

    #[test]
    fn default_cloud_rate_is_004() {
        let toml_str = r#"
[project]
name = "test"
objective = "custom"
form = "factorial"

[budget]
"#;
        let config = parse_toml(toml_str).unwrap();
        assert!((config.budget.unwrap().cloud_rate_usd_per_core_hour - 0.04).abs() < f64::EPSILON);
    }

    #[test]
    fn missing_optional_sections_are_none() {
        let toml_str = r#"
[project]
name = "test"
objective = "custom"
form = "factorial"
"#;
        let config = parse_toml(toml_str).unwrap();
        assert!(config.competitive.is_none());
        assert!(config.infrastructure.is_none());
        assert!(config.budget.is_none());
        assert!(config.workers.is_none());
    }

    #[test]
    fn empty_tags_default() {
        let toml_str = r#"
[project]
name = "test"
objective = "custom"
form = "factorial"
"#;
        let config = parse_toml(toml_str).unwrap();
        assert!(config.project.tags.is_empty());
    }

    #[test]
    fn default_description_is_empty() {
        let toml_str = r#"
[project]
name = "test"
objective = "custom"
form = "factorial"
"#;
        let config = parse_toml(toml_str).unwrap();
        assert!(config.project.description.is_empty());
    }
}
