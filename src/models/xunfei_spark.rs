use crate::config;
use crate::types::Message;
use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::Sha256;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use url::Url;

// Spark Lite 配置
const HOST: &str = "spark-api.xf-yun.com";
const PATH: &str = "/v1.1/chat";
const DOMAIN: &str = "lite";

#[derive(Serialize)]
struct TextItem {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct RespHeader {
    code: i32,
    message: String,
    #[serde(rename = "status")]
    _status: i32,
}

#[derive(Deserialize)]
struct RespTextItem {
    content: String,
    role: String,
    #[serde(rename = "index")]
    _index: i32,
}

#[derive(Deserialize)]
struct RespChoices {
    status: i32,
    #[serde(rename = "seq")]
    _seq: i32,
    text: Vec<RespTextItem>,
}

#[derive(Deserialize)]
struct RespPayload {
    choices: RespChoices,
}

#[derive(Deserialize)]
struct SparkResponse {
    header: RespHeader,
    payload: Option<RespPayload>,
}

type HmacSha256 = Hmac<Sha256>;

fn build_auth_url(_app_id: &str, api_key: &str, api_secret: &str) -> Result<Url> {
    let date: DateTime<Utc> = Utc::now();
    let date_str = date.to_rfc2822();

    let signature_origin = format!("host: {}\ndate: {}\nGET {} HTTP/1.1", HOST, date_str, PATH);

    let mut mac = HmacSha256::new_from_slice(api_secret.as_bytes()).context("创建 HMAC 失败")?;
    mac.update(signature_origin.as_bytes());
    let signature = BASE64.encode(mac.finalize().into_bytes());

    let authorization_origin = format!(
        "api_key=\"{}\", algorithm=\"hmac-sha256\", headers=\"host date request-line\", signature=\"{}\"",
        api_key, signature
    );
    let authorization = BASE64.encode(authorization_origin.as_bytes());

    let mut url =
        Url::parse(&format!("wss://{}{}", HOST, PATH)).context("解析 WebSocket URL 失败")?;
    url.query_pairs_mut()
        .append_pair("authorization", &authorization)
        .append_pair("date", &date_str)
        .append_pair("host", HOST);
    Ok(url)
}

fn build_request_body(messages: &[Message]) -> Result<serde_json::Value> {
    let app_id = config::xunfei_app_id();

    // Spark Lite 不支持 system 角色，将 system 消息跳过或合并到 user 中
    let text: Vec<TextItem> = messages
        .iter()
        .filter(|m| m.role == "user" || m.role == "assistant")
        .map(|m| TextItem {
            role: m.role.clone(),
            content: m.content.clone(),
        })
        .collect();

    Ok(json!({
        "header": {
            "app_id": app_id,
            "uid": "ai_chat_user"
        },
        "parameter": {
            "chat": {
                "domain": DOMAIN,
                "temperature": 0.5,
                "max_tokens": 4096
            }
        },
        "payload": {
            "message": {
                "text": text
            }
        }
    }))
}

pub async fn request(messages: &[Message]) -> Result<String> {
    let app_id = config::xunfei_app_id();
    let api_key = config::xunfei_api_key();
    let api_secret = config::xunfei_api_secret();

    let url = build_auth_url(&app_id, &api_key, &api_secret)?;
    let request_body = build_request_body(messages)?;

    let (mut ws_stream, _) = tokio_tungstenite::connect_async(url.to_string())
        .await
        .context("连接讯飞星火 WebSocket 失败")?;

    ws_stream
        .send(WsMessage::Text(request_body.to_string().into()))
        .await
        .context("发送讯飞星火请求失败")?;

    let mut full_content = String::new();
    let timeout = tokio::time::Duration::from_secs(120);

    loop {
        let msg = tokio::time::timeout(timeout, ws_stream.next())
            .await
            .context("讯飞星火响应超时")?
            .context("读取讯飞星火响应失败")??;

        match msg {
            WsMessage::Text(text) => {
                let resp: SparkResponse = serde_json::from_str(&text)
                    .with_context(|| format!("解析讯飞星火响应失败: {}", text))?;

                if resp.header.code != 0 {
                    bail!(
                        "讯飞星火返回错误: {} - {}",
                        resp.header.code,
                        resp.header.message
                    );
                }

                if let Some(payload) = resp.payload {
                    for item in payload.choices.text {
                        if item.role == "assistant" {
                            full_content.push_str(&item.content);
                        }
                    }
                    if payload.choices.status == 2 {
                        break;
                    }
                }
            }
            WsMessage::Close(_) => break,
            _ => {}
        }
    }

    Ok(full_content)
}

pub async fn request_stream(
    messages: &[Message],
    tx: tokio::sync::mpsc::Sender<Result<String>>,
) -> Result<()> {
    let app_id = config::xunfei_app_id();
    let api_key = config::xunfei_api_key();
    let api_secret = config::xunfei_api_secret();

    let url = build_auth_url(&app_id, &api_key, &api_secret)?;
    let request_body = build_request_body(messages)?;

    let (mut ws_stream, _) = tokio_tungstenite::connect_async(url.to_string())
        .await
        .context("连接讯飞星火 WebSocket 失败")?;

    ws_stream
        .send(WsMessage::Text(request_body.to_string().into()))
        .await
        .context("发送讯飞星火请求失败")?;

    let timeout = tokio::time::Duration::from_secs(120);

    loop {
        let msg = tokio::time::timeout(timeout, ws_stream.next())
            .await
            .context("讯飞星火响应超时")?
            .context("读取讯飞星火响应失败")??;

        match msg {
            WsMessage::Text(text) => {
                let resp: SparkResponse = serde_json::from_str(&text)
                    .with_context(|| format!("解析讯飞星火响应失败: {}", text))?;

                if resp.header.code != 0 {
                    bail!(
                        "讯飞星火返回错误: {} - {}",
                        resp.header.code,
                        resp.header.message
                    );
                }

                if let Some(payload) = resp.payload {
                    let status = payload.choices.status;
                    for item in payload.choices.text {
                        if item.role == "assistant" && !item.content.is_empty() {
                            if tx.send(Ok(item.content)).await.is_err() {
                                return Ok(());
                            }
                        }
                    }
                    if status == 2 {
                        break;
                    }
                }
            }
            WsMessage::Close(_) => break,
            _ => {}
        }
    }

    Ok(())
}
