use crate::config;
use crate::types::{AttachmentKind, Message};
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::Path;

const ENDPOINT: &str = "https://apihub.agnes-ai.com/v1";
const MODEL_NAME: &str = "agnes-2.0-flash";

#[derive(Serialize)]
#[serde(untagged)]
enum Content {
    Text(String),
    Parts(Vec<ContentPart>),
}

#[derive(Serialize)]
#[serde(tag = "type")]
enum ContentPart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: ImageUrlRef },
}

#[derive(Serialize)]
struct ImageUrlRef {
    url: String,
}

#[derive(Serialize)]
struct ReqMessage {
    role: String,
    content: Content,
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

#[derive(Deserialize)]
struct StreamDelta {
    content: Option<String>,
}

#[derive(Deserialize)]
struct StreamChoice {
    delta: StreamDelta,
    #[serde(default)]
    #[allow(dead_code)]
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct StreamChunkResponse {
    choices: Vec<StreamChoice>,
}

fn file_to_data_uri(path: &str) -> Result<String> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("读取图片失败: {}", path))?;
    let ext = Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("png");
    let mime = match ext.to_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => "image/png",
    };
    Ok(format!("data:{};base64,{}", mime, BASE64.encode(&bytes)))
}

fn build_req_messages(messages: &[Message]) -> Vec<ReqMessage> {
    messages
        .iter()
        .map(|m| {
            let image_parts: Vec<_> = m
                .attachments
                .iter()
                .filter(|a| a.kind == AttachmentKind::Image)
                .filter_map(|a| match file_to_data_uri(&a.local_path) {
                    Ok(uri) => Some(ContentPart::ImageUrl {
                        image_url: ImageUrlRef { url: uri },
                    }),
                    Err(_) => None,
                })
                .collect();

            if image_parts.is_empty() {
                ReqMessage {
                    role: m.role.clone(),
                    content: Content::Text(m.content.clone()),
                }
            } else {
                let mut parts = vec![ContentPart::Text {
                    text: m.content.clone(),
                }];
                parts.extend(image_parts);
                ReqMessage {
                    role: m.role.clone(),
                    content: Content::Parts(parts),
                }
            }
        })
        .collect()
}

fn build_body(messages: &[Message], stream: bool) -> serde_json::Value {
    let req_messages = build_req_messages(messages);
    json!({
        "model": MODEL_NAME,
        "messages": req_messages,
        "temperature": 0.7,
        "max_tokens": 4096,
        "stream": stream,
    })
}

pub async fn request(messages: &[Message]) -> Result<String> {
    let api_key = config::agnes_api_key();
    let body = build_body(messages, false);

    let client = Client::new();
    let response = client
        .post(format!("{}/chat/completions", ENDPOINT))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .context("Agnes 请求发送失败")?;

    let status = response.status();
    let text = response.text().await.context("读取 Agnes 响应失败")?;
    if !status.is_success() {
        anyhow::bail!("Agnes 请求失败: HTTP {} - {}", status, text);
    }

    let parsed: ChatCompletionResponse = serde_json::from_str(&text)
        .with_context(|| format!("解析 Agnes 响应失败: {}", text))?;
    let content = parsed
        .choices
        .into_iter()
        .next()
        .context("Agnes 响应中无 choices")?
        .message
        .content;
    Ok(content)
}

pub async fn request_stream(
    messages: &[Message],
    tx: tokio::sync::mpsc::Sender<Result<String>>,
) -> Result<()> {
    let api_key = config::agnes_api_key();
    let body = build_body(messages, true);

    let client = Client::new();
    let response = client
        .post(format!("{}/chat/completions", ENDPOINT))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .context("Agnes 流式请求发送失败")?;

    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        anyhow::bail!("Agnes 流式请求失败: HTTP {} - {}", status, text);
    }

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.context("读取 Agnes 流式数据失败")?;
        buffer.push_str(&String::from_utf8_lossy(&bytes));

        while let Some(pos) = buffer.find('\n') {
            let line = buffer[..pos].trim().to_string();
            buffer = buffer[pos + 1..].to_string();

            if line.is_empty() || !line.starts_with("data: ") {
                continue;
            }

            let data = &line[6..];
            if data == "[DONE]" {
                return Ok(());
            }

            match serde_json::from_str::<StreamChunkResponse>(data) {
                Ok(resp) => {
                    for choice in resp.choices {
                        if let Some(content) = choice.delta.content {
                            if tx.send(Ok(content)).await.is_err() {
                                return Ok(());
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(Err(anyhow::anyhow!("解析流式响应失败: {} - {}", e, data))).await;
                    return Ok(());
                }
            }
        }
    }

    Ok(())
}
