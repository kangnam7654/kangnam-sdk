//! Korean four-pillars (사주팔자) astrology computation engine.
//!
//! Pure-computation library: given a birth date/time, returns typed values
//! (`FourPillars`, `ElementBalance`, `TenGod`, …). The legacy
//! `SajuEngine::generate("saju", ...)` compatibility payload still includes
//! Korean interpretation text, but calculation-first consumers should use the
//! core saju fields instead. No IO, no rendering, no database.
//!
//! # Example
//!
//! ```
//! use saju_engine::{calculate_four_pillars, ElementBalance};
//!
//! // 1990-05-15 14:00 KST
//! let pillars = calculate_four_pillars(1990, 5, 15, 14);
//! let balance = ElementBalance::from_pillars(&pillars);
//!
//! // Every birth produces a non-trivial element distribution.
//! let total = balance.wood + balance.fire + balance.earth
//!           + balance.metal + balance.water;
//! assert_eq!(total, 8, "four pillars × (stem + branch) = 8 element counts");
//! ```
//!
//! # Modules
//!
//! - [`types`] — `Stem`, `Branch`, `Element`, `Polarity`, `TenGod`, `Pillar`,
//!   `FourPillars`, `ElementBalance`.
//! - [`pillars`] — Year/month/day/hour pillar computation from a birth date.
//! - [`elements`] — Five-element relations (생/극, generating/controlling).
//! - [`ten_gods`] — Ten-gods (십신) derivation and analysis.
//! - [`branches`] — Earthly-branch relations (삼합/육합/상충/상형).
//! - [`interpreter`] — legacy Korean interpretation text generators.
//! - [`daily`] — Daily fortune calculation (including `daily_detail`).
//! - [`monthly`] — Monthly fortune over a whole year.
//! - [`daeun`] — Great-luck (대운) 10-year period calculation.
//! - [`tables`] — Sexagenary cycle lookup tables.

#![deny(unsafe_code)]

pub mod api;
pub mod branches;
pub mod calendar;
pub mod daeun;
pub mod daily;
pub mod elements;
pub mod engine;
pub mod enrichment;
pub mod gongmang;
pub mod interpretation;
pub mod interpreter;
pub mod lucky;
pub mod monthly;
pub mod natal_categories;
pub mod pillars;
pub mod profile;
pub mod shinsal;
pub mod solar_terms;
pub mod tables;
pub mod ten_gods;
pub mod types;

// Re-export the most commonly used items at the crate root to match the
// previous `use crate::services::fortune_engine::saju::{calculate_four_pillars, types::*}`
// ergonomics that backend consumers relied on.
pub use api::{
    BirthInput, ParsedBirth, ParsedBirthTime, SAJU_DAILY_CATEGORIES, SajuEngineError,
    SajuEngineRequest, SajuEngineResponse, generate_daily_saju, generate_saju_compatibility,
    generate_saju_profile, generate_saju_reading, parse_birth_input, parse_birth_time,
};
pub use engine::{
    SAJU_CORE_SCHEMA_VERSION, SAJU_ENGINE_VERSION, SAJU_READING_TYPES, SajuEngine,
    is_valid_reading_type,
};
pub use pillars::{calculate_four_pillars, calculate_four_pillars_precise};
pub use profile::{
    SAJU_COMPATIBILITY_TARGET, SAJU_PROFILE_ID, SAJU_PROFILE_VERSION, calculation_profile_json,
};
pub use types::{Branch, Element, ElementBalance, FourPillars, Pillar, Polarity, Stem, TenGod};
