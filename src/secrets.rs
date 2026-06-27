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

macro_rules! compile_or_env {
    ($name:literal) => {
        option_env!($name)
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| env::var($name).unwrap_or_default())
    };
}

fn secrets() -> Secrets {
    static SECRETS: Lazy<Secrets> = Lazy::new(|| Secrets {
        github_token: compile_or_env!("STARAI_GITHUB_TOKEN"),
        github_token_fallback_1: compile_or_env!("STARAI_GITHUB_TOKEN_FALLBACK_1"),
        github_token_fallback_2: compile_or_env!("STARAI_GITHUB_TOKEN_FALLBACK_2"),
        github_token_fallback_3: compile_or_env!("STARAI_GITHUB_TOKEN_FALLBACK_3"),
        xunfei_app_id: compile_or_env!("STARAI_XUNFEI_APP_ID"),
        xunfei_api_key: compile_or_env!("STARAI_XUNFEI_API_KEY"),
        xunfei_api_secret: compile_or_env!("STARAI_XUNFEI_API_SECRET"),
        agnes_api_key: compile_or_env!("STARAI_AGNES_API_KEY"),
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
