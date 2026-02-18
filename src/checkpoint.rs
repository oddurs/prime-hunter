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
                eprintln!(
                    "Warning: recovered checkpoint from generation {} ({})",
                    gen,
                    p.display()
                );
            }
            return Some(cp);
        }
    }

    // Legacy fallback: try loading without envelope (pre-hardening checkpoints)
    let data = fs::read_to_string(path).ok()?;
    let cp: Checkpoint = serde_json::from_str(&data).ok()?;
    eprintln!("Loaded legacy checkpoint (no checksum)");
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
        eprintln!(
            "Checkpoint integrity check failed: {} (expected {}, got {})",
            path.display(),
            &expected[..12],
            &envelope.checksum[..12.min(envelope.checksum.len())]
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
    use super::*;
    use std::io::Write;

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
}
