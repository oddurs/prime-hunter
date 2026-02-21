//! # SearchManager — Deprecated Subprocess-Based Search Orchestration
//!
//! **Deprecated**: The subprocess-based `SearchManager` has been replaced by
//! PostgreSQL-backed search job management. Searches are now created as
//! `search_jobs` + `work_blocks` in the database, and nodes claim blocks
//! directly via `FOR UPDATE SKIP LOCKED`.
//!
//! This module re-exports `SearchParams` from [`crate::search_params`] for
//! backward compatibility. New code should import from `search_params` directly.

/// Re-export SearchParams for backward compatibility.
pub use crate::search_params::SearchParams;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_params_reexport_works() {
        let p = SearchParams::Factorial { start: 1, end: 100 };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("factorial"));
    }
}
