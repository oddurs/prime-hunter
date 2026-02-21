//! # SearchParams — Typed Search Configuration for All Prime Forms
//!
//! Defines the `SearchParams` enum that represents typed parameter variants
//! for every supported prime form. Serialized as tagged JSON for the REST API
//! and stored in `search_jobs.params` in PostgreSQL.
//!
//! This module was extracted from `search_manager.rs` during the Phase 1
//! architecture migration (coordinator removal). `SearchParams` is used by:
//! - `routes_searches.rs` — creating search jobs in PG
//! - `routes_jobs.rs` — search job parameter parsing
//! - `deploy.rs` — SSH deployment command building (feature-gated)

use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "search_type")]
pub enum SearchParams {
    #[serde(rename = "factorial")]
    Factorial { start: u64, end: u64 },
    #[serde(rename = "palindromic")]
    Palindromic {
        base: u32,
        min_digits: u64,
        max_digits: u64,
    },
    #[serde(rename = "kbn")]
    Kbn {
        k: u64,
        base: u32,
        min_n: u64,
        max_n: u64,
    },
    #[serde(rename = "primorial")]
    Primorial { start: u64, end: u64 },
    #[serde(rename = "cullen_woodall")]
    CullenWoodall { min_n: u64, max_n: u64 },
    #[serde(rename = "wagstaff")]
    Wagstaff { min_exp: u64, max_exp: u64 },
    #[serde(rename = "carol_kynea")]
    CarolKynea { min_n: u64, max_n: u64 },
    #[serde(rename = "twin")]
    Twin {
        k: u64,
        base: u32,
        min_n: u64,
        max_n: u64,
    },
    #[serde(rename = "sophie_germain")]
    SophieGermain {
        k: u64,
        base: u32,
        min_n: u64,
        max_n: u64,
    },
    #[serde(rename = "repunit")]
    Repunit { base: u32, min_n: u64, max_n: u64 },
    #[serde(rename = "gen_fermat")]
    GenFermat {
        fermat_exp: u32,
        min_base: u64,
        max_base: u64,
    },
}

impl SearchParams {
    pub fn search_type_name(&self) -> &'static str {
        match self {
            SearchParams::Factorial { .. } => "factorial",
            SearchParams::Palindromic { .. } => "palindromic",
            SearchParams::Kbn { .. } => "kbn",
            SearchParams::Primorial { .. } => "primorial",
            SearchParams::CullenWoodall { .. } => "cullen_woodall",
            SearchParams::Wagstaff { .. } => "wagstaff",
            SearchParams::CarolKynea { .. } => "carol_kynea",
            SearchParams::Twin { .. } => "twin",
            SearchParams::SophieGermain { .. } => "sophie_germain",
            SearchParams::Repunit { .. } => "repunit",
            SearchParams::GenFermat { .. } => "gen_fermat",
        }
    }

    pub fn to_args(&self) -> Vec<String> {
        match self {
            SearchParams::Factorial { start, end } => {
                vec![
                    "factorial".into(),
                    "--start".into(),
                    start.to_string(),
                    "--end".into(),
                    end.to_string(),
                ]
            }
            SearchParams::Palindromic {
                base,
                min_digits,
                max_digits,
            } => {
                vec![
                    "palindromic".into(),
                    "--base".into(),
                    base.to_string(),
                    "--min-digits".into(),
                    min_digits.to_string(),
                    "--max-digits".into(),
                    max_digits.to_string(),
                ]
            }
            SearchParams::Kbn {
                k,
                base,
                min_n,
                max_n,
            } => {
                vec![
                    "kbn".into(),
                    "--k".into(),
                    k.to_string(),
                    "--base".into(),
                    base.to_string(),
                    "--min-n".into(),
                    min_n.to_string(),
                    "--max-n".into(),
                    max_n.to_string(),
                ]
            }
            SearchParams::Primorial { start, end } => {
                vec![
                    "primorial".into(),
                    "--start".into(),
                    start.to_string(),
                    "--end".into(),
                    end.to_string(),
                ]
            }
            SearchParams::CullenWoodall { min_n, max_n } => {
                vec![
                    "cullen-woodall".into(),
                    "--min-n".into(),
                    min_n.to_string(),
                    "--max-n".into(),
                    max_n.to_string(),
                ]
            }
            SearchParams::Wagstaff { min_exp, max_exp } => {
                vec![
                    "wagstaff".into(),
                    "--min-exp".into(),
                    min_exp.to_string(),
                    "--max-exp".into(),
                    max_exp.to_string(),
                ]
            }
            SearchParams::CarolKynea { min_n, max_n } => {
                vec![
                    "carol-kynea".into(),
                    "--min-n".into(),
                    min_n.to_string(),
                    "--max-n".into(),
                    max_n.to_string(),
                ]
            }
            SearchParams::Twin {
                k,
                base,
                min_n,
                max_n,
            } => {
                vec![
                    "twin".into(),
                    "--k".into(),
                    k.to_string(),
                    "--base".into(),
                    base.to_string(),
                    "--min-n".into(),
                    min_n.to_string(),
                    "--max-n".into(),
                    max_n.to_string(),
                ]
            }
            SearchParams::SophieGermain {
                k,
                base,
                min_n,
                max_n,
            } => {
                vec![
                    "sophie-germain".into(),
                    "--k".into(),
                    k.to_string(),
                    "--base".into(),
                    base.to_string(),
                    "--min-n".into(),
                    min_n.to_string(),
                    "--max-n".into(),
                    max_n.to_string(),
                ]
            }
            SearchParams::Repunit { base, min_n, max_n } => {
                vec![
                    "repunit".into(),
                    "--base".into(),
                    base.to_string(),
                    "--min-n".into(),
                    min_n.to_string(),
                    "--max-n".into(),
                    max_n.to_string(),
                ]
            }
            SearchParams::GenFermat {
                fermat_exp,
                min_base,
                max_base,
            } => {
                vec![
                    "gen-fermat".into(),
                    "--fermat-exp".into(),
                    fermat_exp.to_string(),
                    "--min-base".into(),
                    min_base.to_string(),
                    "--max-base".into(),
                    max_base.to_string(),
                ]
            }
        }
    }

    /// Compute the range for this search form (range_start, range_end) for work block generation.
    pub fn range(&self) -> (i64, i64) {
        match self {
            SearchParams::Factorial { start, end } => (*start as i64, *end as i64),
            SearchParams::Palindromic {
                min_digits,
                max_digits,
                ..
            } => (*min_digits as i64, *max_digits as i64),
            SearchParams::Kbn { min_n, max_n, .. } => (*min_n as i64, *max_n as i64),
            SearchParams::Primorial { start, end } => (*start as i64, *end as i64),
            SearchParams::CullenWoodall { min_n, max_n } => (*min_n as i64, *max_n as i64),
            SearchParams::Wagstaff { min_exp, max_exp } => (*min_exp as i64, *max_exp as i64),
            SearchParams::CarolKynea { min_n, max_n } => (*min_n as i64, *max_n as i64),
            SearchParams::Twin { min_n, max_n, .. } => (*min_n as i64, *max_n as i64),
            SearchParams::SophieGermain { min_n, max_n, .. } => (*min_n as i64, *max_n as i64),
            SearchParams::Repunit { min_n, max_n, .. } => (*min_n as i64, *max_n as i64),
            SearchParams::GenFermat {
                min_base, max_base, ..
            } => (*min_base as i64, *max_base as i64),
        }
    }

    /// Default block size for this search form.
    pub fn default_block_size(&self) -> i64 {
        match self {
            SearchParams::Factorial { .. } => 100,
            SearchParams::Palindromic { .. } => 2,
            SearchParams::Kbn { .. } => 10_000,
            SearchParams::Primorial { .. } => 100,
            SearchParams::CullenWoodall { .. } => 1000,
            SearchParams::Wagstaff { .. } => 1000,
            SearchParams::CarolKynea { .. } => 1000,
            SearchParams::Twin { .. } => 10_000,
            SearchParams::SophieGermain { .. } => 10_000,
            SearchParams::Repunit { .. } => 1000,
            SearchParams::GenFermat { .. } => 1000,
        }
    }
}

#[cfg(test)]
mod tests {
    //! Tests for SearchParams — typed search configuration for all 11 prime forms.
    //!
    //! Validates JSON serialization/deserialization round-trips, CLI argument
    //! generation, range extraction for work block generation, block size
    //! defaults, and the serde tag consistency between search_type_name() and
    //! the JSON "search_type" field.
    //!
    //! ## Testing Strategy
    //!
    //! - **Serialization**: round-trip all 11 variants through JSON, verify tag matches
    //! - **CLI args**: verify subcommand name and --flag value pair structure
    //! - **Range**: verify range extraction returns correct (start, end) for each form
    //! - **Block size**: verify positive default block sizes for work distribution
    //! - **Edge cases**: unknown search_type rejection, clone equality

    use super::*;

    /// Helper: constructs one instance of every SearchParams variant with
    /// representative parameter values for exhaustive testing.
    fn all_variants() -> Vec<SearchParams> {
        vec![
            SearchParams::Factorial { start: 1, end: 100 },
            SearchParams::Palindromic {
                base: 10,
                min_digits: 1,
                max_digits: 9,
            },
            SearchParams::Kbn {
                k: 3,
                base: 2,
                min_n: 1,
                max_n: 1000,
            },
            SearchParams::Primorial { start: 2, end: 100 },
            SearchParams::CullenWoodall {
                min_n: 1,
                max_n: 100,
            },
            SearchParams::Wagstaff {
                min_exp: 3,
                max_exp: 100,
            },
            SearchParams::CarolKynea {
                min_n: 1,
                max_n: 100,
            },
            SearchParams::Twin {
                k: 3,
                base: 2,
                min_n: 1,
                max_n: 1000,
            },
            SearchParams::SophieGermain {
                k: 1,
                base: 2,
                min_n: 2,
                max_n: 100,
            },
            SearchParams::Repunit {
                base: 10,
                min_n: 2,
                max_n: 50,
            },
            SearchParams::GenFermat {
                fermat_exp: 1,
                min_base: 2,
                max_base: 100,
            },
        ]
    }

    // ── Serialization ───────────────────────────────────────────

    /// Every SearchParams variant must survive a JSON round-trip without data
    /// loss. This is critical because params are stored as JSON in the
    /// search_jobs.params column in PostgreSQL.
    #[test]
    fn search_params_serde_roundtrip_all_variants() {
        for params in all_variants() {
            let json = serde_json::to_string(&params).unwrap();
            let parsed: SearchParams = serde_json::from_str(&json).unwrap();
            let json2 = serde_json::to_string(&parsed).unwrap();
            assert_eq!(json, json2, "Serde roundtrip failed for: {}", json);
        }
    }

    /// The search_type_name() method must return exactly the same string
    /// as the serde tag in serialized JSON. The search manager uses both
    /// interchangeably for job type identification.
    #[test]
    fn search_type_name_matches_serde_tag() {
        let expected_names = [
            "factorial",
            "palindromic",
            "kbn",
            "primorial",
            "cullen_woodall",
            "wagstaff",
            "carol_kynea",
            "twin",
            "sophie_germain",
            "repunit",
            "gen_fermat",
        ];
        for (params, expected) in all_variants().iter().zip(expected_names.iter()) {
            assert_eq!(params.search_type_name(), *expected);
            let json = serde_json::to_string(params).unwrap();
            assert!(json.contains(&format!("\"search_type\":\"{}\"", expected)));
        }
    }

    // ── CLI Argument Generation ──────────────────────────────────

    /// The first element of to_args() must be the subcommand name (no leading
    /// dash). The deploy module passes these args directly to SSH commands.
    #[test]
    fn to_args_first_element_is_subcommand() {
        for params in all_variants() {
            let args = params.to_args();
            assert!(!args.is_empty());
            assert!(!args[0].starts_with('-'));
        }
    }

    // ── Range Extraction ─────────────────────────────────────────

    /// Validates that range() returns the correct (start, end) tuple for work
    /// block generation. Each form extracts its primary iteration variable.
    #[test]
    fn range_returns_valid_bounds() {
        let p = SearchParams::Factorial { start: 10, end: 200 };
        assert_eq!(p.range(), (10, 200));
        let p = SearchParams::Kbn { k: 3, base: 2, min_n: 100, max_n: 5000 };
        assert_eq!(p.range(), (100, 5000));
    }

    // ── Block Size ──────────────────────────────────────────────

    /// Every form must have a positive default block size. Zero or negative
    /// would cause division errors in the work block generator.
    #[test]
    fn default_block_size_positive() {
        for params in all_variants() {
            assert!(params.default_block_size() > 0);
        }
    }

    /// Validates the exact CLI argument sequence for factorial searches.
    #[test]
    fn to_args_factorial_format() {
        let p = SearchParams::Factorial { start: 10, end: 200 };
        let args = p.to_args();
        assert_eq!(args[0], "factorial");
        assert_eq!(args[1], "--start");
        assert_eq!(args[2], "10");
        assert_eq!(args[3], "--end");
        assert_eq!(args[4], "200");
    }

    /// Validates the exact CLI argument sequence for k*b^n searches,
    /// which have 4 parameters (k, base, min_n, max_n).
    #[test]
    fn to_args_kbn_format() {
        let p = SearchParams::Kbn { k: 3, base: 2, min_n: 100, max_n: 5000 };
        let args = p.to_args();
        assert_eq!(args[0], "kbn");
        assert_eq!(args[1], "--k");
        assert_eq!(args[2], "3");
        assert_eq!(args[3], "--base");
        assert_eq!(args[4], "2");
        assert_eq!(args[5], "--min-n");
        assert_eq!(args[6], "100");
        assert_eq!(args[7], "--max-n");
        assert_eq!(args[8], "5000");
    }

    // ── Hyphenated Subcommand Names ──────────────────────────────
    // Multi-word form names use underscores in Rust (CullenWoodall) but
    // hyphens in CLI subcommands (cullen-woodall). These tests verify
    // the conversion is correct for clap argument parsing.

    /// CullenWoodall must generate "cullen-woodall" (hyphenated) as the CLI
    /// subcommand, not "cullen_woodall" (underscored serde tag).
    #[test]
    fn to_args_cullen_woodall_uses_hyphenated_subcommand() {
        let p = SearchParams::CullenWoodall { min_n: 1, max_n: 100 };
        let args = p.to_args();
        assert_eq!(args[0], "cullen-woodall", "CLI subcommand should be hyphenated");
    }

    /// CarolKynea must generate "carol-kynea" as the CLI subcommand.
    #[test]
    fn to_args_carol_kynea_uses_hyphenated_subcommand() {
        let p = SearchParams::CarolKynea { min_n: 1, max_n: 100 };
        let args = p.to_args();
        assert_eq!(args[0], "carol-kynea");
    }

    /// SophieGermain must generate "sophie-germain" as the CLI subcommand.
    #[test]
    fn to_args_sophie_germain_uses_hyphenated_subcommand() {
        let p = SearchParams::SophieGermain { k: 1, base: 2, min_n: 2, max_n: 100 };
        let args = p.to_args();
        assert_eq!(args[0], "sophie-germain");
    }

    /// GenFermat must generate "gen-fermat" and "--fermat-exp" (hyphenated).
    #[test]
    fn to_args_gen_fermat_uses_hyphenated_subcommand() {
        let p = SearchParams::GenFermat { fermat_exp: 3, min_base: 2, max_base: 1000 };
        let args = p.to_args();
        assert_eq!(args[0], "gen-fermat");
        assert_eq!(args[1], "--fermat-exp");
        assert_eq!(args[2], "3");
    }

    /// For all test variants, range start must be <= end. Inverted ranges
    /// would cause the work block generator to produce no blocks.
    #[test]
    fn range_all_variants_start_le_end() {
        for p in all_variants() {
            let (start, end) = p.range();
            assert!(start <= end, "range start > end for {:?}", p.search_type_name());
        }
    }

    // ── Form-Specific Range Extraction ──────────────────────────

    /// Palindromic range uses digit count (not some internal index).
    /// Work blocks iterate over digit counts, not candidate values.
    #[test]
    fn range_palindromic_uses_digits() {
        let p = SearchParams::Palindromic { base: 10, min_digits: 3, max_digits: 99 };
        assert_eq!(p.range(), (3, 99));
    }

    /// GenFermat range uses base values (min_base, max_base), not the
    /// Fermat exponent. Work blocks iterate over candidate bases.
    #[test]
    fn range_gen_fermat_uses_base() {
        let p = SearchParams::GenFermat { fermat_exp: 2, min_base: 10, max_base: 10000 };
        assert_eq!(p.range(), (10, 10000));
    }

    /// Wagstaff range uses exponent bounds, not the Wagstaff number values.
    #[test]
    fn range_wagstaff_uses_exp() {
        let p = SearchParams::Wagstaff { min_exp: 3, max_exp: 500 };
        assert_eq!(p.range(), (3, 500));
    }

    // ── Edge Cases ──────────────────────────────────────────────

    /// Clone must produce an identical copy. SearchParams is cloned when
    /// stored in both the search job and individual work blocks.
    #[test]
    fn clone_preserves_values() {
        let p = SearchParams::Kbn { k: 7, base: 3, min_n: 50, max_n: 999 };
        let cloned = p.clone();
        let json1 = serde_json::to_string(&p).unwrap();
        let json2 = serde_json::to_string(&cloned).unwrap();
        assert_eq!(json1, json2);
    }

    /// Unknown search types must fail deserialization rather than silently
    /// creating a default. This prevents the coordinator from accepting
    /// malformed job parameters.
    #[test]
    fn deserialize_unknown_search_type_fails() {
        let json = r#"{"search_type":"nonexistent","foo":42}"#;
        let result: Result<SearchParams, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    /// Validates specific block sizes: factorial=100 (1 minute per block),
    /// palindromic=2 (2 digit counts per block), kbn=10000 (exponent range).
    #[test]
    fn default_block_size_specific_values() {
        assert_eq!(SearchParams::Factorial { start: 1, end: 100 }.default_block_size(), 100);
        assert_eq!(SearchParams::Palindromic { base: 10, min_digits: 1, max_digits: 9 }.default_block_size(), 2);
        assert_eq!(SearchParams::Kbn { k: 3, base: 2, min_n: 1, max_n: 1000 }.default_block_size(), 10_000);
    }

    /// Validates the exact CLI argument sequence for repunit searches.
    #[test]
    fn to_args_repunit_format() {
        let p = SearchParams::Repunit { base: 10, min_n: 2, max_n: 50 };
        let args = p.to_args();
        assert_eq!(args[0], "repunit");
        assert_eq!(args[1], "--base");
        assert_eq!(args[2], "10");
        assert_eq!(args[3], "--min-n");
        assert_eq!(args[4], "2");
        assert_eq!(args[5], "--max-n");
        assert_eq!(args[6], "50");
    }

    /// After the subcommand, all arguments must come in --flag value pairs
    /// (even count). An odd count would indicate a flag without a value or
    /// a positional argument, both of which break the SSH deployment command.
    #[test]
    fn to_args_all_variants_have_even_flags() {
        // After the subcommand, args should come in --flag value pairs (even count)
        for p in all_variants() {
            let args = p.to_args();
            let flag_args = &args[1..]; // skip subcommand
            assert_eq!(
                flag_args.len() % 2, 0,
                "Flags for {} should come in pairs, got {}",
                p.search_type_name(), flag_args.len()
            );
        }
    }
}
