//! Empirical cost estimation model for prime-hunting projects.
//!
//! Uses a per-form power-law timing model to estimate computation cost from
//! candidate count and digit size. Cloud pricing defaults to Hetzner AX42
//! ($0.04/core-hr). PFGW gives ~50x speedup, GWNUM ~100x.

use super::config::ProjectConfig;
use serde::Serialize;

/// Estimated resource usage and cost for a project or phase.
#[derive(Debug, Clone, Serialize)]
pub struct CostEstimate {
    pub estimated_candidates: u64,
    pub estimated_test_time_secs: f64,
    pub total_core_hours: f64,
    pub total_cost_usd: f64,
    pub estimated_duration_hours: f64,
    pub workers_recommended: u32,
}

/// Empirical seconds per candidate by form and digit count.
/// Power-law model: base_secs * (digits / 1000)^exponent.
/// Values calibrated against GIMPS and darkreach benchmarks.
pub fn secs_per_candidate(form: &str, digits: u64, has_pfgw: bool) -> f64 {
    let d = (digits as f64) / 1000.0;
    let base = match form {
        "factorial" | "primorial" => 0.5 * d.powf(2.5),
        "kbn" | "twin" | "sophie_germain" => 0.1 * d.powf(2.0),
        "cullen_woodall" | "carol_kynea" => 0.2 * d.powf(2.2),
        "wagstaff" => 0.8 * d.powf(2.5),
        "palindromic" | "near_repdigit" => 0.3 * d.powf(2.0),
        "repunit" => 0.4 * d.powf(2.3),
        "gen_fermat" => 0.3 * d.powf(2.2),
        _ => 0.5 * d.powf(2.5),
    };

    // PFGW/GWNUM accelerated forms are ~50x faster for large candidates
    if has_pfgw && digits >= 10_000 {
        base / 50.0
    } else {
        base
    }
}

/// Estimate project cost from configuration.
pub fn estimate_project_cost(config: &ProjectConfig) -> CostEstimate {
    let cloud_rate = config
        .budget
        .as_ref()
        .map(|b| b.cloud_rate_usd_per_core_hour)
        .unwrap_or(0.04);
    let has_pfgw = config
        .infrastructure
        .as_ref()
        .map(|i| {
            i.preferred_tools.contains(&"pfgw".to_string())
                || i.preferred_tools.contains(&"gwnum".to_string())
        })
        .unwrap_or(false);
    let workers = config
        .workers
        .as_ref()
        .and_then(|w| w.recommended_workers)
        .unwrap_or(4);
    let cores_per_worker = config
        .infrastructure
        .as_ref()
        .and_then(|i| i.recommended_cores)
        .unwrap_or(16);

    // Estimate candidate count from phases or target
    let (candidates, avg_digits) = estimate_candidates(config);

    let spc = secs_per_candidate(&config.project.form, avg_digits, has_pfgw);
    let total_test_secs = candidates as f64 * spc;
    let total_core_hours = total_test_secs / 3600.0;
    let total_cores = workers * cores_per_worker;
    let duration_hours = total_core_hours / total_cores as f64;
    let cost = total_core_hours * cloud_rate;

    CostEstimate {
        estimated_candidates: candidates,
        estimated_test_time_secs: total_test_secs,
        total_core_hours,
        total_cost_usd: cost,
        estimated_duration_hours: duration_hours,
        workers_recommended: workers,
    }
}

/// Estimate candidate count and average digit size from config.
fn estimate_candidates(config: &ProjectConfig) -> (u64, u64) {
    // If phases are defined, sum their ranges
    if !config.strategy.phases.is_empty() {
        let mut total = 0u64;
        let mut total_digits = 0u64;
        let mut phase_count = 0u64;
        for phase in &config.strategy.phases {
            let (start, end) = extract_range_from_params(&phase.search_params);
            if end > start {
                total += end - start;
                // Rough digit estimate from range midpoint
                let mid = (start + end) / 2;
                total_digits += estimate_digits_for_form(&config.project.form, mid);
                phase_count += 1;
            }
        }
        let avg_digits = if phase_count > 0 {
            total_digits / phase_count
        } else {
            1000
        };
        return (total.max(1), avg_digits.max(1));
    }

    // Fall back to target range
    if let (Some(start), Some(end)) = (config.target.range_start, config.target.range_end) {
        let mid = (start + end) / 2;
        let digits = estimate_digits_for_form(&config.project.form, mid);
        return ((end - start).max(1), digits.max(1));
    }

    // Default: assume 10K candidates at 1K digits
    (10_000, 1000)
}

/// Extract (start, end) range from search_params JSON.
pub(crate) fn extract_range_from_params(params: &serde_json::Value) -> (u64, u64) {
    let start = params
        .get("start")
        .or_else(|| params.get("min_n"))
        .or_else(|| params.get("min_exp"))
        .or_else(|| params.get("min_digits"))
        .or_else(|| params.get("min_base"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let end = params
        .get("end")
        .or_else(|| params.get("max_n"))
        .or_else(|| params.get("max_exp"))
        .or_else(|| params.get("max_digits"))
        .or_else(|| params.get("max_base"))
        .and_then(|v| v.as_u64())
        .unwrap_or(start);
    (start, end)
}

/// Rough decimal digit estimate for a form at parameter value n.
pub fn estimate_digits_for_form(form: &str, n: u64) -> u64 {
    match form {
        // n! has ~n*log10(n/e) digits (Stirling)
        "factorial" => {
            if n < 3 {
                return 1;
            }
            let nf = n as f64;
            (nf * (nf / std::f64::consts::E).log10()) as u64
        }
        // p# has ~p/ln(10) digits (prime number theorem)
        "primorial" => (n as f64 / std::f64::consts::LN_10) as u64,
        // k*b^n has ~n*log10(b) digits
        "kbn" | "twin" | "sophie_germain" => (n as f64 * 2.0f64.log10()) as u64,
        // n*2^n+1 has ~n*log10(2) digits
        "cullen_woodall" => (n as f64 * 2.0f64.log10()) as u64,
        // (2^p+1)/3 has ~p*log10(2) digits
        "wagstaff" => (n as f64 * 2.0f64.log10()) as u64,
        // (2^n±1)²-2 has ~2n*log10(2) digits
        "carol_kynea" => (2.0 * n as f64 * 2.0f64.log10()) as u64,
        // Palindromic primes: n is the digit count
        "palindromic" | "near_repdigit" => n,
        // (b^n-1)/(b-1): n digits in base b, ~n*log10(b) decimal digits
        "repunit" => n,
        // b^(2^n)+1: digits depend on base range
        "gen_fermat" => (n as f64 * 2.0f64.log10()) as u64,
        _ => n,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── secs_per_candidate ──────────────────────────────────────

    #[test]
    fn secs_per_candidate_factorial_scales_with_digits() {
        let t1 = secs_per_candidate("factorial", 1000, false);
        let t2 = secs_per_candidate("factorial", 2000, false);
        // Larger digit count should take more time (power-law d^2.5)
        assert!(t2 > t1, "2000-digit factorial should be slower than 1000-digit");
        // The ratio should be roughly 2^2.5 ≈ 5.66
        let ratio = t2 / t1;
        assert!(ratio > 4.0 && ratio < 7.0, "ratio should be ~5.66, got {}", ratio);
    }

    #[test]
    fn secs_per_candidate_kbn_scales_with_digits() {
        let t1 = secs_per_candidate("kbn", 1000, false);
        let t2 = secs_per_candidate("kbn", 2000, false);
        assert!(t2 > t1);
        // Power-law d^2.0, ratio should be 2^2 = 4.0
        let ratio = t2 / t1;
        assert!((ratio - 4.0).abs() < 0.1, "ratio should be ~4.0, got {}", ratio);
    }

    #[test]
    fn secs_per_candidate_palindromic() {
        let t = secs_per_candidate("palindromic", 5000, false);
        assert!(t > 0.0);
    }

    #[test]
    fn secs_per_candidate_near_repdigit_same_as_palindromic() {
        let t_pal = secs_per_candidate("palindromic", 3000, false);
        let t_nr = secs_per_candidate("near_repdigit", 3000, false);
        assert!((t_pal - t_nr).abs() < f64::EPSILON, "palindromic and near_repdigit should share the same model");
    }

    #[test]
    fn secs_per_candidate_wagstaff_is_slowest() {
        // Wagstaff has the highest base coefficient (0.8) with d^2.5 — should be
        // slower than most forms at the same digit count.
        let t_wag = secs_per_candidate("wagstaff", 5000, false);
        let t_kbn = secs_per_candidate("kbn", 5000, false);
        assert!(t_wag > t_kbn, "wagstaff should be slower than kbn");
    }

    #[test]
    fn secs_per_candidate_primorial_same_as_factorial() {
        let t_fac = secs_per_candidate("factorial", 2000, false);
        let t_pri = secs_per_candidate("primorial", 2000, false);
        assert!((t_fac - t_pri).abs() < f64::EPSILON);
    }

    #[test]
    fn secs_per_candidate_twin_same_as_kbn() {
        let t_kbn = secs_per_candidate("kbn", 3000, false);
        let t_twin = secs_per_candidate("twin", 3000, false);
        assert!((t_kbn - t_twin).abs() < f64::EPSILON);
    }

    #[test]
    fn secs_per_candidate_sophie_germain_same_as_kbn() {
        let t_kbn = secs_per_candidate("kbn", 4000, false);
        let t_sg = secs_per_candidate("sophie_germain", 4000, false);
        assert!((t_kbn - t_sg).abs() < f64::EPSILON);
    }

    #[test]
    fn secs_per_candidate_cullen_woodall() {
        let t = secs_per_candidate("cullen_woodall", 2000, false);
        assert!(t > 0.0);
        // cullen_woodall uses 0.2 * d^2.2
        let t_ck = secs_per_candidate("carol_kynea", 2000, false);
        assert!((t - t_ck).abs() < f64::EPSILON, "cullen_woodall and carol_kynea share the same model");
    }

    #[test]
    fn secs_per_candidate_repunit() {
        let t = secs_per_candidate("repunit", 3000, false);
        assert!(t > 0.0);
    }

    #[test]
    fn secs_per_candidate_gen_fermat() {
        let t = secs_per_candidate("gen_fermat", 2000, false);
        assert!(t > 0.0);
    }

    #[test]
    fn secs_per_candidate_unknown_form_uses_default() {
        let t_unknown = secs_per_candidate("unknown_xyz", 5000, false);
        let t_fac = secs_per_candidate("factorial", 5000, false);
        assert!((t_unknown - t_fac).abs() < f64::EPSILON, "unknown form should use factorial's model (default)");
    }

    #[test]
    fn secs_per_candidate_zero_digits() {
        // Should not panic even with 0 digits (d=0, result=0)
        let t = secs_per_candidate("factorial", 0, false);
        assert!(t == 0.0 || t >= 0.0);
    }

    // ── PFGW acceleration factor ────────────────────────────────

    #[test]
    fn pfgw_acceleration_applied_at_10k_digits() {
        let without = secs_per_candidate("factorial", 10_000, false);
        let with = secs_per_candidate("factorial", 10_000, true);
        let ratio = without / with;
        assert!((ratio - 50.0).abs() < 0.01, "PFGW should give 50x speedup at 10K digits, got {}", ratio);
    }

    #[test]
    fn pfgw_acceleration_not_applied_below_10k() {
        let without = secs_per_candidate("factorial", 9_999, false);
        let with = secs_per_candidate("factorial", 9_999, true);
        assert!((without - with).abs() < f64::EPSILON, "PFGW should not apply below 10K digits");
    }

    #[test]
    fn pfgw_acceleration_applied_above_10k() {
        let without = secs_per_candidate("kbn", 50_000, false);
        let with = secs_per_candidate("kbn", 50_000, true);
        let ratio = without / with;
        assert!((ratio - 50.0).abs() < 0.01, "PFGW should give 50x speedup above 10K digits");
    }

    #[test]
    fn pfgw_false_no_acceleration() {
        let t1 = secs_per_candidate("wagstaff", 20_000, false);
        let t2 = secs_per_candidate("wagstaff", 20_000, false);
        assert!((t1 - t2).abs() < f64::EPSILON);
    }

    // ── estimate_digits_for_form ────────────────────────────────

    #[test]
    fn digits_factorial_stirling() {
        // 100! has 158 digits (known value)
        let est = estimate_digits_for_form("factorial", 100);
        // Stirling's approximation: 100 * log10(100/e) ≈ 100 * 1.564 ≈ 156
        assert!(est >= 140 && est <= 170, "100! should have ~158 digits, got {}", est);
    }

    #[test]
    fn digits_factorial_small_n() {
        assert_eq!(estimate_digits_for_form("factorial", 1), 1);
        assert_eq!(estimate_digits_for_form("factorial", 2), 1);
    }

    #[test]
    fn digits_primorial_pnt() {
        // p# ≈ e^p, so digits ≈ p / ln(10) ≈ p / 2.303
        let est = estimate_digits_for_form("primorial", 1000);
        // 1000 / 2.303 ≈ 434
        assert!(est >= 400 && est <= 460, "1000# should have ~434 digits, got {}", est);
    }

    #[test]
    fn digits_kbn_log_base2() {
        // k*2^n has ~n*log10(2) ≈ n*0.301 digits
        let est = estimate_digits_for_form("kbn", 10_000);
        // 10000 * 0.301 ≈ 3010
        assert!(est >= 2900 && est <= 3100, "k*2^10000 should have ~3010 digits, got {}", est);
    }

    #[test]
    fn digits_twin_same_as_kbn() {
        assert_eq!(
            estimate_digits_for_form("twin", 5000),
            estimate_digits_for_form("kbn", 5000),
        );
    }

    #[test]
    fn digits_sophie_germain_same_as_kbn() {
        assert_eq!(
            estimate_digits_for_form("sophie_germain", 5000),
            estimate_digits_for_form("kbn", 5000),
        );
    }

    #[test]
    fn digits_cullen_woodall_same_as_kbn() {
        // n*2^n+1: dominant term is 2^n, digits ≈ n*log10(2)
        assert_eq!(
            estimate_digits_for_form("cullen_woodall", 5000),
            estimate_digits_for_form("kbn", 5000),
        );
    }

    #[test]
    fn digits_wagstaff_same_as_kbn() {
        // (2^p+1)/3: digits ≈ p*log10(2)
        assert_eq!(
            estimate_digits_for_form("wagstaff", 5000),
            estimate_digits_for_form("kbn", 5000),
        );
    }

    #[test]
    fn digits_carol_kynea_double() {
        // (2^n±1)²−2 has ~2n*log10(2) digits — double kbn
        let carol = estimate_digits_for_form("carol_kynea", 5000);
        let kbn = estimate_digits_for_form("kbn", 5000);
        assert_eq!(carol, 2 * kbn, "carol_kynea should have twice the digits of kbn");
    }

    #[test]
    fn digits_palindromic_is_n() {
        assert_eq!(estimate_digits_for_form("palindromic", 100), 100);
        assert_eq!(estimate_digits_for_form("palindromic", 12345), 12345);
    }

    #[test]
    fn digits_near_repdigit_is_n() {
        assert_eq!(estimate_digits_for_form("near_repdigit", 200), 200);
    }

    #[test]
    fn digits_repunit_is_n() {
        assert_eq!(estimate_digits_for_form("repunit", 500), 500);
    }

    #[test]
    fn digits_gen_fermat() {
        // b^(2^n)+1: digits ≈ n*log10(2) — same formula as kbn
        assert_eq!(
            estimate_digits_for_form("gen_fermat", 5000),
            estimate_digits_for_form("kbn", 5000),
        );
    }

    #[test]
    fn digits_unknown_form_is_n() {
        assert_eq!(estimate_digits_for_form("some_unknown", 777), 777);
    }

    // ── extract_range_from_params ───────────────────────────────

    #[test]
    fn extract_range_start_end() {
        let params = serde_json::json!({"start": 100, "end": 500});
        assert_eq!(extract_range_from_params(&params), (100, 500));
    }

    #[test]
    fn extract_range_min_n_max_n() {
        let params = serde_json::json!({"min_n": 1000, "max_n": 5000});
        assert_eq!(extract_range_from_params(&params), (1000, 5000));
    }

    #[test]
    fn extract_range_min_exp_max_exp() {
        let params = serde_json::json!({"min_exp": 14000000, "max_exp": 20000000});
        assert_eq!(extract_range_from_params(&params), (14000000, 20000000));
    }

    #[test]
    fn extract_range_min_digits_max_digits() {
        let params = serde_json::json!({"min_digits": 5, "max_digits": 21});
        assert_eq!(extract_range_from_params(&params), (5, 21));
    }

    #[test]
    fn extract_range_min_base_max_base() {
        let params = serde_json::json!({"min_base": 2, "max_base": 100});
        assert_eq!(extract_range_from_params(&params), (2, 100));
    }

    #[test]
    fn extract_range_missing_keys_defaults_to_zero() {
        let params = serde_json::json!({"search_type": "factorial"});
        assert_eq!(extract_range_from_params(&params), (0, 0));
    }

    #[test]
    fn extract_range_priority_start_over_min_n() {
        // "start" takes priority when both "start" and "min_n" are present
        let params = serde_json::json!({"start": 100, "min_n": 200, "end": 500, "max_n": 600});
        let (s, e) = extract_range_from_params(&params);
        assert_eq!(s, 100);
        assert_eq!(e, 500);
    }

    #[test]
    fn extract_range_only_start_no_end() {
        // If only start is provided, end defaults to start
        let params = serde_json::json!({"start": 42});
        let (s, e) = extract_range_from_params(&params);
        assert_eq!(s, 42);
        assert_eq!(e, 42);
    }

    #[test]
    fn extract_range_empty_object() {
        let params = serde_json::json!({});
        assert_eq!(extract_range_from_params(&params), (0, 0));
    }

    // ── estimate_project_cost ───────────────────────────────────

    #[test]
    fn estimate_project_cost_minimal_config() {
        let config = ProjectConfig {
            project: super::super::config::ProjectMeta {
                name: "test".into(),
                description: String::new(),
                objective: super::super::config::Objective::Survey,
                form: "factorial".into(),
                author: String::new(),
                tags: vec![],
            },
            target: super::super::config::TargetConfig {
                target_digits: None,
                range_start: Some(1),
                range_end: Some(1000),
            },
            competitive: None,
            strategy: super::super::config::StrategyConfig {
                auto_strategy: false,
                phases: vec![],
            },
            infrastructure: None,
            budget: None,
            workers: None,
        };
        let est = estimate_project_cost(&config);
        assert_eq!(est.estimated_candidates, 999); // 1000 - 1
        assert!(est.total_core_hours >= 0.0);
        assert!(est.total_cost_usd >= 0.0);
        assert_eq!(est.workers_recommended, 4); // default
    }

    #[test]
    fn estimate_project_cost_with_phases() {
        let config = ProjectConfig {
            project: super::super::config::ProjectMeta {
                name: "test".into(),
                description: String::new(),
                objective: super::super::config::Objective::Survey,
                form: "kbn".into(),
                author: String::new(),
                tags: vec![],
            },
            target: super::super::config::TargetConfig::default(),
            competitive: None,
            strategy: super::super::config::StrategyConfig {
                auto_strategy: false,
                phases: vec![super::super::config::PhaseConfig {
                    name: "sweep".into(),
                    description: String::new(),
                    search_params: serde_json::json!({"min_n": 1000, "max_n": 5000}),
                    block_size: Some(100),
                    depends_on: None,
                    activation_condition: None,
                    completion: "all_blocks_done".into(),
                }],
            },
            infrastructure: None,
            budget: None,
            workers: None,
        };
        let est = estimate_project_cost(&config);
        assert_eq!(est.estimated_candidates, 4000); // 5000 - 1000
        assert!(est.total_core_hours > 0.0);
        assert!(est.total_cost_usd > 0.0);
    }

    #[test]
    fn estimate_project_cost_with_pfgw_reduces_time() {
        let base_config = ProjectConfig {
            project: super::super::config::ProjectMeta {
                name: "test".into(),
                description: String::new(),
                objective: super::super::config::Objective::Record,
                form: "factorial".into(),
                author: String::new(),
                tags: vec![],
            },
            target: super::super::config::TargetConfig {
                target_digits: None,
                range_start: Some(10_000),
                range_end: Some(20_000),
            },
            competitive: None,
            strategy: super::super::config::StrategyConfig::default(),
            infrastructure: None,
            budget: None,
            workers: None,
        };

        let est_no_pfgw = estimate_project_cost(&base_config);

        let mut pfgw_config = base_config.clone();
        pfgw_config.infrastructure = Some(super::super::config::InfrastructureConfig {
            min_ram_gb: None,
            min_cores: None,
            recommended_cores: None,
            required_tools: vec![],
            preferred_tools: vec!["pfgw".to_string()],
        });

        let est_with_pfgw = estimate_project_cost(&pfgw_config);

        // Both should estimate the same number of candidates
        assert_eq!(est_no_pfgw.estimated_candidates, est_with_pfgw.estimated_candidates);

        // PFGW config should be faster (only if digits >= 10K)
        // The digit estimate for factorial at n=15000 is large enough
        let avg_n = 15000u64;
        let digits = estimate_digits_for_form("factorial", avg_n);
        if digits >= 10_000 {
            assert!(
                est_with_pfgw.total_core_hours < est_no_pfgw.total_core_hours,
                "PFGW should reduce core hours"
            );
        }
    }

    #[test]
    fn estimate_project_cost_default_cloud_rate() {
        let config = ProjectConfig {
            project: super::super::config::ProjectMeta {
                name: "test".into(),
                description: String::new(),
                objective: super::super::config::Objective::Survey,
                form: "factorial".into(),
                author: String::new(),
                tags: vec![],
            },
            target: super::super::config::TargetConfig::default(),
            competitive: None,
            strategy: super::super::config::StrategyConfig::default(),
            infrastructure: None,
            budget: None,
            workers: None,
        };
        let est = estimate_project_cost(&config);
        // Default cloud rate is $0.04/core-hr
        // total_cost_usd = total_core_hours * 0.04
        if est.total_core_hours > 0.0 {
            let expected_cost = est.total_core_hours * 0.04;
            assert!(
                (est.total_cost_usd - expected_cost).abs() < 0.001,
                "cost should use default $0.04/core-hr"
            );
        }
    }

    #[test]
    fn estimate_project_cost_custom_workers() {
        let config = ProjectConfig {
            project: super::super::config::ProjectMeta {
                name: "test".into(),
                description: String::new(),
                objective: super::super::config::Objective::Survey,
                form: "factorial".into(),
                author: String::new(),
                tags: vec![],
            },
            target: super::super::config::TargetConfig {
                target_digits: None,
                range_start: Some(1),
                range_end: Some(100),
            },
            competitive: None,
            strategy: super::super::config::StrategyConfig::default(),
            infrastructure: None,
            budget: None,
            workers: Some(super::super::config::WorkerConfig {
                min_workers: None,
                max_workers: None,
                recommended_workers: Some(8),
            }),
        };
        let est = estimate_project_cost(&config);
        assert_eq!(est.workers_recommended, 8);
    }
}
