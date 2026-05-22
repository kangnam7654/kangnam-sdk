//! Tarot card fortune engine.
//!
//! Provides [`TarotEngine`] — a rule-based fortune engine that draws from the
//! Rider-Waite-Smith style tarot deck and returns calculation/card identity
//! facts plus an engine version string used by callers for cache invalidation.
//!
//! Pure computation — no IO, no async, no clocks beyond [`chrono::Utc::now`]
//! for timestamping the draw in the response.

#![deny(unsafe_code)]

pub mod api;
pub mod cards;
mod category_meanings;
pub mod draw;
pub mod engine;
pub mod profile;
pub mod types;

#[cfg(test)]
mod major_arcana_tests;

pub use api::{
    TarotEngineError, TarotEngineRequest, TarotEngineResponse, generate_public_daily_tarot,
    generate_tarot_reading,
};
pub use category_meanings::{TAROT_CATEGORIES, is_valid_category};
pub use draw::{
    DRAW_POOL_SIZE, FULL_DECK_SIZE, draw_cards, draw_cards_from_pool, draw_cards_n,
    draw_cards_n_from_pool,
};
pub use engine::{TAROT_ENGINE_VERSION, TAROT_READING_TYPES, TarotEngine, is_valid_reading_type};
pub use profile::{
    TAROT_COMPATIBILITY_TARGET, TAROT_PROFILE_ID, TAROT_PROFILE_VERSION, deck_source_profile_json,
};
pub use types::{ArcanaType, DrawnCard, Ohang, SpreadType, Suit, TarotCard, TarotElement};
