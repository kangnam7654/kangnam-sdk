use thiserror::Error;

/// Errors returned by [`crate::CryptoService`].
#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("invalid base64 key: {0}")]
    InvalidKeyEncoding(#[from] base64::DecodeError),

    #[error("key must be {expected} bytes, got {actual}")]
    InvalidKeyLength { expected: usize, actual: usize },

    #[error("cipher initialization failed: {0}")]
    CipherInit(String),

    #[error("encryption failed: {0}")]
    EncryptFailed(String),

    #[error("decryption failed (ciphertext may be tampered or corrupt): {0}")]
    DecryptFailed(String),

    #[error(
        "ciphertext too short: need at least {minimum} bytes (12 nonce + 16 GCM tag), got {actual}"
    )]
    CiphertextTooShort { minimum: usize, actual: usize },

    #[error("decrypted bytes are not valid UTF-8: {0}")]
    InvalidUtf8(#[from] std::string::FromUtf8Error),
}

/// Convenience alias.
pub type Result<T> = std::result::Result<T, CryptoError>;
