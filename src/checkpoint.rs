//! # Checkpoint — Resumable Search State Persistence
//!
//! Saves and loads search progress as JSON files with SHA-256 integrity
//! verification and generational backups. Each search form has its own
//! `Checkpoint` variant storing the minimal state needed to resume.
//!
//! ## Atomic Writes
//!
//! Checkpoint files are written atomically: write to a temp file, then rename.
//! This prevents corruption from mid-write crashes or power loss.
//!
//! ## Integrity
//!
//! A SHA-256 hash is stored alongside the JSON data. On load, the hash is
//! verified — corrupted checkpoints are detected and skipped, falling back
//! to the most recent valid generation (up to 3 generations kept).
//!
//! ## Checkpoint Variants
//!
//! One variant per search form (Factorial, Palindromic, Kbn, Primorial,
//! CullenWoodall, Wagstaff, CarolKynea, Twin, SophieGermain, Repunit,
//! GenFermat, NearRepdigit). Each stores the minimum state needed to
//! resume without re-sieving or re-computing intermediate values.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Number of backup generations to keep.
const GENERATIONS: usize = 3;

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Checkpoint {
    Factorial {
        last_n: u64,
        #[serde(default)]
        start: Option<u64>,
        #[serde(default)]
        end: Option<u64>,
    },
    Palindromic {
        digit_count: u64,
        half_value: String,
        #[serde(default)]
        min_digits: Option<u64>,
        #[serde(default)]
        max_digits: Option<u64>,
    },
    Kbn {
        last_n: u64,
        #[serde(default)]
        min_n: Option<u64>,
        #[serde(default)]
        max_n: Option<u64>,
    },
    NearRepdigit {
        digit_count: u64,
        d: u32,
        m: u64,
        #[serde(default)]
        min_digits: Option<u64>,
        #[serde(default)]
        max_digits: Option<u64>,
    },
    Primorial {
        last_prime: u64,
        #[serde(default)]
        start: Option<u64>,
        #[serde(default)]
        end: Option<u64>,
    },
    CullenWoodall {
        last_n: u64,
        #[serde(default)]
        min_n: Option<u64>,
        #[serde(default)]
        max_n: Option<u64>,
    },
    Wagstaff {
        last_exp: u64,
        #[serde(default)]
        min_exp: Option<u64>,
        #[serde(default)]
        max_exp: Option<u64>,
    },
    CarolKynea {
        last_n: u64,
        #[serde(default)]
        min_n: Option<u64>,
        #[serde(default)]
        max_n: Option<u64>,
    },
    Twin {
        last_n: u64,
        #[serde(default)]
        k: Option<u64>,
        #[serde(default)]
        base: Option<u32>,
        #[serde(default)]
        min_n: Option<u64>,
        #[serde(default)]
        max_n: Option<u64>,
    },
    SophieGermain {
        last_n: u64,
        #[serde(default)]
        k: Option<u64>,
        #[serde(default)]
        base: Option<u32>,
        #[serde(default)]
        min_n: Option<u64>,
        #[serde(default)]
        max_n: Option<u64>,
    },
    Repunit {
        last_n: u64,
        #[serde(default)]
        base: Option<u32>,
        #[serde(default)]
        min_n: Option<u64>,
        #[serde(default)]
        max_n: Option<u64>,
    },
    GenFermat {
        last_base: u64,
        #[serde(default)]
        fermat_n: Option<u32>,
        #[serde(default)]
        min_base: Option<u64>,
        #[serde(default)]
        max_base: Option<u64>,
    },
}

/// Wrapper that includes a SHA-256 checksum for integrity verification.
#[derive(Serialize, Deserialize)]
struct CheckpointEnvelope {
    checksum: String,
    data: serde_json::Value,
}

/// Compute SHA-256 hex digest of a string.
fn sha256_hex(data: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Return the path for generation `gen` (0 = current, 1 = .1, 2 = .2, ...).
fn generation_path(base: &Path, gen: usize) -> PathBuf {
    if gen == 0 {
        base.to_path_buf()
    } else {
        let mut p = base.as_os_str().to_os_string();
        p.push(format!(".{}", gen));
        PathBuf::from(p)
    }
}

/// Save a checkpoint with integrity checksum and rotating generations.
///
/// Rotation: current → .1 → .2 (oldest .2 is discarded).
/// The new checkpoint is written atomically via a .tmp file.
pub fn save(path: &Path, checkpoint: &Checkpoint) -> Result<()> {
    // Rotate existing generations: .2 is discarded, .1 → .2, current → .1
    for gen in (1..GENERATIONS).rev() {
        let src = generation_path(path, gen - 1);
        let dst = generation_path(path, gen);
        if src.exists() {
            let _ = fs::rename(&src, &dst);
        }
    }

    // Serialize the checkpoint data
    let data = serde_json::to_value(checkpoint)?;
    let data_str = serde_json::to_string_pretty(&data)?;
    let checksum = sha256_hex(&data_str);

    let envelope = CheckpointEnvelope { checksum, data };
    let json = serde_json::to_string_pretty(&envelope)?;

    // Atomic write: write to .tmp then rename
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, &json)?;
    fs::rename(&tmp, path)?;

    Ok(())
}

/// Load the newest valid checkpoint, falling back to older generations on corruption.
pub fn load(path: &Path) -> Option<Checkpoint> {
    for gen in 0..GENERATIONS {
        let p = generation_path(path, gen);
        if let Some(cp) = load_single(&p) {
            if gen > 0 {
                warn!(
                    generation = gen,
                    path = %p.display(),
                    "Recovered checkpoint from older generation"
                );
            }
            return Some(cp);
        }
    }

    // Legacy fallback: try loading without envelope (pre-hardening checkpoints)
    let data = fs::read_to_string(path).ok()?;
    let cp: Checkpoint = serde_json::from_str(&data).ok()?;
    info!("Loaded legacy checkpoint (no checksum)");
    Some(cp)
}

/// Try to load and verify a single checkpoint file.
fn load_single(path: &Path) -> Option<Checkpoint> {
    let raw = fs::read_to_string(path).ok()?;
    let envelope: CheckpointEnvelope = serde_json::from_str(&raw).ok()?;

    // Verify integrity
    let data_str = serde_json::to_string_pretty(&envelope.data).ok()?;
    let expected = sha256_hex(&data_str);
    if expected != envelope.checksum {
        warn!(
            path = %path.display(),
            expected = &expected[..12],
            got = &envelope.checksum[..12.min(envelope.checksum.len())],
            "Checkpoint integrity check failed"
        );
        return None;
    }

    serde_json::from_value(envelope.data).ok()
}

/// Clear all checkpoint files (current + all generations).
pub fn clear(path: &Path) {
    for gen in 0..GENERATIONS {
        let _ = fs::remove_file(generation_path(path, gen));
    }
    // Also clean up any leftover .tmp file
    let _ = fs::remove_file(path.with_extension("tmp"));
}

#[cfg(test)]
mod tests {
    //! Tests for the checkpoint subsystem — resumable search state persistence.
    //!
    //! Validates the atomic write strategy (write to .tmp, rename), SHA-256
    //! integrity verification, generational rotation (3 generations max),
    //! corruption fallback, legacy format loading, and save/load round-trips
    //! for all 12 checkpoint variants.
    //!
    //! ## Atomic Write + Generation Rotation Strategy
    //!
    //! Each save() performs: rotate .1->.2, current->.1, write new to .tmp,
    //! rename .tmp to current. This ensures:
    //! 1. A crash during write leaves the .tmp file (ignored on load)
    //! 2. Corruption of current falls back to generation .1 or .2
    //! 3. Disk usage is bounded to 3x the checkpoint size
    //!
    //! ## Integrity Verification
    //!
    //! Each checkpoint is wrapped in a CheckpointEnvelope containing a SHA-256
    //! hash of the pretty-printed data JSON. On load, the hash is recomputed
    //! and compared — any tampering or bit-rot is detected.

    use super::*;
    use std::io::Write;

    // ── Round-Trip Tests ───────────────────────────────────────────

    /// Basic save/load round-trip for the Factorial variant. Verifies that
    /// all fields (last_n, start, end) survive serialization through the
    /// CheckpointEnvelope wrapper with SHA-256 integrity.
    #[test]
    fn save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("checkpoint.json");

        let cp = Checkpoint::Factorial {
            last_n: 42,
            start: Some(1),
            end: Some(100),
        };
        save(&path, &cp).unwrap();

        let loaded = load(&path).unwrap();
        match loaded {
            Checkpoint::Factorial { last_n, start, end } => {
                assert_eq!(last_n, 42);
                assert_eq!(start, Some(1));
                assert_eq!(end, Some(100));
            }
            _ => panic!("Wrong checkpoint type"),
        }
    }

    // ── Generation Rotation ──────────────────────────────────────

    /// Validates that 3 consecutive saves create 3 generation files with
    /// correct values: current=30, .1=20, .2=10. The rotation strategy
    /// shifts each file down one generation on every save.
    #[test]
    fn rotation_keeps_generations() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("checkpoint.json");

        // Save 3 generations
        for n in 1..=3u64 {
            save(
                &path,
                &Checkpoint::Factorial {
                    last_n: n * 10,
                    start: None,
                    end: None,
                },
            )
            .unwrap();
        }

        // Current should be last_n=30, gen .1 should be 20, gen .2 should be 10
        assert!(path.exists());
        assert!(generation_path(&path, 1).exists());
        assert!(generation_path(&path, 2).exists());

        let current = load_single(&path).unwrap();
        match current {
            Checkpoint::Factorial { last_n, .. } => assert_eq!(last_n, 30),
            _ => panic!("Wrong type"),
        }

        let gen1 = load_single(&generation_path(&path, 1)).unwrap();
        match gen1 {
            Checkpoint::Factorial { last_n, .. } => assert_eq!(last_n, 20),
            _ => panic!("Wrong type"),
        }

        let gen2 = load_single(&generation_path(&path, 2)).unwrap();
        match gen2 {
            Checkpoint::Factorial { last_n, .. } => assert_eq!(last_n, 10),
            _ => panic!("Wrong type"),
        }
    }

    // ── Corruption Recovery ──────────────────────────────────────

    /// When the current checkpoint is corrupted (invalid JSON), load() must
    /// fall back to generation .1. This simulates a power loss during write
    /// or disk corruption of the most recent file.
    #[test]
    fn fallback_on_corruption() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("checkpoint.json");

        // Save a valid checkpoint, then save another (pushing first to .1)
        save(
            &path,
            &Checkpoint::Kbn {
                last_n: 100,
                min_n: Some(1),
                max_n: Some(1000),
            },
        )
        .unwrap();
        save(
            &path,
            &Checkpoint::Kbn {
                last_n: 200,
                min_n: Some(1),
                max_n: Some(1000),
            },
        )
        .unwrap();

        // Corrupt the current file
        {
            let mut f = fs::File::create(&path).unwrap();
            f.write_all(b"corrupted data!!!").unwrap();
        }

        // load() should fall back to generation .1 (last_n=100)
        let loaded = load(&path).unwrap();
        match loaded {
            Checkpoint::Kbn { last_n, .. } => assert_eq!(last_n, 100),
            _ => panic!("Wrong type"),
        }
    }

    // ── Legacy Format ────────────────────────────────────────────

    /// Pre-hardening checkpoints (no envelope, raw JSON) must still load.
    /// This ensures backward compatibility when upgrading from older versions.
    #[test]
    fn legacy_checkpoint_loads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("checkpoint.json");

        // Write a legacy checkpoint (no envelope, just raw JSON)
        let legacy = r#"{"type":"Palindromic","digit_count":7,"half_value":"1234"}"#;
        fs::write(&path, legacy).unwrap();

        let loaded = load(&path).unwrap();
        match loaded {
            Checkpoint::Palindromic {
                digit_count,
                half_value,
                ..
            } => {
                assert_eq!(digit_count, 7);
                assert_eq!(half_value, "1234");
            }
            _ => panic!("Wrong type"),
        }
    }

    // ── All-Variants Exhaustive ──────────────────────────────────

    /// Exhaustive round-trip test for all 12 checkpoint variants. Each form
    /// stores different state (last_n, digit_count, exponent, etc.) and
    /// optional bounds. A missing variant here means a new search form was
    /// added without updating the checkpoint system.
    #[test]
    fn all_checkpoint_variants_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let variants: Vec<(&str, Checkpoint)> = vec![
            (
                "factorial",
                Checkpoint::Factorial {
                    last_n: 42,
                    start: Some(1),
                    end: Some(100),
                },
            ),
            (
                "palindromic",
                Checkpoint::Palindromic {
                    digit_count: 7,
                    half_value: "1234".into(),
                    min_digits: Some(1),
                    max_digits: Some(99),
                },
            ),
            (
                "kbn",
                Checkpoint::Kbn {
                    last_n: 500,
                    min_n: Some(1),
                    max_n: Some(1000),
                },
            ),
            (
                "near_repdigit",
                Checkpoint::NearRepdigit {
                    digit_count: 11,
                    d: 3,
                    m: 2,
                    min_digits: Some(5),
                    max_digits: Some(99),
                },
            ),
            (
                "primorial",
                Checkpoint::Primorial {
                    last_prime: 29,
                    start: Some(2),
                    end: Some(100),
                },
            ),
            (
                "cullen_woodall",
                Checkpoint::CullenWoodall {
                    last_n: 50,
                    min_n: Some(1),
                    max_n: Some(200),
                },
            ),
            (
                "wagstaff",
                Checkpoint::Wagstaff {
                    last_exp: 43,
                    min_exp: Some(3),
                    max_exp: Some(200),
                },
            ),
            (
                "carol_kynea",
                Checkpoint::CarolKynea {
                    last_n: 25,
                    min_n: Some(1),
                    max_n: Some(100),
                },
            ),
            (
                "twin",
                Checkpoint::Twin {
                    last_n: 100,
                    k: Some(3),
                    base: Some(2),
                    min_n: Some(1),
                    max_n: Some(1000),
                },
            ),
            (
                "sophie_germain",
                Checkpoint::SophieGermain {
                    last_n: 80,
                    k: Some(1),
                    base: Some(2),
                    min_n: Some(2),
                    max_n: Some(500),
                },
            ),
            (
                "repunit",
                Checkpoint::Repunit {
                    last_n: 23,
                    base: Some(10),
                    min_n: Some(2),
                    max_n: Some(1000),
                },
            ),
            (
                "gen_fermat",
                Checkpoint::GenFermat {
                    last_base: 42,
                    fermat_n: Some(3),
                    min_base: Some(2),
                    max_base: Some(10000),
                },
            ),
        ];

        for (name, cp) in &variants {
            let path = dir.path().join(format!("{}.json", name));
            save(&path, cp).unwrap();
            let loaded = load(&path).expect(&format!("Failed to load {} checkpoint", name));
            // Verify by re-serializing both and comparing
            let original_json = serde_json::to_string(cp).unwrap();
            let loaded_json = serde_json::to_string(&loaded).unwrap();
            assert_eq!(
                original_json, loaded_json,
                "Roundtrip mismatch for {} checkpoint",
                name
            );
        }
    }

    /// Optional fields (start, end) set to None must round-trip correctly.
    /// Older checkpoints may lack these fields due to #[serde(default)].
    #[test]
    fn checkpoint_with_none_optional_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cp.json");

        let cp = Checkpoint::Factorial {
            last_n: 10,
            start: None,
            end: None,
        };
        save(&path, &cp).unwrap();
        let loaded = load(&path).unwrap();
        match loaded {
            Checkpoint::Factorial { last_n, start, end } => {
                assert_eq!(last_n, 10);
                assert!(start.is_none());
                assert!(end.is_none());
            }
            _ => panic!("Wrong type"),
        }
    }

    // ── Integrity Verification ──────────────────────────────────

    /// Tampering with the data field (changing 42 to 99) while keeping the
    /// original checksum must cause load_single to reject the file. This
    /// validates the SHA-256 integrity check.
    #[test]
    fn checkpoint_checksum_detects_tampering() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cp.json");

        save(
            &path,
            &Checkpoint::Factorial {
                last_n: 42,
                start: None,
                end: None,
            },
        )
        .unwrap();

        // Tamper with the data field but keep the envelope valid JSON
        let raw = fs::read_to_string(&path).unwrap();
        let tampered = raw.replace("42", "99");
        fs::write(&path, &tampered).unwrap();

        // load_single should reject due to checksum mismatch
        assert!(load_single(&path).is_none());
    }

    // ── Clear and Cleanup ────────────────────────────────────────

    /// clear() must remove all generation files (current, .1, .2). Called
    /// when a search completes successfully and the checkpoint is no longer
    /// needed.
    #[test]
    fn clear_removes_all() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("checkpoint.json");

        for _ in 0..4 {
            save(
                &path,
                &Checkpoint::Factorial {
                    last_n: 1,
                    start: None,
                    end: None,
                },
            )
            .unwrap();
        }

        clear(&path);

        assert!(!path.exists());
        assert!(!generation_path(&path, 1).exists());
        assert!(!generation_path(&path, 2).exists());
    }

    // ── Hash Function ───────────────────────────────────────────

    /// SHA-256 of "abc" must match the NIST test vector. This is the same
    /// hash function used for checkpoint integrity verification.
    #[test]
    fn sha256_hex_known_value() {
        // SHA-256 of "abc" (well-known test vector)
        let hash = sha256_hex("abc");
        assert_eq!(
            hash,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    /// SHA-256 of empty string must match the well-known empty digest.
    #[test]
    fn sha256_hex_empty_string() {
        let hash = sha256_hex("");
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    // ── Path Generation ──────────────────────────────────────────

    /// Generation 0 returns the base path unchanged; generations 1+ append
    /// ".N" suffix. This naming scheme is assumed by both save() and load().
    #[test]
    fn generation_path_formats_correctly() {
        let base = Path::new("/tmp/checkpoint.json");
        assert_eq!(generation_path(base, 0), PathBuf::from("/tmp/checkpoint.json"));
        assert_eq!(generation_path(base, 1), PathBuf::from("/tmp/checkpoint.json.1"));
        assert_eq!(generation_path(base, 2), PathBuf::from("/tmp/checkpoint.json.2"));
    }

    // ── Edge Cases ──────────────────────────────────────────────

    /// Loading a nonexistent file must return None (fresh search, no resume).
    #[test]
    fn load_nonexistent_file_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does_not_exist.json");
        assert!(load(&path).is_none());
    }

    /// save() must create the file if it does not exist.
    #[test]
    fn save_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cp.json");
        assert!(!path.exists());
        save(
            &path,
            &Checkpoint::Factorial {
                last_n: 1,
                start: None,
                end: None,
            },
        )
        .unwrap();
        assert!(path.exists());
    }

    /// The .tmp file must not remain after a successful save. A leftover
    /// .tmp file indicates a crash during the atomic write, and is intentionally
    /// ignored by load().
    #[test]
    fn save_cleans_up_tmp_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cp.json");
        save(
            &path,
            &Checkpoint::Factorial {
                last_n: 1,
                start: None,
                end: None,
            },
        )
        .unwrap();
        // The .tmp file should not remain after a successful save
        let tmp_path = path.with_extension("tmp");
        assert!(!tmp_path.exists(), ".tmp file should not remain after save");
    }

    /// The saved file must be valid JSON containing both "checksum" and "data"
    /// fields (the CheckpointEnvelope structure).
    #[test]
    fn saved_file_is_valid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cp.json");
        save(
            &path,
            &Checkpoint::Kbn {
                last_n: 500,
                min_n: Some(1),
                max_n: Some(1000),
            },
        )
        .unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(parsed.get("checksum").is_some(), "Envelope should have checksum");
        assert!(parsed.get("data").is_some(), "Envelope should have data");
    }

    /// The stored checksum must match the SHA-256 of the pretty-printed data.
    /// This verifies the envelope was constructed correctly by save().
    #[test]
    fn envelope_checksum_matches_data() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cp.json");
        save(
            &path,
            &Checkpoint::Primorial {
                last_prime: 29,
                start: Some(2),
                end: Some(100),
            },
        )
        .unwrap();

        let raw = fs::read_to_string(&path).unwrap();
        let envelope: CheckpointEnvelope = serde_json::from_str(&raw).unwrap();
        let data_str = serde_json::to_string_pretty(&envelope.data).unwrap();
        let expected_checksum = sha256_hex(&data_str);
        assert_eq!(envelope.checksum, expected_checksum);
    }

    /// Validates that checkpoint file rotation discards the oldest generation
    /// when exceeding the 3-generation limit. After 4 saves with values
    /// [10, 20, 30, 40], the files should contain [40, 30, 20] and the
    /// original value 10 should be gone. This rotation strategy ensures
    /// crash recovery always has at least one valid checkpoint, while bounding
    /// disk usage to 3x the checkpoint size.
    #[test]
    fn fourth_save_discards_oldest_generation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("checkpoint.json");

        // Save 4 checkpoints: gen 0 (last_n=10) should be discarded
        for n in [10u64, 20, 30, 40] {
            save(
                &path,
                &Checkpoint::Factorial {
                    last_n: n,
                    start: None,
                    end: None,
                },
            )
            .unwrap();
        }

        // Current should be 40, .1 should be 30, .2 should be 20
        // The original 10 should be gone
        match load_single(&path).unwrap() {
            Checkpoint::Factorial { last_n, .. } => assert_eq!(last_n, 40),
            _ => panic!("Wrong type"),
        }
        match load_single(&generation_path(&path, 1)).unwrap() {
            Checkpoint::Factorial { last_n, .. } => assert_eq!(last_n, 30),
            _ => panic!("Wrong type"),
        }
        match load_single(&generation_path(&path, 2)).unwrap() {
            Checkpoint::Factorial { last_n, .. } => assert_eq!(last_n, 20),
            _ => panic!("Wrong type"),
        }
    }

    // ── Form-Specific Round-Trips ─────────────────────────────────

    /// NearRepdigit has additional fields (d, m) beyond the standard last_n.
    /// Verifies all 5 fields survive the round-trip.
    #[test]
    fn near_repdigit_checkpoint_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cp.json");
        let cp = Checkpoint::NearRepdigit {
            digit_count: 15,
            d: 7,
            m: 3,
            min_digits: Some(5),
            max_digits: Some(99),
        };
        save(&path, &cp).unwrap();
        let loaded = load(&path).unwrap();
        match loaded {
            Checkpoint::NearRepdigit { digit_count, d, m, min_digits, max_digits } => {
                assert_eq!(digit_count, 15);
                assert_eq!(d, 7);
                assert_eq!(m, 3);
                assert_eq!(min_digits, Some(5));
                assert_eq!(max_digits, Some(99));
            }
            _ => panic!("Wrong type"),
        }
    }

    /// Twin has optional k and base fields for the k*b^n form parameters.
    /// All 5 fields must survive the round-trip.
    #[test]
    fn twin_checkpoint_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cp.json");
        let cp = Checkpoint::Twin {
            last_n: 5000,
            k: Some(3),
            base: Some(2),
            min_n: Some(1),
            max_n: Some(10000),
        };
        save(&path, &cp).unwrap();
        let loaded = load(&path).unwrap();
        match loaded {
            Checkpoint::Twin { last_n, k, base, min_n, max_n } => {
                assert_eq!(last_n, 5000);
                assert_eq!(k, Some(3));
                assert_eq!(base, Some(2));
                assert_eq!(min_n, Some(1));
                assert_eq!(max_n, Some(10000));
            }
            _ => panic!("Wrong type"),
        }
    }

    /// clear() must also remove leftover .tmp files from interrupted saves.
    #[test]
    fn clear_also_removes_tmp_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("checkpoint.json");
        let tmp = path.with_extension("tmp");
        // Create a leftover .tmp file
        fs::write(&tmp, "leftover").unwrap();
        save(
            &path,
            &Checkpoint::Factorial { last_n: 1, start: None, end: None },
        )
        .unwrap();

        clear(&path);
        assert!(!tmp.exists(), ".tmp file should be removed by clear()");
    }
}
