use crate::types::{Message, ModelKind};
use anyhow::Result;
use futures_util::StreamExt;
use serde_json::json;
use tokio::sync::mpsc::Sender;

pub mod agnes;
pub mod deepseek_v3_0324;
pub mod gpt4_1;
pub mod image_generation;
pub mod llama4_scout;
pub mod xunfei_spark;

pub async fn request(model: ModelKind, messages: &[Message]) -> Result<String> {
    match model {
        ModelKind::Gpt4_1 => gpt4_1::request(messages).await,
        ModelKind::DeepseekV3_0324 => deepseek_v3_0324::request(messages).await,
        ModelKind::Llama4Scout => llama4_scout::request(messages).await,
        ModelKind::XunfeiSpark => xunfei_spark::request(messages).await,
        ModelKind::Agnes => agnes::request(messages).await,
    }
}

pub async fn request_stream(
    model: ModelKind,
    messages: &[Message],
    tx: Sender<Result<String>>,
) -> Result<()> {
    match model {
        ModelKind::Gpt4_1 => gpt4_1::request_stream(messages, tx).await,
        ModelKind::DeepseekV3_0324 => deepseek_v3_0324::request_stream(messages, tx).await,
        ModelKind::Llama4Scout => llama4_scout::request_stream(messages, tx).await,
        ModelKind::XunfeiSpark => xunfei_spark::request_stream(messages, tx).await,
        ModelKind::Agnes => agnes::request_stream(messages, tx).await,
    }
}

/// 通用 OpenAI 兼容 SSE 流式请求辅助函数，支持 token 回退。
pub async fn stream_openai_compatible(
    endpoint: &str,
    model: &str,
    messages: &[Message],
    tokens: Vec<String>,
    extra_body: serde_json::Value,
    tx: Sender<Result<String>>,
) -> Result<()> {
    let req_messages: Vec<serde_json::Value> = messages
        .iter()
        .map(|m| json!({"role": m.role, "content": m.content}))
        .collect();
    let client = reqwest::Client::new();
    let mut last_error: Option<anyhow::Error> = None;

    for token in tokens {
        let mut body = json!({
            "model": model,
            "messages": req_messages,
            "stream": true,
        });
        if let Some(obj) = body.as_object_mut() {
            if let Some(extra) = extra_body.as_object() {
                for (k, v) in extra {
                    obj.insert(k.clone(), v.clone());
                }
            }
        }

        let response = client
            .post(format!("{}/chat/completions", endpoint))
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await;

        match response {
            Ok(resp) => {
                let status = resp.status();
                if !status.is_success() {
                    let text = resp.text().await.unwrap_or_default();
                    last_error = Some(anyhow::anyhow!("HTTP {} - {}", status, text));
                    continue;
                }

                let mut stream = resp.bytes_stream();
                let mut buffer = String::new();
                while let Some(chunk) = stream.next().await {
                    let bytes = match chunk {
                        Ok(b) => b,
                        Err(e) => {
                            let _ = tx.send(Err(anyhow::anyhow!("流读取失败: {}", e))).await;
                            return Ok(());
                        }
                    };
                    buffer.push_str(&String::from_utf8_lossy(&bytes));

                    while let Some(pos) = buffer.find('\n') {
                        let line = buffer[..pos].trim().to_string();
                        buffer = buffer[pos + 1..].to_string();
                        if !line.starts_with("data: ") {
                            continue;
                        }
                        let data = &line[6..];
                        if data == "[DONE]" {
                            continue;
                        }
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data) {
                            if let Some(content) =
                                parsed["choices"][0]["delta"]["content"].as_str()
                            {
                                if tx.send(Ok(content.to_string())).await.is_err() {
                                    return Ok(());
                                }
                            }
                        }
                    }
                }
                return Ok(());
            }
            Err(e) => last_error = Some(e.into()),
        }
    }

    match last_error {
        Some(e) => Err(e),
        None => anyhow::bail!("没有可用的 token"),
    }
}
