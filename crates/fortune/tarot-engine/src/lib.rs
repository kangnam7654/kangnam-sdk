//! Tarot card fortune engine.
//!
//! Provides [`TarotEngine`] — a rule-based fortune engine that draws from the
//! Rider-Waite Major Arcana (22 cards), interprets the reading, and returns a
//! JSON result plus an engine version string used by callers for cache
//! invalidation.
//!
//! Pure computation — no IO, no async, no clocks beyond [`chrono::Utc::now`]
//! for timestamping the draw in the response.

#![deny(unsafe_code)]

pub mod api;
pub mod cards;
pub mod category_meanings;
pub mod draw;
pub mod engine;
pub mod interpretation_enrichment;
pub mod interpreter;
pub mod types;

#[cfg(test)]
mod major_arcana_tests;

pub use api::{
    TarotEngineError, TarotEngineRequest, TarotEngineResponse, generate_public_daily_tarot,
    generate_tarot_reading,
};
pub use category_meanings::{TAROT_CATEGORIES, is_valid_category, major_category_meaning};
pub use draw::{DRAW_POOL_SIZE, draw_cards, draw_cards_n};
pub use engine::{TAROT_ENGINE_VERSION, TAROT_READING_TYPES, TarotEngine, is_valid_reading_type};
pub use interpretation_enrichment::{
    TAROT_INTERPRETATION_VERSION, enrich_tarot_result, is_current_tarot_version,
};
pub use types::{
    ArcanaType, DrawnCard, Ohang, SpreadType, Suit, TarotCard, TarotElement, TarotReading,
};
