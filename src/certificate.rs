//! # Certificate — Exportable Primality Certificates
//!
//! Contains witness data sufficient for independent verification of primality
//! without re-running expensive tests. Each certificate variant captures the
//! specific witness data produced by the corresponding proof method.
//!
//! ## Certificate Types
//!
//! - **Proth**: The witness base `a` where `a^((p-1)/2) ≡ -1 (mod p)`.
//! - **Llr**: The LLR initial seed value `u_0 = V_k(P, 1) mod N`.
//! - **Pocklington**: For each prime factor `q` of `N-1`, a witness base `a`
//!   where `a^(N-1) ≡ 1 (mod N)` and `gcd(a^((N-1)/q) - 1, N) = 1`.
//! - **Morrison**: A Lucas `P` value and per-factor witnesses where
//!   `V_{(N+1)/q}(P,1) ≢ 2 (mod N)`.
//! - **Bls**: Morrison-style witnesses with factored/total bit counts for the
//!   BLS ≥ 1/3 threshold check.
//! - **Pepin**: The base used in `a^((N-1)/2) ≡ -1 (mod N)` for generalized
//!   Fermat numbers.
//! - **MillerRabin**: Round count only (probabilistic, no deterministic witness).
//! - **Pfgw** / **Prst**: Method string from external tool verification.
//!
//! ## Serialization
//!
//! All types derive `serde::Serialize` and `serde::Deserialize`, using
//! `#[serde(tag = "type")]` for the top-level enum so JSON includes a `"type"`
//! discriminator field.
//!
//! ## References
//!
//! - François Proth, "Théorèmes sur les nombres premiers", 1878.
//! - H.C. Pocklington, "The Determination of the Prime or Composite Nature
//!   of Large Numbers by Fermat's Theorem", 1914.
//! - M.A. Morrison, "A Note on Primality Testing Using Lucas Sequences", 1975.
//! - Brillhart, Lehmer, Selfridge, "New Primality Criteria and Factorizations
//!   of 2^m ± 1", 1975.

use serde::{Deserialize, Serialize};

/// Exportable primality certificate containing witness data sufficient
/// for independent verification without re-running the full test.
///
/// Stored as JSONB in the `primes.certificate` column and exposed via
/// `GET /api/primes/:id`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum PrimalityCertificate {
    /// Proth test: `a^((p-1)/2) ≡ -1 (mod p)` for witness base `a`.
    Proth { base: u32 },

    /// LLR test: Lucas–Lehmer–Riesel with seed `u_0 = V_k(P, 1) mod N`.
    Llr { k: u64, n: u64, seed: String },

    /// Pocklington N−1 proof: for each prime factor `q` of `N-1`,
    /// a witness base `a` where `a^(N-1) ≡ 1` and `gcd(a^((N-1)/q) - 1, N) = 1`.
    Pocklington { factors: Vec<PocklingtonWitness> },

    /// Morrison N+1 proof: Lucas `P` value and per-factor witnesses where
    /// `V_{(N+1)/q}(P,1) ≢ 2 (mod N)`.
    Morrison {
        p_value: u32,
        factors: Vec<MorrisonWitness>,
    },

    /// BLS N+1 proof for near-repdigit palindromes: Morrison-style witnesses
    /// with factored/total bit counts proving the ≥ 1/3 factored threshold.
    Bls {
        p_value: u32,
        factors: Vec<MorrisonWitness>,
        factored_bits: u32,
        total_bits: u32,
    },

    /// Pépin test for generalized Fermat numbers: `base^((N-1)/2) ≡ -1 (mod N)`.
    Pepin { base: u32 },

    /// Probabilistic (Miller–Rabin only, no deterministic proof).
    MillerRabin { rounds: u32 },

    /// PFGW external verification.
    Pfgw { method: String },

    /// PRST external verification.
    Prst { method: String },
}

/// Witness for one prime factor in a Pocklington N−1 proof.
///
/// For prime factor `q` of `N-1`, the witness base `a` satisfies:
/// - `a^(N-1) ≡ 1 (mod N)`
/// - `gcd(a^((N-1)/q) - 1, N) = 1`
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PocklingtonWitness {
    pub factor: String,
    pub base: u32,
}

/// Witness for one prime factor in a Morrison N+1 or BLS proof.
///
/// For prime factor `q` of `N+1` and Lucas parameter `P`, the witness confirms:
/// - `V_{N+1}(P, 1) ≡ 2 (mod N)`
/// - `gcd(V_{(N+1)/q}(P, 1) - 2, N) = 1`
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MorrisonWitness {
    pub factor: String,
    pub p_value: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn certificate_json_roundtrip_proth() {
        let cert = PrimalityCertificate::Proth { base: 2 };
        let json = serde_json::to_string(&cert).unwrap();
        assert!(json.contains(r#""type":"Proth""#));
        let decoded: PrimalityCertificate = serde_json::from_str(&json).unwrap();
        assert_eq!(cert, decoded);
    }

    #[test]
    fn certificate_json_roundtrip_llr() {
        let cert = PrimalityCertificate::Llr {
            k: 1,
            n: 31,
            seed: "1234".to_string(),
        };
        let json = serde_json::to_string(&cert).unwrap();
        assert!(json.contains(r#""type":"Llr""#));
        let decoded: PrimalityCertificate = serde_json::from_str(&json).unwrap();
        assert_eq!(cert, decoded);
    }

    #[test]
    fn certificate_json_roundtrip_pocklington() {
        let cert = PrimalityCertificate::Pocklington {
            factors: vec![
                PocklingtonWitness {
                    factor: "2".to_string(),
                    base: 3,
                },
                PocklingtonWitness {
                    factor: "5".to_string(),
                    base: 2,
                },
            ],
        };
        let json = serde_json::to_string(&cert).unwrap();
        assert!(json.contains(r#""type":"Pocklington""#));
        let decoded: PrimalityCertificate = serde_json::from_str(&json).unwrap();
        assert_eq!(cert, decoded);
    }

    #[test]
    fn certificate_json_roundtrip_morrison() {
        let cert = PrimalityCertificate::Morrison {
            p_value: 5,
            factors: vec![MorrisonWitness {
                factor: "3".to_string(),
                p_value: 5,
            }],
        };
        let json = serde_json::to_string(&cert).unwrap();
        assert!(json.contains(r#""type":"Morrison""#));
        let decoded: PrimalityCertificate = serde_json::from_str(&json).unwrap();
        assert_eq!(cert, decoded);
    }

    #[test]
    fn certificate_json_roundtrip_bls() {
        let cert = PrimalityCertificate::Bls {
            p_value: 7,
            factors: vec![
                MorrisonWitness {
                    factor: "2".to_string(),
                    p_value: 7,
                },
                MorrisonWitness {
                    factor: "5".to_string(),
                    p_value: 7,
                },
            ],
            factored_bits: 12,
            total_bits: 10,
        };
        let json = serde_json::to_string(&cert).unwrap();
        assert!(json.contains(r#""type":"Bls""#));
        let decoded: PrimalityCertificate = serde_json::from_str(&json).unwrap();
        assert_eq!(cert, decoded);
    }

    #[test]
    fn certificate_json_roundtrip_all_variants() {
        let variants = vec![
            PrimalityCertificate::Proth { base: 2 },
            PrimalityCertificate::Llr {
                k: 1,
                n: 13,
                seed: "4".to_string(),
            },
            PrimalityCertificate::Pocklington { factors: vec![] },
            PrimalityCertificate::Morrison {
                p_value: 3,
                factors: vec![],
            },
            PrimalityCertificate::Bls {
                p_value: 3,
                factors: vec![],
                factored_bits: 10,
                total_bits: 30,
            },
            PrimalityCertificate::Pepin { base: 3 },
            PrimalityCertificate::MillerRabin { rounds: 25 },
            PrimalityCertificate::Pfgw {
                method: "PRP".to_string(),
            },
            PrimalityCertificate::Prst {
                method: "k=1*2^31-1".to_string(),
            },
        ];

        for cert in variants {
            let json = serde_json::to_string(&cert).unwrap();
            let decoded: PrimalityCertificate = serde_json::from_str(&json).unwrap();
            assert_eq!(cert, decoded, "Roundtrip failed for {:?}", cert);
        }
    }
}
