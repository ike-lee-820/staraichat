use once_cell::sync::Lazy;
use std::env;

#[derive(Clone, Default)]
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

fn secrets() -> Secrets {
    static SECRETS: Lazy<Secrets> = Lazy::new(|| Secrets {
        github_token: env::var("STARAI_GITHUB_TOKEN").unwrap_or_default(),
        github_token_fallback_1: env::var("STARAI_GITHUB_TOKEN_FALLBACK_1").unwrap_or_default(),
        github_token_fallback_2: env::var("STARAI_GITHUB_TOKEN_FALLBACK_2").unwrap_or_default(),
        github_token_fallback_3: env::var("STARAI_GITHUB_TOKEN_FALLBACK_3").unwrap_or_default(),
        xunfei_app_id: env::var("STARAI_XUNFEI_APP_ID").unwrap_or_default(),
        xunfei_api_key: env::var("STARAI_XUNFEI_API_KEY").unwrap_or_default(),
        xunfei_api_secret: env::var("STARAI_XUNFEI_API_SECRET").unwrap_or_default(),
        agnes_api_key: env::var("STARAI_AGNES_API_KEY").unwrap_or_default(),
    });
    SECRETS.clone()
}

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
