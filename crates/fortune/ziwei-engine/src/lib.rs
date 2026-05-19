//! Zi Wei Dou Shu (Ziwei / 자미두수) chart computation engine.
//!
//! This crate is pure computation: it receives birth date/time data and
//! returns a typed chart plus a compatibility JSON payload. It performs no IO,
//! rendering, database access, or LLM calls.
//!
//! The MVP scope intentionally locks the structural chart contract first:
//! 12 palaces, life/body palaces, five-element bureau, Ziwei/Tianfu positions,
//! and the 14 major stars.

#![deny(unsafe_code)]

pub mod api;
pub mod calendar;
pub mod chart;
pub mod engine;
pub mod types;

pub use api::{
    ZiweiEngineError, ZiweiEngineRequest, ZiweiEngineResponse, generate_ziwei_chart,
    generate_ziwei_reading,
};
pub use chart::{ChartError, ChartInput, ZIWEI_SCHEMA_VERSION, branch_for_hour, calculate_chart};
pub use engine::{ZIWEI_ENGINE_VERSION, ZIWEI_READING_TYPES, ZiweiEngine, is_valid_reading_type};
pub use types::{
    BirthData, Branch, Element, FiveElementBureau, MajorStar, Palace, PalaceName, StarPlacement,
    Stem, ZiweiChart,
};
