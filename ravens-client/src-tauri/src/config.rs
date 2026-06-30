use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use aes_gcm::aead::rand_core::RngCore;
use base64::{engine::general_purpose::STANDARD, Engine};
use sha2::{Sha256, Digest};

fn derive_key(hwid: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"ravens-nexus-v1:");
    hasher.update(hwid.as_bytes());
    hasher.finalize().into()
}

pub fn encrypt(plaintext: &str, hwid: &str) -> anyhow::Result<String> {
    let key = derive_key(hwid);
    let cipher = Aes256Gcm::new_from_slice(&key)?;
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher.encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| anyhow::anyhow!("encrypt: {e}"))?;
    let mut out = nonce_bytes.to_vec();
    out.extend_from_slice(&ciphertext);
    Ok(STANDARD.encode(out))
}

pub fn decrypt(encoded: &str, hwid: &str) -> anyhow::Result<String> {
    let data = STANDARD.decode(encoded)?;
    if data.len() < 12 {
        anyhow::bail!("invalid ciphertext");
    }
    let (nonce_bytes, ciphertext) = data.split_at(12);
    let key = derive_key(hwid);
    let cipher = Aes256Gcm::new_from_slice(&key)?;
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher.decrypt(nonce, ciphertext)
        .map_err(|e| anyhow::anyhow!("decrypt: {e}"))?;
    Ok(String::from_utf8(plaintext)?)
}
