use pii_crypto::{CryptoError, CryptoService};

fn test_key_b64() -> String {
    use base64::{Engine, engine::general_purpose::STANDARD as B64};
    B64.encode(b"01234567890123456789012345678901")
}

#[test]
fn encrypt_decrypt_roundtrip_bytes() {
    let svc = CryptoService::new(&test_key_b64()).expect("service");
    let plaintext = b"sensitive birth data: 1990-01-01";
    let encrypted = svc.encrypt(plaintext).expect("encrypt");
    let decrypted = svc.decrypt(&encrypted).expect("decrypt");
    assert_eq!(decrypted, plaintext);
}

#[test]
fn encrypt_decrypt_roundtrip_string_with_korean() {
    let svc = CryptoService::new(&test_key_b64()).expect("service");
    let plaintext = "1995-03-15 서울 오전 6시 35분";
    let encrypted = svc.encrypt_string(plaintext).expect("encrypt");
    let decrypted = svc.decrypt_to_string(&encrypted).expect("decrypt");
    assert_eq!(decrypted, plaintext);
}

#[test]
fn nonce_is_random_ciphertexts_differ() {
    let svc = CryptoService::new(&test_key_b64()).expect("service");
    let a = svc.encrypt(b"same input").expect("encrypt a");
    let b = svc.encrypt(b"same input").expect("encrypt b");
    assert_ne!(
        a, b,
        "random nonce should produce different ciphertexts for identical plaintext"
    );
}

#[test]
fn rejects_key_with_wrong_length() {
    use base64::{Engine, engine::general_purpose::STANDARD as B64};
    let bad_key = B64.encode(b"too short"); // 9 bytes, not 32
    let err = CryptoService::new(&bad_key).expect_err("must reject");
    assert!(matches!(
        err,
        CryptoError::InvalidKeyLength {
            expected: 32,
            actual: 9
        }
    ));
}

#[test]
fn rejects_malformed_base64_key() {
    let err = CryptoService::new("!!!not-base64!!!").expect_err("must reject");
    assert!(matches!(err, CryptoError::InvalidKeyEncoding(_)));
}

#[test]
fn rejects_data_shorter_than_nonce_plus_tag() {
    let svc = CryptoService::new(&test_key_b64()).expect("service");
    // Below the nonce length (12).
    let err = svc.decrypt(&[0u8; 5]).expect_err("must reject");
    assert!(matches!(err, CryptoError::CiphertextTooShort { .. }));
}

#[test]
fn rejects_data_between_nonce_and_min_ciphertext() {
    // 12..28 bytes is long enough to hold a nonce but not a GCM tag.
    // Guard must catch this before aes-gcm's own decrypt call.
    let svc = CryptoService::new(&test_key_b64()).expect("service");
    for len in [
        CryptoService::NONCE_LEN,
        20,
        CryptoService::MIN_CIPHERTEXT_LEN - 1,
    ] {
        let err = svc.decrypt(&vec![0u8; len]).expect_err("must reject");
        assert!(
            matches!(
                err,
                CryptoError::CiphertextTooShort {
                    minimum: CryptoService::MIN_CIPHERTEXT_LEN,
                    ..
                }
            ),
            "len={len} should hit the guard, got {err:?}",
        );
    }
}

#[test]
fn rejects_tampered_ciphertext() {
    let svc = CryptoService::new(&test_key_b64()).expect("service");
    let mut encrypted = svc.encrypt(b"payload").expect("encrypt");
    // Flip a bit in the ciphertext body (after the 12-byte nonce).
    encrypted[15] ^= 0x01;
    let err = svc.decrypt(&encrypted).expect_err("GCM must detect tamper");
    assert!(matches!(err, CryptoError::DecryptFailed(_)));
}

#[test]
fn decrypt_to_string_rejects_non_utf8() {
    let svc = CryptoService::new(&test_key_b64()).expect("service");
    let encrypted = svc.encrypt(&[0xff, 0xfe, 0xfd]).expect("encrypt");
    let err = svc.decrypt_to_string(&encrypted).expect_err("must reject");
    assert!(matches!(err, CryptoError::InvalidUtf8(_)));
}
