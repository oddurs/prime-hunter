//! Empirical cost estimation model for prime-hunting projects.
//!
//! Uses a per-form power-law timing model to estimate computation cost from
//! candidate count and digit size. Cloud pricing defaults to Hetzner AX42
//! ($0.04/core-hr). PFGW gives ~50x speedup, GWNUM ~100x.

use serde::Serialize;
use super::config::ProjectConfig;

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
fn secs_per_candidate(form: &str, digits: u64, has_pfgw: bool) -> f64 {
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
pub(crate) fn estimate_digits_for_form(form: &str, n: u64) -> u64 {
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
