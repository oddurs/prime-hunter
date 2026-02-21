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
    //! # Primality Certificate Serialization Tests
    //!
    //! Validates the JSON serialization and deserialization of all 9
    //! `PrimalityCertificate` variants and their associated witness structs.
    //!
    //! Certificates are the exportable proof artifacts that allow independent
    //! verification of primality without re-running the full test. They are
    //! stored as JSONB in the `primes.certificate` database column and served
    //! via `GET /api/primes/:id`. Correct serialization is critical for:
    //!
    //! 1. **Database persistence**: Certificates must survive insert/query cycles
    //!    through PostgreSQL JSONB without data loss.
    //! 2. **API consumption**: External verifiers parse the JSON to reconstruct
    //!    the witness data needed for independent re-verification.
    //! 3. **Type discrimination**: The `#[serde(tag = "type")]` attribute adds a
    //!    `"type"` field to each JSON object, enabling polymorphic deserialization.
    //!
    //! ## Test Strategy
    //!
    //! - **Roundtrip tests**: Serialize to JSON, then deserialize and check equality
    //!   for every variant (Proth, Llr, Pocklington, Morrison, Bls, Pepin,
    //!   MillerRabin, Pfgw, Prst).
    //! - **Edge cases**: u32::MAX/u64::MAX field values, empty witness vectors,
    //!   empty method strings, zero MR rounds.
    //! - **Error handling**: Unknown `"type"` discriminator must fail deserialization.
    //! - **Derive traits**: Clone, Debug, PartialEq verified on all types.
    //!
    //! ## References
    //!
    //! - Serde `#[serde(tag = "type")]`: internally tagged enum representation.
    //! - Each certificate variant corresponds to a proof method documented in
    //!   `src/proof.rs` and `src/verify.rs`.

    use super::*;

    // ── Per-Variant Roundtrip Tests ─────────────────────────────────────

    /// Proth certificate: stores the witness base `a` where a^{(p-1)/2} = -1 (mod p).
    /// Uses base=2, the most common Proth witness. Verifies the JSON contains
    /// the `"type":"Proth"` discriminator tag for polymorphic deserialization.
    #[test]
    fn certificate_json_roundtrip_proth() {
        let cert = PrimalityCertificate::Proth { base: 2 };
        let json = serde_json::to_string(&cert).unwrap();
        assert!(json.contains(r#""type":"Proth""#));
        let decoded: PrimalityCertificate = serde_json::from_str(&json).unwrap();
        assert_eq!(cert, decoded);
    }

    /// LLR certificate: stores (k, n, seed) for the Lucas-Lehmer-Riesel test
    /// of k*2^n - 1. The seed is the initial value u_0 = V_k(P, 1) mod N, stored
    /// as a string because it can be hundreds of digits for large primes.
    /// Uses k=1, n=31, seed="1234" (a mock value for testing serialization).
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

    /// Pocklington N-1 certificate: stores one (factor, base) witness per prime
    /// factor q of N-1. Each witness proves a^{N-1} = 1 (mod N) and
    /// gcd(a^{(N-1)/q} - 1, N) = 1. Uses factors {2, 5} with bases {3, 2},
    /// representing a typical small Pocklington proof with 2 witnesses.
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

    /// Morrison N+1 certificate: stores the Lucas parameter P and one witness per
    /// prime factor q of N+1. Each witness confirms V_{(N+1)/q}(P,1) != 2 (mod N).
    /// Uses p_value=5 and a single witness for factor "3", representing a simple
    /// Morrison proof where N+1 has few prime factors.
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

    /// BLS N+1 certificate: Morrison-style witnesses plus factored_bits and
    /// total_bits fields that prove the BLS >= 1/3 factored threshold. Here
    /// factored_bits=12 > total_bits=10, indicating > 100% factorization (this
    /// is a serialization test, not a mathematical validity test). Uses two
    /// MorrisonWitness entries for factors {2, 5}.
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

    // ── Comprehensive Roundtrip Test ───────────────────────────────────

    /// Exhaustive roundtrip test covering all 9 certificate variants in a single
    /// loop. Each variant is serialized to JSON and deserialized back, verifying
    /// exact equality via the derived PartialEq. This catches any variant that
    /// might have been added to the enum but forgotten in the serde configuration.
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

    // ── Multi-Witness Certificates ─────────────────────────────────────

    /// Pocklington certificate with 5 witness entries, representing a proof
    /// for a prime where N-1 has 5 distinct prime factors: {2, 3, 5, 7, 11}.
    /// Each factor requires its own witness base. Verifies that the Vec<PocklingtonWitness>
    /// serializes as a JSON array and deserializes with correct length.
    #[test]
    fn certificate_pocklington_many_witnesses() {
        let cert = PrimalityCertificate::Pocklington {
            factors: vec![
                PocklingtonWitness { factor: "2".to_string(), base: 3 },
                PocklingtonWitness { factor: "3".to_string(), base: 2 },
                PocklingtonWitness { factor: "5".to_string(), base: 2 },
                PocklingtonWitness { factor: "7".to_string(), base: 3 },
                PocklingtonWitness { factor: "11".to_string(), base: 2 },
            ],
        };
        let json = serde_json::to_string(&cert).unwrap();
        let decoded: PrimalityCertificate = serde_json::from_str(&json).unwrap();
        assert_eq!(cert, decoded);
        if let PrimalityCertificate::Pocklington { factors } = decoded {
            assert_eq!(factors.len(), 5);
        }
    }

    /// Morrison certificate with 3 witness entries for factors {2, 3, 5} of N+1,
    /// all using the same Lucas parameter P=11. The p_value field appears both in
    /// the top-level Morrison variant and in each MorrisonWitness (they should match
    /// but the serialization format allows them to differ for flexibility).
    #[test]
    fn certificate_morrison_many_witnesses() {
        let cert = PrimalityCertificate::Morrison {
            p_value: 11,
            factors: vec![
                MorrisonWitness { factor: "2".to_string(), p_value: 11 },
                MorrisonWitness { factor: "3".to_string(), p_value: 11 },
                MorrisonWitness { factor: "5".to_string(), p_value: 11 },
            ],
        };
        let json = serde_json::to_string(&cert).unwrap();
        let decoded: PrimalityCertificate = serde_json::from_str(&json).unwrap();
        assert_eq!(cert, decoded);
    }

    /// Verifies that the BLS threshold fields (factored_bits, total_bits) survive
    /// serialization as exact integer values in JSON. These fields must be precise:
    /// factored_bits/total_bits >= 1/3 is the mathematical condition that makes the
    /// BLS proof valid. Here factored_bits=100, total_bits=200, giving a 50% ratio
    /// which satisfies the >= 33.3% threshold.
    #[test]
    fn certificate_bls_threshold_data() {
        let cert = PrimalityCertificate::Bls {
            p_value: 5,
            factors: vec![
                MorrisonWitness { factor: "2".to_string(), p_value: 5 },
            ],
            factored_bits: 100,
            total_bits: 200,
        };
        let json = serde_json::to_string(&cert).unwrap();
        assert!(json.contains("\"factored_bits\":100"));
        assert!(json.contains("\"total_bits\":200"));
        let decoded: PrimalityCertificate = serde_json::from_str(&json).unwrap();
        assert_eq!(cert, decoded);
    }

    // ── Edge Cases and Boundary Values ─────────────────────────────────

    /// LLR certificate with u64::MAX for both k and n fields, and a 30-digit seed
    /// string. Tests that serde correctly handles the maximum u64 value
    /// (18446744073709551615) in JSON, which exceeds JavaScript's Number.MAX_SAFE_INTEGER
    /// but is valid in serde_json's u64 handling. The seed is stored as a string
    /// precisely to avoid this integer overflow issue.
    #[test]
    fn certificate_llr_large_values() {
        let cert = PrimalityCertificate::Llr {
            k: u64::MAX,
            n: u64::MAX,
            seed: "999999999999999999999999999999".to_string(),
        };
        let json = serde_json::to_string(&cert).unwrap();
        let decoded: PrimalityCertificate = serde_json::from_str(&json).unwrap();
        assert_eq!(cert, decoded);
    }

    /// Proth certificate with base = u32::MAX (4294967295). While no real Proth
    /// test would use a witness this large (small bases like 2, 3, 5 are tried
    /// first), the serialization must handle the full u32 range without truncation.
    #[test]
    fn certificate_proth_large_base() {
        let cert = PrimalityCertificate::Proth { base: u32::MAX };
        let json = serde_json::to_string(&cert).unwrap();
        let decoded: PrimalityCertificate = serde_json::from_str(&json).unwrap();
        assert_eq!(cert, decoded);
    }

    /// MillerRabin certificate with 0 rounds: a degenerate case that should never
    /// occur in practice (0 rounds means no testing was performed). The serialization
    /// must still handle it gracefully rather than panicking, because external
    /// tools might produce this value.
    #[test]
    fn certificate_miller_rabin_zero_rounds() {
        // Edge case: 0 rounds (degenerate but should still serialize)
        let cert = PrimalityCertificate::MillerRabin { rounds: 0 };
        let json = serde_json::to_string(&cert).unwrap();
        let decoded: PrimalityCertificate = serde_json::from_str(&json).unwrap();
        assert_eq!(cert, decoded);
    }

    // ── Error Handling ────────────────────────────────────────────────

    /// An unknown `"type"` discriminator must fail deserialization. This is
    /// critical for forward compatibility: if a newer version of the software
    /// introduces a new certificate type, older versions must reject it with
    /// a clear error rather than silently dropping data. The serde `tag = "type"`
    /// attribute enforces this via enum variant matching.
    #[test]
    fn certificate_unknown_type_fails_deserialization() {
        let json = r#"{"type":"Unknown","data":"something"}"#;
        let result: Result<PrimalityCertificate, _> = serde_json::from_str(json);
        assert!(result.is_err(), "Deserializing unknown type should fail");
    }

    // ── Derive Trait Verification ──────────────────────────────────────

    /// Verifies the derived Clone, Debug, and PartialEq traits on PrimalityCertificate.
    /// Clone is needed for certificate duplication in the verification pipeline.
    /// Debug is needed for error messages and logging. PartialEq is needed for
    /// test assertions and deduplication.
    #[test]
    fn certificate_clone_and_debug() {
        let cert = PrimalityCertificate::Proth { base: 42 };
        let cloned = cert.clone();
        assert_eq!(cert, cloned);
        let debug = format!("{:?}", cert);
        assert!(debug.contains("Proth"));
        assert!(debug.contains("42"));
    }

    /// Verifies Clone, Debug, PartialEq on PocklingtonWitness. The Debug output
    /// must contain "PocklingtonWitness" for diagnostic logging, and Clone is
    /// needed because witnesses are stored in Vec and may be copied during
    /// certificate construction.
    #[test]
    fn pocklington_witness_clone_and_debug() {
        let w = PocklingtonWitness { factor: "2".to_string(), base: 3 };
        let cloned = w.clone();
        assert_eq!(w, cloned);
        let debug = format!("{:?}", w);
        assert!(debug.contains("PocklingtonWitness"));
    }

    /// Verifies Clone, Debug, PartialEq on MorrisonWitness. Uses factor "7"
    /// with p_value 13 -- a typical witness for a Morrison N+1 proof where 7
    /// divides N+1 and the Lucas parameter P=13 was used.
    #[test]
    fn morrison_witness_clone_and_debug() {
        let w = MorrisonWitness { factor: "7".to_string(), p_value: 13 };
        let cloned = w.clone();
        assert_eq!(w, cloned);
        let debug = format!("{:?}", w);
        assert!(debug.contains("MorrisonWitness"));
    }

    // ── External Tool Certificates ─────────────────────────────────────

    /// PFGW certificate with an empty method string. While real PFGW certificates
    /// have method strings like "PRP" or "3-PRP", an empty string is a valid
    /// edge case that can occur if PFGW output parsing fails to extract the method.
    /// The serialization must handle it without error.
    #[test]
    fn certificate_pfgw_empty_method() {
        let cert = PrimalityCertificate::Pfgw { method: String::new() };
        let json = serde_json::to_string(&cert).unwrap();
        let decoded: PrimalityCertificate = serde_json::from_str(&json).unwrap();
        assert_eq!(cert, decoded);
    }

    /// Pepin certificate for generalized Fermat numbers b^{2^n} + 1: stores the
    /// base used in the Pepin test a^{(N-1)/2} = -1 (mod N). Uses base=3, the
    /// standard Pepin base for classical Fermat numbers F_n = 2^{2^n} + 1.
    /// Verifies the `"type":"Pepin"` discriminator tag.
    #[test]
    fn certificate_pepin_roundtrip() {
        let cert = PrimalityCertificate::Pepin { base: 3 };
        let json = serde_json::to_string(&cert).unwrap();
        assert!(json.contains(r#""type":"Pepin""#));
        let decoded: PrimalityCertificate = serde_json::from_str(&json).unwrap();
        assert_eq!(cert, decoded);
    }

    // ── Type Discriminator Validation ──────────────────────────────────

    /// Verifies that the `#[serde(tag = "type")]` attribute produces a `"type"`
    /// field in every certificate variant's JSON. This is essential for
    /// polymorphic deserialization: the consumer reads the `"type"` field first
    /// to determine which variant to instantiate. Tests 3 representative variants
    /// (Proth, Pepin, MillerRabin) and parses JSON as a generic Value to check
    /// for the `"type"` key's existence.
    #[test]
    fn certificate_json_contains_type_discriminator() {
        // Every variant should have a "type" field in the JSON
        let variants: Vec<PrimalityCertificate> = vec![
            PrimalityCertificate::Proth { base: 2 },
            PrimalityCertificate::Pepin { base: 3 },
            PrimalityCertificate::MillerRabin { rounds: 25 },
        ];
        for cert in variants {
            let json = serde_json::to_string(&cert).unwrap();
            let v: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert!(
                v.get("type").is_some(),
                "Certificate JSON should contain 'type' field: {}",
                json
            );
        }
    }
}
