use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Checkpoint {
    Factorial {
        last_n: u64,
    },
    Palindromic {
        digit_count: u64,
        half_value: String,
    },
    Kbn {
        last_n: u64,
    },
}

pub fn save(path: &Path, checkpoint: &Checkpoint) -> Result<()> {
    let json = serde_json::to_string_pretty(checkpoint)?;
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, &json)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

pub fn load(path: &Path) -> Option<Checkpoint> {
    let data = fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

pub fn clear(path: &Path) {
    let _ = fs::remove_file(path);
}
