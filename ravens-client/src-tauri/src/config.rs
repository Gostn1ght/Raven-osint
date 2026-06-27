//! Encrypted config stored in %APPDATA%/RavensNexus/config.json (AES-256-GCM)

use aes_gcm::{Aes256Gcm, Key, Nonce, aead::{Aead, KeyInit, OsRng as AesOsRng}};
use aes_gcm::aead::rand_core::RngCore;
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Default)]
pub struct Config {
    pub server_url: Option<String>,
    pub theme:      Option<String>,
    pub language:   Option<String>,
    pub token:      Option<String>,
}

fn config_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("RavensNexus")
        .join("config.enc")
}

fn derive_key(hwid: &str) -> [u8; 32] {
    use sha2::{Sha256, Digest};
    let mut h = Sha256::new();
    h.update(hwid.as_bytes());
    h.update(b"ravens-nexus-config-v1");
    h.finalize().into()
}

pub fn load(hwid: &str) -> Config {
    let path = config_path();
    let Ok(data) = std::fs::read(&path) else { return Config::default() };
    decrypt_json(&data, hwid).unwrap_or_default()
}

pub fn save(hwid: &str, cfg: &Config) -> anyhow::Result<()> {
    let path = config_path();
    std::fs::create_dir_all(path.parent().unwrap())?;
    let enc = encrypt_json(cfg, hwid)?;
    std::fs::write(path, enc)?;
    Ok(())
}

fn encrypt_json<T: Serialize>(val: &T, hwid: &str) -> anyhow::Result<Vec<u8>> {
    let key = Key::<Aes256Gcm>::from_slice(&derive_key(hwid));
    let cipher = Aes256Gcm::new(key);
    let mut nonce_bytes = [0u8; 12];
    AesOsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let plain = serde_json::to_vec(val)?;
    let ct = cipher.encrypt(nonce, plain.as_slice())
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    // Format: 12-byte nonce || ciphertext
    let mut out = nonce_bytes.to_vec();
    out.extend(ct);
    Ok(out)
}

fn decrypt_json<T: for<'de> Deserialize<'de>>(data: &[u8], hwid: &str) -> anyhow::Result<T> {
    if data.len() < 13 { anyhow::bail!("too short"); }
    let key = Key::<Aes256Gcm>::from_slice(&derive_key(hwid));
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(&data[..12]);
    let pt = cipher.decrypt(nonce, &data[12..])
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    Ok(serde_json::from_slice(&pt)?)
}
