use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, OsRng},
};
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use rand::RngCore;

use crate::error::{CryptoError, Result};

/// AES-256-GCM envelope for PII. Ciphertext format: `[12-byte nonce || GCM ciphertext || 16-byte tag]`.
///
/// Construct with a base64-encoded 32-byte key:
///
/// ```
/// use pii_crypto::CryptoService;
/// use base64::{engine::general_purpose::STANDARD as B64, Engine};
/// let key = B64.encode(&[0u8; 32]);
/// let svc = CryptoService::new(&key).unwrap();
/// let ct = svc.encrypt(b"hello").unwrap();
/// assert_eq!(svc.decrypt(&ct).unwrap(), b"hello");
/// ```
#[derive(Clone)]
pub struct CryptoService {
    cipher: Aes256Gcm,
}

impl std::fmt::Debug for CryptoService {
    /// Intentionally opaque — never print the underlying key material.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CryptoService").finish_non_exhaustive()
    }
}

impl CryptoService {
    /// Expected key length in bytes (AES-256 ⇒ 32 bytes).
    pub const KEY_LEN: usize = 32;

    /// Nonce length in bytes (GCM standard is 96-bit = 12 bytes).
    pub const NONCE_LEN: usize = 12;

    /// GCM authentication tag length in bytes (128-bit).
    pub const TAG_LEN: usize = 16;

    /// Minimum valid ciphertext envelope: nonce + empty-plaintext tag.
    pub const MIN_CIPHERTEXT_LEN: usize = Self::NONCE_LEN + Self::TAG_LEN;

    /// Build a service from a base64-encoded 32-byte key.
    pub fn new(key_b64: &str) -> Result<Self> {
        let key_bytes = B64.decode(key_b64)?;
        if key_bytes.len() != Self::KEY_LEN {
            return Err(CryptoError::InvalidKeyLength {
                expected: Self::KEY_LEN,
                actual: key_bytes.len(),
            });
        }
        let cipher = Aes256Gcm::new_from_slice(&key_bytes)
            .map_err(|e| CryptoError::CipherInit(e.to_string()))?;
        Ok(Self { cipher })
    }

    /// Encrypt raw bytes. Output = nonce (12B) || ciphertext || GCM tag (16B).
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let mut nonce_bytes = [0u8; Self::NONCE_LEN];
        OsRng.fill_bytes(&mut nonce_bytes);
        // `Nonce::from_slice` is deprecated in generic-array 1.x; aes-gcm 0.10 still
        // ships with generic-array 0.x. Revisit when aes-gcm ≥ 0.11 lands.
        #[allow(deprecated)]
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| CryptoError::EncryptFailed(e.to_string()))?;
        let mut out = Vec::with_capacity(Self::NONCE_LEN + ciphertext.len());
        out.extend_from_slice(&nonce_bytes);
        out.extend(ciphertext);
        Ok(out)
    }

    /// Decrypt bytes produced by [`Self::encrypt`]. Returns `CiphertextTooShort` if fewer
    /// than `MIN_CIPHERTEXT_LEN` bytes (nonce + GCM tag), `DecryptFailed` on tag mismatch
    /// or corruption.
    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        if data.len() < Self::MIN_CIPHERTEXT_LEN {
            return Err(CryptoError::CiphertextTooShort {
                minimum: Self::MIN_CIPHERTEXT_LEN,
                actual: data.len(),
            });
        }
        let (nonce_bytes, ciphertext) = data.split_at(Self::NONCE_LEN);
        // See `encrypt` for why `#[allow(deprecated)]` is needed.
        #[allow(deprecated)]
        let nonce = Nonce::from_slice(nonce_bytes);
        self.cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| CryptoError::DecryptFailed(e.to_string()))
    }

    /// Encrypt a UTF-8 string.
    pub fn encrypt_string(&self, plaintext: &str) -> Result<Vec<u8>> {
        self.encrypt(plaintext.as_bytes())
    }

    /// Decrypt and parse as UTF-8. Returns `InvalidUtf8` if the plaintext is not valid UTF-8.
    pub fn decrypt_to_string(&self, data: &[u8]) -> Result<String> {
        let bytes = self.decrypt(data)?;
        Ok(String::from_utf8(bytes)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> String {
        B64.encode(b"01234567890123456789012345678901")
    }

    #[test]
    fn roundtrip_bytes() {
        let svc = CryptoService::new(&key()).unwrap();
        let pt = b"hello";
        assert_eq!(svc.decrypt(&svc.encrypt(pt).unwrap()).unwrap(), pt);
    }

    #[test]
    fn random_nonce_ciphertexts_differ() {
        let svc = CryptoService::new(&key()).unwrap();
        assert_ne!(svc.encrypt(b"x").unwrap(), svc.encrypt(b"x").unwrap());
    }
}
