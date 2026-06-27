use std::env;
use std::fs;

fn main() {
    println!("cargo:rerun-if-changed=secrets.toml");
    println!("cargo:rerun-if-env-changed=STARAI_SECRETS_KEY");

    let has_key = env::var("STARAI_SECRETS_KEY").is_ok();
    let has_secrets = fs::read_to_string("secrets.toml").is_ok();

    if has_key && has_secrets {
        let key = env::var("STARAI_SECRETS_KEY").unwrap();
        let plaintext = fs::read_to_string("secrets.toml").unwrap();
        let encrypted = encrypt(&plaintext, &key);
        fs::write("secrets.enc", encrypted).expect("写入 secrets.enc 失败");
    } else {
        // 没有密钥或没有 secrets.toml 时生成空占位文件，
        // 运行时解密为空字符串，应用仍可编译启动（但 API 会鉴权失败）。
        fs::write("secrets.enc", b"").expect("写入 secrets.enc 失败");
    }
}

fn encrypt(plaintext: &str, password: &str) -> Vec<u8> {
    use aes_gcm::{
        aead::{Aead, KeyInit, OsRng, rand_core::RngCore},
        Aes256Gcm, Nonce,
    };
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    let key = hasher.finalize();

    let cipher = Aes256Gcm::new_from_slice(&key).expect("无效密钥长度");

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .expect("加密失败");

    let mut result = nonce_bytes.to_vec();
    result.extend_from_slice(&ciphertext);
    result
}
