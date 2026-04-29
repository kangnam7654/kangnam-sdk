//! Persistence for the harness — storage trait plus a default SQLite impl.
//!
//! Consumers can plug in their own backend by implementing [`HarnessStore`].
//! The SQLite implementation is gated behind the `sqlite` feature (default-on)
//! and lives in [`sqlite`].

mod error;
mod store;

#[cfg(feature = "sqlite")]
pub mod sqlite;

pub use error::StoreError;
pub use store::HarnessStore;

pub type Result<T> = std::result::Result<T, StoreError>;
