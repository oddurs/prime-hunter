//! # Project — Campaign-Style Prime Discovery Management
//!
//! Organizes prime-hunting searches into goal-driven **projects**: multi-phase
//! campaigns with objectives like record-hunting, systematic surveys, and
//! verification. Each project is defined in TOML (version-controlled), imported
//! into PostgreSQL, and orchestrated by a 30-second tick loop that advances
//! phases, creates search jobs, and tracks costs.
//!
//! ## Architecture
//!
//! ```text
//! TOML project definition (version-controlled in projects/)
//!     ↓ import
//! PostgreSQL runtime state (projects, phases, records, events)
//!     ↓ orchestrate (30s tick)
//! Search jobs + work blocks (existing infrastructure in db.rs / search_manager.rs)
//!     ↓ claim
//! Workers execute, report primes
//!     ↓ react
//! Orchestration engine advances phases, tracks records, alerts on budget
//! ```
//!
//! ## Module Structure
//!
//! - [`config`] — TOML configuration structs, parsing, validation, slugification
//! - [`types`] — Database row types for projects, phases, records, events
//! - [`cost`] — Empirical cost estimation model (power-law timing per form)
//! - [`orchestration`] — Phase state machine, auto-strategy generation, tick loop
//! - [`records`] — World record tracking via t5k.org scraping

mod config;
mod types;
mod cost;
mod orchestration;
mod records;

pub use config::*;
pub use types::*;
pub use cost::*;
pub use orchestration::*;
pub use records::*;

#[cfg(test)]
mod tests;
