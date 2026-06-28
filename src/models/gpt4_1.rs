use crate::config;
use crate::models;
use crate::types::Message;
use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::mpsc::Sender;

const ENDPOINT: &str = "https://models.github.ai/inference";
const MODEL_NAME: &str = "openai/gpt-4.1";

#[derive(Serialize)]
struct ReqMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct RespMessage {
    content: String,
}

#[derive(Deserialize)]
struct RespChoice {
    message: RespMessage,
}

#[derive(Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<RespChoice>,
}

pub async fn request(messages: &[Message]) -> Result<String> {
    let req_messages: Vec<ReqMessage> = messages
        .iter()
        .map(|m| ReqMessage {
            role: m.role.clone(),
            content: m.content.clone(),
        })
        .collect();

    let mut last_error: Option<anyhow::Error> = None;
    for token in config::github_tokens() {
        match try_request(&token, &req_messages).await {
            Ok(content) => return Ok(content),
            Err(e) => last_error = Some(e),
        }
    }

    match last_error {
        Some(e) => Err(e),
        None => anyhow::bail!("没有可用的 GitHub token"),
    }
}

async fn try_request(token: &str, req_messages: &[ReqMessage]) -> Result<String> {
    let body = json!({
        "messages": req_messages,
        "temperature": 1.0,
        "top_p": 1.0,
        "model": MODEL_NAME,
    });

    let client = Client::new();
    let response = client
        .post(format!("{}/chat/completions", ENDPOINT))
        .header("Authorization", format!("Bearer {}", token))
        .json(&body)
        .send()
        .await
        .context("GPT-4.1 请求发送失败")?;

    let status = response.status();
    let text = response.text().await.context("读取 GPT-4.1 响应失败")?;
    if !status.is_success() {
        anyhow::bail!("GPT-4.1 请求失败: HTTP {} - {}", status, text);
    }

    let parsed: ChatCompletionResponse =
        serde_json::from_str(&text).with_context(|| format!("解析 GPT-4.1 响应失败: {}", text))?;
    let content = parsed
        .choices
        .into_iter()
        .next()
        .context("GPT-4.1 响应中无 choices")?
        .message
        .content;
    Ok(content)
}

pub async fn request_stream(messages: &[Message], tx: Sender<Result<String>>) -> Result<()> {
    let tokens: Vec<String> = config::github_tokens().into_iter().map(|s| s.to_string()).collect();
    models::stream_openai_compatible(
        ENDPOINT,
        MODEL_NAME,
        messages,
        tokens,
        json!({"temperature": 1.0, "top_p": 1.0}),
        tx,
    )
    .await
}
