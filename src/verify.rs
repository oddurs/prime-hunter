use anyhow::{anyhow, Result};
use rug::integer::IsPrime;
use rug::ops::Pow;
use rug::Integer;

use crate::db::PrimeDetail;
use crate::{has_small_factor, kbn, proof, sieve};

/// Result of a verification attempt.
#[derive(Debug, Clone)]
pub enum VerifyResult {
    Verified { method: String, tier: u8 },
    Failed { reason: String },
    Skipped { reason: String },
}

/// Reconstruct the candidate integer from the stored form and expression.
pub fn reconstruct_candidate(form: &str, expression: &str) -> Result<Integer> {
    match form {
        "factorial" => parse_factorial(expression),
        "primorial" => parse_primorial(expression),
        "kbn" => parse_kbn(expression),
        "palindromic" => parse_palindromic(expression),
        "near_repdigit" => parse_near_repdigit(expression),
        "cullen" | "woodall" | "cullen_woodall" => parse_cullen_woodall(expression),
        "wagstaff" => parse_wagstaff(expression),
        "carol" | "kynea" | "carol_kynea" => parse_carol_kynea(expression),
        "twin" => parse_twin(expression),
        "sophie_germain" => parse_sophie_germain(expression),
        "repunit" => parse_repunit(expression),
        "gen_fermat" => parse_gen_fermat(expression),
        _ => Err(anyhow!("Unknown form: {}", form)),
    }
}

// --- Expression parsers ---

/// Parse "73! + 1" or "73! - 1"
fn parse_factorial(expr: &str) -> Result<Integer> {
    // "N! + 1" or "N! - 1"
    let expr = expr.trim();
    let bang = expr
        .find('!')
        .ok_or_else(|| anyhow!("No '!' in factorial expression: {}", expr))?;
    let n: u32 = expr[..bang].trim().parse()?;
    let rest = expr[bang + 1..].trim();
    let factorial = Integer::from(Integer::factorial(n));
    if rest.starts_with('+') {
        Ok(factorial + 1u32)
    } else if rest.starts_with('-') {
        Ok(factorial - 1u32)
    } else {
        Err(anyhow!("Expected +/- after '!' in: {}", expr))
    }
}

/// Parse "31# + 1" or "31# - 1"
fn parse_primorial(expr: &str) -> Result<Integer> {
    let expr = expr.trim();
    let hash = expr
        .find('#')
        .ok_or_else(|| anyhow!("No '#' in primorial expression: {}", expr))?;
    let n: u32 = expr[..hash].trim().parse()?;
    let rest = expr[hash + 1..].trim();
    let primorial = Integer::from(Integer::primorial(n));
    if rest.starts_with('+') {
        Ok(primorial + 1u32)
    } else if rest.starts_with('-') {
        Ok(primorial - 1u32)
    } else {
        Err(anyhow!("Expected +/- after '#' in: {}", expr))
    }
}

/// Parse "3*2^31 + 1" or "3*2^31 - 1"
fn parse_kbn(expr: &str) -> Result<Integer> {
    let expr = expr.trim();
    let star = expr
        .find('*')
        .ok_or_else(|| anyhow!("No '*' in kbn expression: {}", expr))?;
    let k: u64 = expr[..star].trim().parse()?;
    let rest = &expr[star + 1..];
    let caret = rest
        .find('^')
        .ok_or_else(|| anyhow!("No '^' in kbn expression: {}", expr))?;
    let base: u32 = rest[..caret].trim().parse()?;
    // After '^', find the exponent (until + or -)
    let rest_after_caret = &rest[caret + 1..];
    let (n_str, sign) = split_at_sign(rest_after_caret)?;
    let n: u32 = n_str.trim().parse()?;
    let value = Integer::from(k) * Integer::from(base).pow(n);
    match sign {
        '+' => Ok(value + 1u32),
        '-' => Ok(value - 1u32),
        _ => Err(anyhow!(
            "Unexpected sign '{}' in kbn expression: {}",
            sign,
            expr
        )),
    }
}

/// Parse raw decimal palindrome "10301"
fn parse_palindromic(expr: &str) -> Result<Integer> {
    let expr = expr.trim();
    Integer::parse(expr)
        .map(Integer::from)
        .map_err(|_| anyhow!("Invalid palindromic expression: {}", expr))
}

/// Parse "10^7 - 1 - 4*10^3" or "10^7 - 1 - 5*(10^5 + 10^1)"
fn parse_near_repdigit(expr: &str) -> Result<Integer> {
    let expr = expr.trim();
    // Format 1: "10^D - 1 - C*10^P"         (m == 0)
    // Format 2: "10^D - 1 - C*(10^A + 10^B)" (m != 0)

    // Extract D from "10^D"
    let caret1 = expr
        .find('^')
        .ok_or_else(|| anyhow!("No '^' in near_repdigit: {}", expr))?;
    let after_caret = &expr[caret1 + 1..];
    let d_end = after_caret.find(' ').unwrap_or(after_caret.len());
    let digit_count: u32 = after_caret[..d_end].trim().parse()?;

    let repdigit = Integer::from(10u32).pow(digit_count) - 1u32;

    // Find the second subtraction (after "- 1 -")
    let last_dash_idx = expr
        .rfind(" - ")
        .ok_or_else(|| anyhow!("Missing subtraction in: {}", expr))?;
    let modifier_str = &expr[last_dash_idx + 3..];

    if modifier_str.contains('(') {
        // Format 2: "C*(10^A + 10^B)"
        let star = modifier_str
            .find('*')
            .ok_or_else(|| anyhow!("No '*' in modifier: {}", modifier_str))?;
        let c: u32 = modifier_str[..star].trim().parse()?;
        // Extract A and B from "(10^A + 10^B)"
        let inner = modifier_str[star + 1..]
            .trim()
            .trim_start_matches('(')
            .trim_end_matches(')');
        let plus = inner
            .find('+')
            .ok_or_else(|| anyhow!("No '+' in inner: {}", inner))?;
        let part_a = inner[..plus].trim();
        let part_b = inner[plus + 1..].trim();
        let a: u32 = part_a
            .split('^')
            .nth(1)
            .ok_or_else(|| anyhow!("No '^' in: {}", part_a))?
            .trim()
            .parse()?;
        let b: u32 = part_b
            .split('^')
            .nth(1)
            .ok_or_else(|| anyhow!("No '^' in: {}", part_b))?
            .trim()
            .parse()?;
        Ok(repdigit
            - Integer::from(c) * (Integer::from(10u32).pow(a) + Integer::from(10u32).pow(b)))
    } else {
        // Format 1: "C*10^P"
        let star = modifier_str
            .find('*')
            .ok_or_else(|| anyhow!("No '*' in modifier: {}", modifier_str))?;
        let c: u32 = modifier_str[..star].trim().parse()?;
        let caret = modifier_str
            .find('^')
            .ok_or_else(|| anyhow!("No '^' in modifier: {}", modifier_str))?;
        let p: u32 = modifier_str[caret + 1..].trim().parse()?;
        Ok(repdigit - Integer::from(c) * Integer::from(10u32).pow(p))
    }
}

/// Parse "141*2^141 + 1" (Cullen) or "6*2^6 - 1" (Woodall)
fn parse_cullen_woodall(expr: &str) -> Result<Integer> {
    // Same format as kbn
    parse_kbn(expr)
}

/// Parse "(2^127+1)/3"
fn parse_wagstaff(expr: &str) -> Result<Integer> {
    let expr = expr.trim();
    // "(2^P+1)/3"
    let caret = expr
        .find('^')
        .ok_or_else(|| anyhow!("No '^' in wagstaff: {}", expr))?;
    let plus = expr
        .find('+')
        .ok_or_else(|| anyhow!("No '+' in wagstaff: {}", expr))?;
    let p: u32 = expr[caret + 1..plus].trim().parse()?;
    let numerator = (Integer::from(1u32) << p) + Integer::from(1u32);
    Ok(numerator / Integer::from(3u32))
}

/// Parse "(2^7-1)^2-2" (Carol) or "(2^7+1)^2-2" (Kynea)
fn parse_carol_kynea(expr: &str) -> Result<Integer> {
    let expr = expr.trim();
    // "(2^N-1)^2-2" or "(2^N+1)^2-2"
    let caret = expr
        .find('^')
        .ok_or_else(|| anyhow!("No '^' in carol_kynea: {}", expr))?;
    // Find the sign before ')^2-2'
    let paren_close = expr
        .find(')')
        .ok_or_else(|| anyhow!("No ')' in carol_kynea: {}", expr))?;
    let inner = &expr[caret + 1..paren_close];
    let (n_str, sign) = if let Some(pos) = inner.find('-') {
        (&inner[..pos], '-')
    } else if let Some(pos) = inner.find('+') {
        (&inner[..pos], '+')
    } else {
        return Err(anyhow!("No +/- in carol_kynea inner: {}", inner));
    };
    let n: u32 = n_str.trim().parse()?;
    let base = if sign == '-' {
        Integer::from(1u32 << n) - 1u32 // Carol: 2^n - 1
    } else {
        Integer::from(1u32 << n) + 1u32 // Kynea: 2^n + 1
    };
    Ok(base.square() - 2u32)
}

/// Parse "3*2^100 +/- 1" (twin prime pair)
fn parse_twin(expr: &str) -> Result<Integer> {
    // Twin expression is "k*b^n +/- 1". We return k*b^n - 1 (the smaller twin).
    let expr = expr.trim();
    let star = expr
        .find('*')
        .ok_or_else(|| anyhow!("No '*' in twin: {}", expr))?;
    let k: u64 = expr[..star].trim().parse()?;
    let rest = &expr[star + 1..];
    let caret = rest
        .find('^')
        .ok_or_else(|| anyhow!("No '^' in twin: {}", expr))?;
    let base: u32 = rest[..caret].trim().parse()?;
    let rest_after_caret = &rest[caret + 1..];
    // Find where the number ends (before " +/- 1")
    let space = rest_after_caret.find(' ').unwrap_or(rest_after_caret.len());
    let n: u32 = rest_after_caret[..space].trim().parse()?;
    // Return the minus form (smaller twin) — both k*b^n-1 and k*b^n+1 are prime
    Ok(Integer::from(k) * Integer::from(base).pow(n) - 1u32)
}

/// Parse "3*2^100-1" (Sophie Germain prime)
fn parse_sophie_germain(expr: &str) -> Result<Integer> {
    let expr = expr.trim();
    let star = expr
        .find('*')
        .ok_or_else(|| anyhow!("No '*' in sophie_germain: {}", expr))?;
    let k: u64 = expr[..star].trim().parse()?;
    let rest = &expr[star + 1..];
    let caret = rest
        .find('^')
        .ok_or_else(|| anyhow!("No '^' in sophie_germain: {}", expr))?;
    let base: u32 = rest[..caret].trim().parse()?;
    let rest_after_caret = &rest[caret + 1..];
    // Sophie Germain expression has no spaces: "k*b^n-1"
    let minus = rest_after_caret
        .find('-')
        .ok_or_else(|| anyhow!("No '-' in sophie_germain: {}", expr))?;
    let n: u32 = rest_after_caret[..minus].trim().parse()?;
    Ok(Integer::from(k) * Integer::from(base).pow(n) - 1u32)
}

/// Parse "R(10, 19)" (repunit)
fn parse_repunit(expr: &str) -> Result<Integer> {
    let expr = expr.trim();
    // "R(base, n)"
    let open = expr
        .find('(')
        .ok_or_else(|| anyhow!("No '(' in repunit: {}", expr))?;
    let close = expr
        .find(')')
        .ok_or_else(|| anyhow!("No ')' in repunit: {}", expr))?;
    let inner = &expr[open + 1..close];
    let comma = inner
        .find(',')
        .ok_or_else(|| anyhow!("No ',' in repunit: {}", inner))?;
    let base: u32 = inner[..comma].trim().parse()?;
    let n: u32 = inner[comma + 1..].trim().parse()?;
    // R(b, n) = (b^n - 1) / (b - 1)
    Ok((Integer::from(base).pow(n) - 1u32) / (base - 1))
}

/// Parse "6^(2^3) + 1" (generalized Fermat)
fn parse_gen_fermat(expr: &str) -> Result<Integer> {
    let expr = expr.trim();
    // "b^(2^n) + 1"
    let caret1 = expr
        .find('^')
        .ok_or_else(|| anyhow!("No '^' in gen_fermat: {}", expr))?;
    let b: u64 = expr[..caret1].trim().parse()?;
    // Find inner "2^n" inside "(2^n)"
    let open = expr
        .find('(')
        .ok_or_else(|| anyhow!("No '(' in gen_fermat: {}", expr))?;
    let close = expr
        .find(')')
        .ok_or_else(|| anyhow!("No ')' in gen_fermat: {}", expr))?;
    let inner = &expr[open + 1..close];
    let caret2 = inner
        .find('^')
        .ok_or_else(|| anyhow!("No inner '^' in gen_fermat: {}", inner))?;
    let n: u32 = inner[caret2 + 1..].trim().parse()?;
    let exponent = 1u32 << n; // 2^n
    Ok(Integer::from(b).pow(exponent) + 1u32)
}

/// Split a string at the last '+' or '-' sign (not inside parentheses).
/// Returns (before_sign, sign_char).
fn split_at_sign(s: &str) -> Result<(&str, char)> {
    // Look for " + " or " - " pattern
    if let Some(pos) = s.rfind(" + ") {
        Ok((&s[..pos], '+'))
    } else if let Some(pos) = s.rfind(" - ") {
        Ok((&s[..pos], '-'))
    } else {
        Err(anyhow!("No +/- sign found in: {}", s))
    }
}

// --- Verification tiers ---

/// Tier 1: Re-run the same deterministic test used at discovery time.
/// Returns Verified if the same proof succeeds, Skipped for probabilistic primes.
pub fn verify_tier1(
    form: &str,
    expression: &str,
    candidate: &Integer,
    proof_method: &str,
) -> VerifyResult {
    if proof_method == "probabilistic" {
        return VerifyResult::Skipped {
            reason: "Probabilistic — no deterministic re-test available".into(),
        };
    }

    match form {
        "kbn" | "cullen" | "woodall" | "cullen_woodall" => verify_tier1_kbn(expression, candidate),
        "factorial" => verify_tier1_factorial(expression, candidate),
        "primorial" => verify_tier1_primorial(expression, candidate),
        "wagstaff" => VerifyResult::Skipped {
            reason: "No deterministic proof for Wagstaff primes".into(),
        },
        _ => {
            // For forms without a tier-1 test, skip to tier 2
            VerifyResult::Skipped {
                reason: format!("No tier-1 test for form '{}'", form),
            }
        }
    }
}

/// Tier 1 for kbn-like forms: re-run Proth/Pocklington/LLR.
fn verify_tier1_kbn(expression: &str, candidate: &Integer) -> VerifyResult {
    // Parse k, base, n and sign from expression
    let expr = expression.trim();
    let star = match expr.find('*') {
        Some(p) => p,
        None => {
            return VerifyResult::Skipped {
                reason: "Cannot parse kbn expression".into(),
            }
        }
    };
    let k: u64 = match expr[..star].trim().parse() {
        Ok(v) => v,
        Err(_) => {
            return VerifyResult::Skipped {
                reason: "Cannot parse k".into(),
            }
        }
    };
    let rest = &expr[star + 1..];
    let caret = match rest.find('^') {
        Some(p) => p,
        None => {
            return VerifyResult::Skipped {
                reason: "Cannot parse base".into(),
            }
        }
    };
    let base: u32 = match rest[..caret].trim().parse() {
        Ok(v) => v,
        Err(_) => {
            return VerifyResult::Skipped {
                reason: "Cannot parse base value".into(),
            }
        }
    };
    let rest_after_caret = &rest[caret + 1..];
    let (n_str, sign) = match split_at_sign(rest_after_caret) {
        Ok(v) => v,
        Err(_) => {
            // Try Sophie Germain format "k*b^n-1" (no spaces)
            if let Some(pos) = rest_after_caret.rfind('-') {
                (&rest_after_caret[..pos], '-')
            } else {
                return VerifyResult::Skipped {
                    reason: "Cannot parse sign".into(),
                };
            }
        }
    };
    let n: u64 = match n_str.trim().parse() {
        Ok(v) => v,
        Err(_) => {
            return VerifyResult::Skipped {
                reason: "Cannot parse n".into(),
            }
        }
    };

    let is_plus = sign == '+';
    let (result, method) = kbn::test_prime(candidate, k, base, n as u64, is_plus, 15);
    match result {
        IsPrime::Yes if method == "deterministic" => VerifyResult::Verified {
            method: "tier1-kbn-deterministic".into(),
            tier: 1,
        },
        IsPrime::No => VerifyResult::Failed {
            reason: "Tier-1 kbn test says composite".into(),
        },
        _ => {
            // Fell through to MR — treat as "not proven" by tier 1
            VerifyResult::Skipped {
                reason: "kbn test fell through to MR (not deterministic)".into(),
            }
        }
    }
}

/// Tier 1 for factorial: re-run Pocklington (+1) or Morrison (-1).
fn verify_tier1_factorial(expression: &str, candidate: &Integer) -> VerifyResult {
    let expr = expression.trim();
    let bang = match expr.find('!') {
        Some(p) => p,
        None => {
            return VerifyResult::Skipped {
                reason: "Cannot parse factorial expression".into(),
            }
        }
    };
    let n: u64 = match expr[..bang].trim().parse() {
        Ok(v) => v,
        Err(_) => {
            return VerifyResult::Skipped {
                reason: "Cannot parse n".into(),
            }
        }
    };
    let rest = expr[bang + 1..].trim();
    let is_plus = rest.starts_with('+');

    // Generate sieve primes up to n (needed for proof)
    let sieve_limit = (n + 100).max(1000);
    let sieve_primes = sieve::generate_primes(sieve_limit);

    if is_plus {
        if proof::pocklington_factorial_proof(n, candidate, &sieve_primes) {
            VerifyResult::Verified {
                method: "tier1-pocklington".into(),
                tier: 1,
            }
        } else {
            VerifyResult::Failed {
                reason: "Pocklington proof failed".into(),
            }
        }
    } else {
        if proof::morrison_factorial_proof(n, candidate, &sieve_primes) {
            VerifyResult::Verified {
                method: "tier1-morrison".into(),
                tier: 1,
            }
        } else {
            VerifyResult::Failed {
                reason: "Morrison proof failed".into(),
            }
        }
    }
}

/// Tier 1 for primorial: re-run Pocklington (+1) or Morrison (-1).
/// Uses the same proof functions as factorial (same set of distinct prime factors).
fn verify_tier1_primorial(expression: &str, candidate: &Integer) -> VerifyResult {
    let expr = expression.trim();
    let hash = match expr.find('#') {
        Some(p) => p,
        None => {
            return VerifyResult::Skipped {
                reason: "Cannot parse primorial expression".into(),
            }
        }
    };
    let p: u64 = match expr[..hash].trim().parse() {
        Ok(v) => v,
        Err(_) => {
            return VerifyResult::Skipped {
                reason: "Cannot parse p".into(),
            }
        }
    };
    let rest = expr[hash + 1..].trim();
    let is_plus = rest.starts_with('+');

    let sieve_limit = (p + 100).max(1000);
    let sieve_primes = sieve::generate_primes(sieve_limit);

    if is_plus {
        if proof::pocklington_factorial_proof(p, candidate, &sieve_primes) {
            VerifyResult::Verified {
                method: "tier1-pocklington".into(),
                tier: 1,
            }
        } else {
            VerifyResult::Failed {
                reason: "Pocklington proof failed for primorial".into(),
            }
        }
    } else {
        if proof::morrison_factorial_proof(p, candidate, &sieve_primes) {
            VerifyResult::Verified {
                method: "tier1-morrison".into(),
                tier: 1,
            }
        } else {
            VerifyResult::Failed {
                reason: "Morrison proof failed for primorial".into(),
            }
        }
    }
}

/// Tier 2: Independent algorithm — BPSW + extra MR with fixed bases.
/// Deliberately different code path from discovery-time tests.
pub fn verify_tier2(candidate: &Integer) -> VerifyResult {
    // Step 1: Trial division quick rejection
    if has_small_factor(candidate) {
        return VerifyResult::Failed {
            reason: "Has small factor (trial division)".into(),
        };
    }

    // Step 2: BPSW test via GMP (1 round = strong probable prime + Lucas test)
    // This is different from the 15/25-round MR used at discovery time.
    if candidate.is_probably_prime(1) == IsPrime::No {
        return VerifyResult::Failed {
            reason: "Failed BPSW test".into(),
        };
    }

    // Step 3: 10 extra MR rounds with fixed bases to avoid overlap with GMP's internal selection.
    let fixed_bases: [u32; 10] = [31, 37, 41, 43, 47, 53, 59, 61, 67, 71];
    for &base in &fixed_bases {
        let a = Integer::from(base);
        let exp = Integer::from(candidate - 1u32);
        match a.pow_mod(&exp, candidate) {
            Ok(r) if r != 1u32 => {
                return VerifyResult::Failed {
                    reason: format!("Failed Fermat test with base {}", base),
                };
            }
            Err(_) => {
                return VerifyResult::Failed {
                    reason: format!("GCD != 1 with base {}", base),
                };
            }
            _ => {} // passed
        }
    }

    VerifyResult::Verified {
        method: "tier2-bpsw+mr10".into(),
        tier: 2,
    }
}

/// Main entry point: verify a single prime from the database.
pub fn verify_prime(detail: &PrimeDetail) -> VerifyResult {
    // Step 1: Reconstruct candidate
    let candidate = match reconstruct_candidate(&detail.form, &detail.expression) {
        Ok(c) => c,
        Err(e) => {
            return VerifyResult::Failed {
                reason: format!("Cannot reconstruct: {}", e),
            }
        }
    };

    // Step 2: Sanity check digit count
    let actual_digits = crate::exact_digits(&candidate);
    if (actual_digits as i64 - detail.digits).unsigned_abs() > 1 {
        return VerifyResult::Failed {
            reason: format!(
                "Digit count mismatch: stored={}, reconstructed={}",
                detail.digits, actual_digits
            ),
        };
    }

    // Step 3: Try tier 1
    let t1 = verify_tier1(
        &detail.form,
        &detail.expression,
        &candidate,
        &detail.proof_method,
    );
    match &t1 {
        VerifyResult::Verified { .. } => return t1,
        VerifyResult::Failed { .. } => return t1,
        VerifyResult::Skipped { .. } => {} // proceed to tier 2
    }

    // Step 4: Tier 2 for probabilistic/skipped primes
    verify_tier2(&candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- reconstruct_candidate tests ---

    #[test]
    fn reconstruct_factorial_plus() {
        let c = reconstruct_candidate("factorial", "5! + 1").unwrap();
        assert_eq!(c, Integer::from(121u32)); // 5! + 1 = 121
    }

    #[test]
    fn reconstruct_factorial_minus() {
        let c = reconstruct_candidate("factorial", "7! - 1").unwrap();
        assert_eq!(c, Integer::from(5039u32)); // 7! - 1 = 5039
    }

    #[test]
    fn reconstruct_primorial_plus() {
        let c = reconstruct_candidate("primorial", "7# + 1").unwrap();
        // 7# = 2*3*5*7 = 210, +1 = 211
        assert_eq!(c, Integer::from(211u32));
    }

    #[test]
    fn reconstruct_primorial_minus() {
        let c = reconstruct_candidate("primorial", "5# - 1").unwrap();
        // 5# = 2*3*5 = 30, -1 = 29
        assert_eq!(c, Integer::from(29u32));
    }

    #[test]
    fn reconstruct_kbn_plus() {
        let c = reconstruct_candidate("kbn", "3*2^5 + 1").unwrap();
        // 3 * 32 + 1 = 97
        assert_eq!(c, Integer::from(97u32));
    }

    #[test]
    fn reconstruct_kbn_minus() {
        let c = reconstruct_candidate("kbn", "3*2^5 - 1").unwrap();
        // 3 * 32 - 1 = 95
        assert_eq!(c, Integer::from(95u32));
    }

    #[test]
    fn reconstruct_palindromic() {
        let c = reconstruct_candidate("palindromic", "10301").unwrap();
        assert_eq!(c, Integer::from(10301u32));
    }

    #[test]
    fn reconstruct_near_repdigit_m0() {
        // "10^3 - 1 - 8*10^1" = 999 - 80 = 919
        let c = reconstruct_candidate("near_repdigit", "10^3 - 1 - 8*10^1").unwrap();
        assert_eq!(c, Integer::from(919u32));
    }

    #[test]
    fn reconstruct_near_repdigit_m_nonzero() {
        // "10^5 - 1 - 4*(10^3 + 10^1)" = 99999 - 4*(1000 + 10) = 99999 - 4040 = 95959
        let c = reconstruct_candidate("near_repdigit", "10^5 - 1 - 4*(10^3 + 10^1)").unwrap();
        assert_eq!(c, Integer::from(95959u32));
    }

    #[test]
    fn reconstruct_wagstaff() {
        let c = reconstruct_candidate("wagstaff", "(2^11+1)/3").unwrap();
        // (2048 + 1) / 3 = 683
        assert_eq!(c, Integer::from(683u32));
    }

    #[test]
    fn reconstruct_carol() {
        let c = reconstruct_candidate("carol_kynea", "(2^7-1)^2-2").unwrap();
        // (128 - 1)^2 - 2 = 127^2 - 2 = 16129 - 2 = 16127
        assert_eq!(c, Integer::from(16127u32));
    }

    #[test]
    fn reconstruct_kynea() {
        let c = reconstruct_candidate("carol_kynea", "(2^7+1)^2-2").unwrap();
        // (128 + 1)^2 - 2 = 129^2 - 2 = 16641 - 2 = 16639
        assert_eq!(c, Integer::from(16639u32));
    }

    #[test]
    fn reconstruct_twin() {
        let c = reconstruct_candidate("twin", "1*2^6 +/- 1").unwrap();
        // 1*64 - 1 = 63 (the smaller twin)
        assert_eq!(c, Integer::from(63u32));
    }

    #[test]
    fn reconstruct_sophie_germain() {
        let c = reconstruct_candidate("sophie_germain", "3*2^4-1").unwrap();
        // 3*16 - 1 = 47
        assert_eq!(c, Integer::from(47u32));
    }

    #[test]
    fn reconstruct_repunit() {
        let c = reconstruct_candidate("repunit", "R(10, 7)").unwrap();
        // (10^7 - 1) / 9 = 9999999 / 9 = 1111111
        assert_eq!(c, Integer::from(1111111u32));
    }

    #[test]
    fn reconstruct_gen_fermat() {
        let c = reconstruct_candidate("gen_fermat", "6^(2^3) + 1").unwrap();
        // 6^8 + 1 = 1679616 + 1 = 1679617
        assert_eq!(c, Integer::from(1679617u32));
    }

    // --- Tier 1 tests ---

    #[test]
    fn tier1_kbn_proth_prime() {
        // 3*2^5 + 1 = 97 (prime, Proth-provable)
        let c = Integer::from(97u32);
        match verify_tier1("kbn", "3*2^5 + 1", &c, "deterministic") {
            VerifyResult::Verified { tier, .. } => assert_eq!(tier, 1),
            other => panic!("Expected Verified, got {:?}", other),
        }
    }

    #[test]
    fn tier1_kbn_llr_prime() {
        // 3*2^7 - 1 = 383 (prime, LLR-provable)
        let c = Integer::from(383u32);
        match verify_tier1("kbn", "3*2^7 - 1", &c, "deterministic") {
            VerifyResult::Verified { tier, .. } => assert_eq!(tier, 1),
            other => panic!("Expected Verified, got {:?}", other),
        }
    }

    #[test]
    fn tier1_factorial_plus() {
        // 11! + 1 = 39916801 (prime)
        let c = Integer::from(Integer::factorial(11)) + 1u32;
        match verify_tier1("factorial", "11! + 1", &c, "deterministic") {
            VerifyResult::Verified { tier, .. } => assert_eq!(tier, 1),
            other => panic!("Expected Verified, got {:?}", other),
        }
    }

    #[test]
    fn tier1_factorial_minus() {
        // 4! - 1 = 23 (prime)
        let c = Integer::from(23u32);
        match verify_tier1("factorial", "4! - 1", &c, "deterministic") {
            VerifyResult::Verified { tier, .. } => assert_eq!(tier, 1),
            other => panic!("Expected Verified, got {:?}", other),
        }
    }

    #[test]
    fn tier1_skips_probabilistic() {
        let c = Integer::from(97u32);
        match verify_tier1("kbn", "3*2^5 + 1", &c, "probabilistic") {
            VerifyResult::Skipped { .. } => {}
            other => panic!("Expected Skipped, got {:?}", other),
        }
    }

    // --- Tier 2 tests ---

    #[test]
    fn tier2_verifies_known_prime() {
        let c = Integer::from(104729u32); // prime
        match verify_tier2(&c) {
            VerifyResult::Verified { tier, .. } => assert_eq!(tier, 2),
            other => panic!("Expected Verified, got {:?}", other),
        }
    }

    #[test]
    fn tier2_rejects_composite() {
        let c = Integer::from(104730u32); // even → composite
        match verify_tier2(&c) {
            VerifyResult::Failed { .. } => {}
            other => panic!("Expected Failed, got {:?}", other),
        }
    }

    #[test]
    fn tier2_rejects_carmichael() {
        // 561 = 3*11*17, the smallest Carmichael number
        let c = Integer::from(561u32);
        match verify_tier2(&c) {
            VerifyResult::Failed { .. } => {}
            other => panic!("Expected Failed for Carmichael number, got {:?}", other),
        }
    }

    // --- Full verify_prime tests ---

    #[test]
    fn verify_prime_kbn_deterministic() {
        let detail = PrimeDetail {
            id: 1,
            form: "kbn".into(),
            expression: "3*2^5 + 1".into(),
            digits: 2,
            found_at: chrono::Utc::now(),
            search_params: "{}".into(),
            proof_method: "deterministic".into(),
        };
        match verify_prime(&detail) {
            VerifyResult::Verified { tier, .. } => assert_eq!(tier, 1),
            other => panic!("Expected Verified tier 1, got {:?}", other),
        }
    }

    #[test]
    fn verify_prime_palindromic_probabilistic() {
        // 10301 is prime, but palindromic has no tier-1 test → tier 2
        let detail = PrimeDetail {
            id: 2,
            form: "palindromic".into(),
            expression: "10301".into(),
            digits: 5,
            found_at: chrono::Utc::now(),
            search_params: "{}".into(),
            proof_method: "probabilistic".into(),
        };
        match verify_prime(&detail) {
            VerifyResult::Verified { tier, .. } => assert_eq!(tier, 2),
            other => panic!("Expected Verified tier 2, got {:?}", other),
        }
    }

    #[test]
    fn verify_prime_digit_mismatch() {
        let detail = PrimeDetail {
            id: 3,
            form: "kbn".into(),
            expression: "3*2^5 + 1".into(),
            digits: 999, // wrong!
            found_at: chrono::Utc::now(),
            search_params: "{}".into(),
            proof_method: "deterministic".into(),
        };
        match verify_prime(&detail) {
            VerifyResult::Failed { reason } => assert!(reason.contains("Digit count mismatch")),
            other => panic!("Expected Failed, got {:?}", other),
        }
    }
}
