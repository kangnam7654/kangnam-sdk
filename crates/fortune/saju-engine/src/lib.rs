//! Korean four-pillars (사주팔자) astrology computation engine.
//!
//! Pure-computation library: given a birth date/time, returns typed values
//! (`FourPillars`, `ElementBalance`, `TenGod`, …) plus Korean interpretation
//! text. No IO, no rendering, no database.
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
//! - [`interpreter`] — Korean interpretation text generators.
//! - [`daily`] — Daily fortune calculation (including `daily_detail`).
//! - [`monthly`] — Monthly fortune over a whole year.
//! - [`daeun`] — Great-luck (대운) 10-year period calculation.
//! - [`tables`] — Sexagenary cycle lookup tables.

#![deny(unsafe_code)]

pub mod branches;
pub mod daeun;
pub mod daily;
pub mod elements;
pub mod engine;
pub mod gongmang;
pub mod interpreter;
pub mod lucky;
pub mod monthly;
pub mod pillars;
pub mod shinsal;
pub mod tables;
pub mod ten_gods;
pub mod types;

// Re-export the most commonly used items at the crate root to match the
// previous `use crate::services::fortune_engine::saju::{calculate_four_pillars, types::*}`
// ergonomics that backend consumers relied on.
pub use engine::SajuEngine;
pub use pillars::calculate_four_pillars;
pub use types::{Branch, Element, ElementBalance, FourPillars, Pillar, Polarity, Stem, TenGod};
