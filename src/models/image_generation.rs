use crate::config;
use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

const ENDPOINT: &str = "https://apihub.agnes-ai.com/v1/images/generations";

#[derive(Serialize)]
struct ImageGenerationRequest {
    model: String,
    prompt: String,
    size: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    n: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    image: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    return_base64: Option<bool>,
}

#[derive(Deserialize)]
struct ImageData {
    url: Option<String>,
    #[allow(dead_code)]
    b64_json: Option<String>,
}

#[derive(Deserialize)]
struct ImageGenerationResponse {
    data: Vec<ImageData>,
}

pub async fn request(
    model: &str,
    prompt: &str,
    size: &str,
    n: u32,
    image_urls: Option<Vec<String>>,
) -> Result<Vec<String>> {
    let api_key = config::agnes_api_key();

    let body = ImageGenerationRequest {
        model: model.to_string(),
        prompt: prompt.to_string(),
        size: size.to_string(),
        n: if n > 1 { Some(n) } else { None },
        image: image_urls,
        return_base64: Some(false),
    };

    let client = Client::new();
    let response = client
        .post(ENDPOINT)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .context("图像生成请求发送失败")?;

    let status = response.status();
    let text = response.text().await.context("读取图像生成响应失败")?;
    if !status.is_success() {
        bail!("图像生成请求失败: HTTP {} - {}", status, text);
    }

    let parsed: ImageGenerationResponse =
        serde_json::from_str(&text).with_context(|| format!("解析图像生成响应失败: {}", text))?;

    let urls: Vec<String> = parsed.data.into_iter().filter_map(|d| d.url).collect();

    if urls.is_empty() {
        bail!("图像生成响应中无 URL");
    }

    Ok(urls)
}
