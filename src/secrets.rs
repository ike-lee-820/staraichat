use once_cell::sync::Lazy;
use serde::Deserialize;
use std::env;

#[derive(Deserialize, Default, Clone)]
pub struct Secrets {
    pub github_token: String,
    pub github_token_fallback_1: String,
    pub github_token_fallback_2: String,
    pub github_token_fallback_3: String,
    pub xunfei_app_id: String,
    pub xunfei_api_key: String,
    pub xunfei_api_secret: String,
    pub agnes_api_key: String,
}

fn load_decrypted() -> String {
    static DECRYPTED: Lazy<String> = Lazy::new(|| {
        let encrypted = include_bytes!("../secrets.enc");
        if encrypted.is_empty() {
            return String::new();
        }
        if let Ok(key) = env::var("STARAI_SECRETS_KEY") {
            if let Some(decrypted) = decrypt(encrypted, &key) {
                return decrypted;
            }
        }
        String::new()
    });
    DECRYPTED.clone()
}

fn secrets() -> Secrets {
    static SECRETS: Lazy<Secrets> = Lazy::new(|| {
        let decrypted = load_decrypted();
        toml::from_str(&decrypted).unwrap_or_default()
    });
    SECRETS.clone()
}

fn decrypt(encrypted: &[u8], password: &str) -> Option<String> {
    use aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Nonce,
    };
    use sha2::{Digest, Sha256};

    if encrypted.len() < 12 {
        return None;
    }
    let (nonce_bytes, ciphertext) = encrypted.split_at(12);

    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    let key = hasher.finalize();

    let cipher = Aes256Gcm::new_from_slice(&key).ok()?;
    let nonce = Nonce::from_slice(nonce_bytes);

    cipher
        .decrypt(nonce, ciphertext)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
}

/// 返回主 token 及所有非空备用 token，按主 → 备用1 → 备用2 → 备用3 顺序。
pub fn github_tokens() -> Vec<String> {
    let s = secrets();
    let mut tokens = Vec::new();
    for token in [
        s.github_token,
        s.github_token_fallback_1,
        s.github_token_fallback_2,
        s.github_token_fallback_3,
    ] {
        if !token.is_empty() {
            tokens.push(token);
        }
    }
    tokens
}

pub fn xunfei_app_id() -> String {
    secrets().xunfei_app_id
}

pub fn xunfei_api_key() -> String {
    secrets().xunfei_api_key
}

pub fn xunfei_api_secret() -> String {
    secrets().xunfei_api_secret
}

pub fn agnes_api_key() -> String {
    secrets().agnes_api_key
}
