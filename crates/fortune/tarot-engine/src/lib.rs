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

pub mod cards;
pub mod draw;
pub mod engine;
pub mod interpreter;
pub mod major_arcana_tests;
pub mod types;

pub use engine::{TAROT_ENGINE_VERSION, TarotEngine};
pub use types::{
    ArcanaType, DrawnCard, Ohang, SpreadType, Suit, TarotCard, TarotElement, TarotReading,
};
