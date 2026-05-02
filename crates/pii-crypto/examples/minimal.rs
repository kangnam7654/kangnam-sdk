//! Encrypt and decrypt a short plaintext, print ciphertext length.
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use pii_crypto::CryptoService;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let key = B64.encode([0u8; 32]);
    let svc = CryptoService::new(&key)?;
    let ct = svc.encrypt(b"hello world")?;
    let pt = svc.decrypt(&ct)?;
    println!(
        "ciphertext {} bytes, plaintext = {:?}",
        ct.len(),
        String::from_utf8(pt)?
    );
    Ok(())
}
