//! AES-256-GCM envelope for personally identifiable information (PII).
//!
//! Extracted from the `lunawave` backend (previously `backend/src/crypto.rs`).
//! Reusable across future Lunawave products and pivot apps that need to store
//! sensitive fields at rest (birth data, etc).
//!
//! Ciphertext format: `[12-byte nonce || AES-256-GCM ciphertext || 16-byte tag]`.
//!
//! # Operational notes
//!
//! - **Key generation.** 32 random bytes, base64-encoded. Use `openssl rand
//!   -base64 32` or an equivalent CSPRNG. Load the key from a secret manager,
//!   never from source control. Examples in the docs that use an all-zero key
//!   are for demonstration only.
//! - **Nonce-reuse ceiling.** Random 96-bit nonces are safe up to the birthday
//!   bound of roughly 2^32 encryptions per key. Rotate keys well before then.

mod error;
mod service;

pub use error::{CryptoError, Result};
pub use service::CryptoService;
