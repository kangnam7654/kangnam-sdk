# pii-crypto

AES-256-GCM envelope encryption for personally identifiable information (PII).

## Purpose

Small, focused Rust library for encrypting short PII blobs (names, birth dates, phone numbers) before persisting to a database. Uses AES-256-GCM with a fresh random 12-byte nonce per encryption, prepended to the ciphertext so the envelope self-contains everything needed for decryption. Pure computation, no IO.

## Installation

```toml
[dependencies]
pii-crypto = { git = "https://github.com/kangnam7654/pii-crypto", tag = "v0.1.0" }
```

## Quick Start

```rust
use pii_crypto::CryptoService;
use base64::{engine::general_purpose::STANDARD as B64, Engine};

// DEMO ONLY — in production generate with `openssl rand -base64 32`
// and load from a secret manager. Never ship an all-zero key.
let key = B64.encode(&[0u8; 32]);                // 32-byte key, base64-encoded
let svc = CryptoService::new(&key)?;
let ct = svc.encrypt(b"hello world")?;
let pt = svc.decrypt(&ct)?;
assert_eq!(pt, b"hello world");
```

## API Overview

- `CryptoService::new(key_b64: &str) -> Result<Self>` — construct with a base64-encoded 32-byte key.
- `encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>>` — returns `nonce(12) ∥ ciphertext ∥ tag(16)`.
- `decrypt(&self, data: &[u8]) -> Result<Vec<u8>>` — inverse of `encrypt`.
- `encrypt_string` / `decrypt_to_string` — UTF-8 convenience wrappers.

## Examples

See `examples/`:

- `cargo run --example minimal` — encrypts `"hello world"`, decrypts it back, prints ciphertext length.

## Operational Notes

- **Key generation.** 32 random bytes, base64-encoded. `openssl rand -base64 32` or any CSPRNG. Load from a secret manager, not source control.
- **Nonce-reuse ceiling.** AES-GCM with random 96-bit nonces is safe until the birthday bound of ~2³² encryptions per key (~4 billion). Rotate keys well before then. For typical PII workloads (one encrypt per row write) this is effectively unreachable, but batch/re-encrypt jobs can burn through it fast — plan key rotation accordingly.
- **Integrity.** The 16-byte GCM tag is appended to the ciphertext. Any bit-flip in the envelope causes `decrypt` to return `CryptoError::Decrypt`.
- **No key rotation helper (yet).** Callers must re-encrypt with the new key and update persisted envelopes themselves. A future release may add a `rotate` helper.

## Stability

v0.x — API may change between minor versions. v1.0 will commit to semver.

## License

MIT. See [LICENSE](LICENSE).
