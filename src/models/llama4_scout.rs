use crate::config;
use crate::types::Message;
use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

const ENDPOINT: &str = "https://models.github.ai/inference";
const MODEL_NAME: &str = "meta/Llama-4-Scout-17B-16E-Instruct";

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
        "max_tokens": 1000,
        "model": MODEL_NAME,
    });

    let client = Client::new();
    let response = client
        .post(format!("{}/chat/completions", ENDPOINT))
        .header("Authorization", format!("Bearer {}", token))
        .json(&body)
        .send()
        .await
        .context("Llama-4-Scout 请求发送失败")?;

    let status = response.status();
    let text = response.text().await.context("读取 Llama-4-Scout 响应失败")?;
    if !status.is_success() {
        anyhow::bail!("Llama-4-Scout 请求失败: HTTP {} - {}", status, text);
    }

    let parsed: ChatCompletionResponse = serde_json::from_str(&text)
        .with_context(|| format!("解析 Llama-4-Scout 响应失败: {}", text))?;
    let content = parsed
        .choices
        .into_iter()
        .next()
        .context("Llama-4-Scout 响应中无 choices")?
        .message
        .content;
    Ok(content)
}
