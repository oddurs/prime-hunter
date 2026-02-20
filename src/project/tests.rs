//! Unit tests for project configuration, cost estimation, orchestration,
//! and records parsing.

use super::*;

#[test]
fn parse_minimal_toml() {
    let toml = r#"
[project]
name = "test-project"
objective = "survey"
form = "factorial"

[target]
range_start = 1
range_end = 1000

[[strategy.phases]]
name = "sweep"
search_params = { search_type = "factorial", start = 1, end = 1000 }
"#;
    let config = parse_toml(toml).unwrap();
    assert_eq!(config.project.name, "test-project");
    assert_eq!(config.project.objective, Objective::Survey);
    assert_eq!(config.project.form, "factorial");
    assert_eq!(config.strategy.phases.len(), 1);
    assert_eq!(config.strategy.phases[0].name, "sweep");
}

#[test]
fn parse_full_toml() {
    let toml = r#"
[project]
name = "wagstaff-record-2026"
description = "Hunt for new Wagstaff primes"
objective = "record"
form = "wagstaff"
author = "oddurs"
tags = ["wagstaff", "record"]

[target]
target_digits = 5000000

[competitive]
current_record_expression = "(2^13347311+1)/3"
current_record_digits = 4017941
current_record_holder = "Ryan Propper"
oeis_sequence = "A000978"

[strategy]
auto_strategy = false

[[strategy.phases]]
name = "sweep"
description = "Sieve exponents 15M..20M"
search_params = { search_type = "wagstaff", min_exp = 15135397, max_exp = 20000000 }
block_size = 1000
completion = "all_blocks_done"

[[strategy.phases]]
name = "extend"
description = "Extend to 25M if no discovery"
search_params = { search_type = "wagstaff", min_exp = 20000001, max_exp = 25000000 }
block_size = 1000
depends_on = ["sweep"]
activation_condition = "previous_phase_found_zero"

[infrastructure]
min_ram_gb = 8
min_cores = 4
recommended_cores = 16
preferred_tools = ["gwnum"]

[budget]
max_cost_usd = 500.0
cost_alert_threshold_usd = 100.0
cloud_rate_usd_per_core_hour = 0.04

[workers]
min_workers = 1
max_workers = 8
recommended_workers = 4
"#;
    let config = parse_toml(toml).unwrap();
    assert_eq!(config.project.name, "wagstaff-record-2026");
    assert_eq!(config.project.objective, Objective::Record);
    assert_eq!(config.strategy.phases.len(), 2);
    assert_eq!(
        config.strategy.phases[1].depends_on,
        Some(vec!["sweep".to_string()])
    );
    assert_eq!(
        config.strategy.phases[1].activation_condition,
        Some("previous_phase_found_zero".to_string())
    );
    assert!(config.budget.is_some());
    assert_eq!(config.budget.as_ref().unwrap().max_cost_usd, Some(500.0));
}

#[test]
fn parse_invalid_form_rejected() {
    let toml = r#"
[project]
name = "test"
objective = "survey"
form = "invalid_form"

[target]
range_start = 1
range_end = 100

[[strategy.phases]]
name = "x"
search_params = { search_type = "invalid_form", start = 1, end = 100 }
"#;
    assert!(parse_toml(toml).is_err());
}

#[test]
fn slugify_names() {
    assert_eq!(slugify("wagstaff-record-2026"), "wagstaff-record-2026");
    assert_eq!(slugify("My Test Project!"), "my-test-project");
    assert_eq!(slugify("  hello   world  "), "hello-world");
    assert_eq!(
        slugify("factorial survey (n=1..1000)"),
        "factorial-survey-n-1-1000"
    );
}

#[test]
fn cost_estimate_produces_nonzero() {
    let config = parse_toml(
        r#"
[project]
name = "test"
objective = "survey"
form = "factorial"

[target]
range_start = 1
range_end = 1000

[[strategy.phases]]
name = "sweep"
search_params = { search_type = "factorial", start = 1, end = 1000 }
"#,
    )
    .unwrap();

    let est = estimate_project_cost(&config);
    assert!(est.estimated_candidates > 0);
    assert!(est.total_core_hours >= 0.0);
    assert!(est.total_cost_usd >= 0.0);
    assert!(est.workers_recommended > 0);
}

#[test]
fn auto_strategy_factorial_record() {
    let config = parse_toml(
        r#"
[project]
name = "test"
objective = "record"
form = "factorial"

[strategy]
auto_strategy = true

[target]
range_start = 500
range_end = 1500
"#,
    )
    .unwrap();
    let phases = generate_auto_strategy(&config);
    assert!(!phases.is_empty());
    assert_eq!(phases[0].name, "sweep");
}

#[test]
fn auto_strategy_wagstaff_record_two_phases() {
    let config = parse_toml(
        r#"
[project]
name = "test"
objective = "record"
form = "wagstaff"

[strategy]
auto_strategy = true

[target]
range_start = 14000000
range_end = 20000000
"#,
    )
    .unwrap();
    let phases = generate_auto_strategy(&config);
    assert_eq!(phases.len(), 2);
    assert_eq!(phases[0].name, "sweep");
    assert_eq!(phases[1].name, "extend");
    assert_eq!(
        phases[1].activation_condition,
        Some("previous_phase_found_zero".to_string())
    );
}

#[test]
fn should_activate_no_deps() {
    let phase = ProjectPhaseRow {
        id: 1,
        project_id: 1,
        name: "sweep".into(),
        description: String::new(),
        phase_order: 0,
        status: "pending".into(),
        search_params: serde_json::json!({}),
        block_size: 1000,
        depends_on: vec![],
        activation_condition: None,
        completion_condition: "all_blocks_done".into(),
        search_job_id: None,
        total_tested: 0,
        total_found: 0,
        started_at: None,
        completed_at: None,
    };
    assert!(should_activate(&phase, &[phase.clone()]));
}

#[test]
fn should_activate_unmet_dep() {
    let sweep = ProjectPhaseRow {
        id: 1,
        project_id: 1,
        name: "sweep".into(),
        description: String::new(),
        phase_order: 0,
        status: "active".into(),
        search_params: serde_json::json!({}),
        block_size: 1000,
        depends_on: vec![],
        activation_condition: None,
        completion_condition: "all_blocks_done".into(),
        search_job_id: None,
        total_tested: 0,
        total_found: 0,
        started_at: None,
        completed_at: None,
    };
    let extend = ProjectPhaseRow {
        id: 2,
        project_id: 1,
        name: "extend".into(),
        description: String::new(),
        phase_order: 1,
        status: "pending".into(),
        search_params: serde_json::json!({}),
        block_size: 1000,
        depends_on: vec!["sweep".into()],
        activation_condition: None,
        completion_condition: "all_blocks_done".into(),
        search_job_id: None,
        total_tested: 0,
        total_found: 0,
        started_at: None,
        completed_at: None,
    };
    assert!(!should_activate(&extend, &[sweep, extend.clone()]));
}

#[test]
fn should_activate_met_dep_with_condition() {
    let sweep = ProjectPhaseRow {
        id: 1,
        project_id: 1,
        name: "sweep".into(),
        description: String::new(),
        phase_order: 0,
        status: "completed".into(),
        search_params: serde_json::json!({}),
        block_size: 1000,
        depends_on: vec![],
        activation_condition: None,
        completion_condition: "all_blocks_done".into(),
        search_job_id: None,
        total_tested: 1000,
        total_found: 0, // found nothing
        started_at: None,
        completed_at: None,
    };
    let extend = ProjectPhaseRow {
        id: 2,
        project_id: 1,
        name: "extend".into(),
        description: String::new(),
        phase_order: 1,
        status: "pending".into(),
        search_params: serde_json::json!({}),
        block_size: 1000,
        depends_on: vec!["sweep".into()],
        activation_condition: Some("previous_phase_found_zero".into()),
        completion_condition: "all_blocks_done".into(),
        search_job_id: None,
        total_tested: 0,
        total_found: 0,
        started_at: None,
        completed_at: None,
    };
    assert!(should_activate(&extend, &[sweep, extend.clone()]));
}

#[test]
fn phase_complete_all_blocks_done() {
    let summary = crate::db::JobBlockSummary {
        available: 0,
        claimed: 0,
        completed: 10,
        failed: 0,
        total_tested: 10000,
        total_found: 2,
    };
    assert!(is_phase_complete("all_blocks_done", &summary));
}

#[test]
fn phase_complete_first_prime_found() {
    let summary = crate::db::JobBlockSummary {
        available: 5,
        claimed: 2,
        completed: 3,
        failed: 0,
        total_tested: 3000,
        total_found: 1,
    };
    assert!(is_phase_complete("first_prime_found", &summary));
}

#[test]
fn t5k_html_parsing() {
    // Minimal mock of t5k.org table structure
    let html = r#"
<html><body>
<table class="list">
<tr><th>rank</th><th>prime</th><th>digits</th><th>who</th><th>when</th></tr>
<tr><td>1</td><td>208003! - 1</td><td>1,015,843</td><td>Fujii</td><td>2023</td></tr>
<tr><td>2</td><td>150209! + 1</td><td>712,355</td><td>Kuosa</td><td>2021</td></tr>
</table>
</body></html>"#;
    let record = parse_t5k_html(html, "factorial").unwrap().unwrap();
    assert_eq!(record.expression, "208003! - 1");
    assert_eq!(record.digits, 1015843);
    assert_eq!(record.holder, "Fujii");
}

#[test]
fn validate_phase_graph_valid() {
    let phases = vec![
        PhaseConfig {
            name: "sweep".into(),
            description: "First phase".into(),
            search_params: serde_json::json!({"search_type": "factorial", "start": 1, "end": 1000}),
            block_size: Some(100),
            depends_on: None,
            activation_condition: None,
            completion: "all_blocks_done".into(),
        },
        PhaseConfig {
            name: "extend".into(),
            description: "Second phase".into(),
            search_params: serde_json::json!({"search_type": "factorial", "start": 1000, "end": 2000}),
            block_size: Some(100),
            depends_on: Some(vec!["sweep".into()]),
            activation_condition: Some("previous_phase_found_zero".into()),
            completion: "all_blocks_done".into(),
        },
    ];
    assert!(validate_phase_graph(&phases).is_ok());
}

#[test]
fn validate_phase_graph_unknown_dep() {
    let phases = vec![PhaseConfig {
        name: "sweep".into(),
        description: String::new(),
        search_params: serde_json::json!({"search_type": "factorial"}),
        block_size: None,
        depends_on: Some(vec!["nonexistent".into()]),
        activation_condition: None,
        completion: "all_blocks_done".into(),
    }];
    let err = validate_phase_graph(&phases).unwrap_err();
    assert!(
        err.to_string().contains("unknown phase"),
        "Expected unknown phase error, got: {}",
        err
    );
}

#[test]
fn validate_phase_graph_circular() {
    let phases = vec![
        PhaseConfig {
            name: "a".into(),
            description: String::new(),
            search_params: serde_json::json!({}),
            block_size: None,
            depends_on: Some(vec!["b".into()]),
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
    ];
    let err = validate_phase_graph(&phases).unwrap_err();
    assert!(
        err.to_string().contains("Circular dependency"),
        "Expected circular dep error, got: {}",
        err
    );
}

#[test]
fn template_files_parse() {
    let templates_dir = std::path::Path::new("projects/templates");
    if !templates_dir.exists() {
        return; // skip if templates not yet created
    }
    for entry in std::fs::read_dir(templates_dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().map_or(false, |e| e == "toml") {
            let content = std::fs::read_to_string(&path).unwrap();
            let result = parse_toml(&content);
            assert!(
                result.is_ok(),
                "Template {:?} failed to parse: {:?}",
                path.file_name().unwrap(),
                result.err()
            );
            let config = result.unwrap();
            assert!(!config.project.name.is_empty());
            assert!(!config.project.form.is_empty());
        }
    }
}

#[test]
fn extract_range_from_params_variants() {
    let params = serde_json::json!({"start": 100, "end": 500});
    assert_eq!(extract_range_from_params(&params), (100, 500));

    let params = serde_json::json!({"min_n": 1000, "max_n": 5000});
    assert_eq!(extract_range_from_params(&params), (1000, 5000));

    let params = serde_json::json!({"min_exp": 14000000, "max_exp": 20000000});
    assert_eq!(extract_range_from_params(&params), (14000000, 20000000));
}

// ── Fleet requirement tests ───────────────────────────────────

fn make_project_row(infra: serde_json::Value) -> crate::project::ProjectRow {
    crate::project::ProjectRow {
        id: 1,
        slug: "test".into(),
        name: "Test".into(),
        description: String::new(),
        objective: "survey".into(),
        form: "factorial".into(),
        status: "active".into(),
        toml_source: None,
        target: serde_json::json!({}),
        competitive: serde_json::json!(null),
        strategy: serde_json::json!({}),
        infrastructure: infra,
        budget: serde_json::json!({}),
        total_tested: 0,
        total_found: 0,
        best_prime_id: None,
        best_digits: 0,
        total_core_hours: 0.0,
        total_cost_usd: 0.0,
        created_at: chrono::Utc::now(),
        started_at: None,
        completed_at: None,
        updated_at: chrono::Utc::now(),
    }
}

fn make_fleet(workers: u32, cores: u32, ram_gb: u32) -> crate::db::FleetSummary {
    crate::db::FleetSummary {
        worker_count: workers,
        total_cores: cores,
        max_ram_gb: ram_gb,
        active_search_types: vec!["factorial".into()],
    }
}

#[test]
fn fleet_check_no_requirements_passes() {
    let project = make_project_row(serde_json::json!(null));
    let fleet = make_fleet(2, 16, 32);
    assert!(check_fleet_requirements(&project, &fleet).is_none());
}

#[test]
fn fleet_check_min_cores_met() {
    let project = make_project_row(serde_json::json!({"min_cores": 8}));
    let fleet = make_fleet(2, 16, 32);
    assert!(check_fleet_requirements(&project, &fleet).is_none());
}

#[test]
fn fleet_check_min_cores_unmet() {
    let project = make_project_row(serde_json::json!({"min_cores": 32}));
    let fleet = make_fleet(2, 16, 32);
    let reason = check_fleet_requirements(&project, &fleet);
    assert!(reason.is_some());
    assert!(reason.unwrap().contains("cores"));
}

#[test]
fn fleet_check_min_ram_unmet() {
    let project = make_project_row(serde_json::json!({"min_ram_gb": 64}));
    let fleet = make_fleet(2, 16, 32);
    let reason = check_fleet_requirements(&project, &fleet);
    assert!(reason.is_some());
    assert!(reason.unwrap().contains("RAM"));
}

#[test]
fn fleet_check_min_workers_unmet() {
    let project = make_project_row(serde_json::json!({"min_workers": 4}));
    let fleet = make_fleet(2, 16, 32);
    let reason = check_fleet_requirements(&project, &fleet);
    assert!(reason.is_some());
    assert!(reason.unwrap().contains("workers"));
}

#[test]
fn fleet_check_required_tools_unmet() {
    let project = make_project_row(serde_json::json!({"required_tools": ["gwnum"]}));
    let fleet = make_fleet(2, 16, 32);
    let reason = check_fleet_requirements(&project, &fleet);
    assert!(reason.is_some());
    assert!(reason.unwrap().contains("gwnum"));
}

#[test]
fn fleet_check_all_requirements_met() {
    let project = make_project_row(serde_json::json!({
        "min_cores": 8,
        "min_ram_gb": 16,
        "min_workers": 2,
        "required_tools": ["factorial"]
    }));
    let fleet = make_fleet(2, 16, 32);
    assert!(check_fleet_requirements(&project, &fleet).is_none());
}

#[test]
fn fleet_check_empty_fleet_fails_with_requirements() {
    let project = make_project_row(serde_json::json!({"min_workers": 1}));
    let fleet = make_fleet(0, 0, 0);
    assert!(check_fleet_requirements(&project, &fleet).is_some());
}

// ── Adaptive phase generation tests ───────────────────────────

fn make_completed_phase(name: &str, found: i64, params: serde_json::Value) -> ProjectPhaseRow {
    ProjectPhaseRow {
        id: 1,
        project_id: 1,
        name: name.into(),
        description: String::new(),
        phase_order: 0,
        status: "completed".into(),
        search_params: params,
        block_size: 1000,
        depends_on: vec![],
        activation_condition: None,
        completion_condition: "all_blocks_done".into(),
        search_job_id: Some(1),
        total_tested: 1000,
        total_found: found,
        started_at: None,
        completed_at: None,
    }
}

#[test]
fn followup_generated_when_no_primes_found() {
    let project = make_project_row(serde_json::json!(null));
    let phase = make_completed_phase(
        "sweep",
        0,
        serde_json::json!({"search_type": "factorial", "start": 1000, "end": 2000}),
    );
    let result = generate_followup_phase(&project, &phase, &[phase.clone()]);
    assert!(result.is_some());
    let followup = result.unwrap();
    assert_eq!(followup.name, "sweep-extend");
    // New range should extend by same span (1000)
    let (start, end) = extract_range_from_params(&followup.search_params);
    assert_eq!(start, 2001);
    assert_eq!(end, 3001);
    assert_eq!(followup.depends_on, Some(vec!["sweep".to_string()]));
    assert_eq!(
        followup.activation_condition,
        Some("previous_phase_found_zero".to_string())
    );
}

#[test]
fn no_followup_when_primes_found() {
    let project = make_project_row(serde_json::json!(null));
    let phase = make_completed_phase(
        "sweep",
        3,
        serde_json::json!({"search_type": "factorial", "start": 1000, "end": 2000}),
    );
    assert!(generate_followup_phase(&project, &phase, &[phase.clone()]).is_none());
}

#[test]
fn no_followup_if_already_exists() {
    let project = make_project_row(serde_json::json!(null));
    let phase = make_completed_phase(
        "sweep",
        0,
        serde_json::json!({"search_type": "factorial", "start": 1000, "end": 2000}),
    );
    let existing_extend = make_completed_phase(
        "sweep-extend",
        0,
        serde_json::json!({"search_type": "factorial", "start": 2001, "end": 3001}),
    );
    let all = vec![phase.clone(), existing_extend];
    assert!(generate_followup_phase(&project, &phase, &all).is_none());
}

#[test]
fn no_followup_for_extension_phase() {
    let project = make_project_row(serde_json::json!(null));
    let phase = make_completed_phase(
        "sweep-extend",
        0,
        serde_json::json!({"search_type": "factorial", "start": 2001, "end": 3001}),
    );
    assert!(generate_followup_phase(&project, &phase, &[phase.clone()]).is_none());
}

#[test]
fn followup_uses_kbn_range_keys() {
    let project = make_project_row(serde_json::json!(null));
    let phase = make_completed_phase(
        "sweep",
        0,
        serde_json::json!({"search_type": "kbn", "k": 1, "base": 2, "min_n": 100000, "max_n": 200000}),
    );
    let followup = generate_followup_phase(&project, &phase, &[phase.clone()]).unwrap();
    let (start, end) = extract_range_from_params(&followup.search_params);
    assert_eq!(start, 200001);
    assert_eq!(end, 300001);
    // Verify k and base are preserved
    assert_eq!(followup.search_params.get("k").unwrap().as_u64(), Some(1));
    assert_eq!(
        followup.search_params.get("base").unwrap().as_u64(),
        Some(2)
    );
}

#[test]
fn followup_preserves_block_size() {
    let project = make_project_row(serde_json::json!(null));
    let mut phase = make_completed_phase(
        "sweep",
        0,
        serde_json::json!({"search_type": "wagstaff", "min_exp": 14000000, "max_exp": 17000000}),
    );
    phase.block_size = 5000;
    let followup = generate_followup_phase(&project, &phase, &[phase.clone()]).unwrap();
    assert_eq!(followup.block_size, Some(5000));
}
