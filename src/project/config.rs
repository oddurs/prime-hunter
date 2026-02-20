//! TOML configuration structs, parsing, and validation.
//!
//! A project TOML defines the full campaign: identity, targets, competitive
//! context, strategy (manual phases or auto-generated), infrastructure
//! requirements, budget limits, and fleet sizing.

use anyhow::Result;
use serde::{Deserialize, Serialize};

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
        eprintln!(
            "Warning: record objective without target_digits — will use world record as target"
        );
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
