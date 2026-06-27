use crate::config;
use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const CREATE_ENDPOINT: &str = "https://apihub.agnes-ai.com/v1/videos";
const POLL_ENDPOINT: &str = "https://apihub.agnes-ai.com/agnesapi";
const MODEL_NAME: &str = "agnes-video-v2.0";
const POLL_INTERVAL: Duration = Duration::from_secs(3);
const POLL_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Serialize)]
struct VideoGenerationRequest {
    model: String,
    prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<String>,
    height: i32,
    width: i32,
    num_frames: i32,
    frame_rate: i32,
}

#[derive(Deserialize)]
struct VideoGenerationResponse {
    #[allow(dead_code)]
    id: String,
    #[serde(rename = "task_id")]
    #[allow(dead_code)]
    task_id: String,
    #[serde(rename = "video_id")]
    video_id: String,
    status: String,
}

#[derive(Deserialize)]
struct VideoResultResponse {
    status: String,
    #[serde(rename = "remixed_from_video_id")]
    video_url: Option<String>,
    error: Option<serde_json::Value>,
}

pub async fn request(
    prompt: &str,
    width: i32,
    height: i32,
    num_frames: i32,
    frame_rate: i32,
    image_url: Option<String>,
    mode: Option<String>,
) -> Result<String> {
    let api_key = config::agnes_api_key();

    // 服务端只接受纯 base64 字符串，需要去掉 data URI 前缀
    let image = image_url.map(|url| {
        url.split_once(',')
            .map(|(_, b64)| b64.to_string())
            .unwrap_or(url)
    });

    let body = VideoGenerationRequest {
        model: MODEL_NAME.to_string(),
        prompt: prompt.to_string(),
        image,
        mode,
        height,
        width,
        num_frames,
        frame_rate,
    };

    let client = Client::new();
    let response = client
        .post(CREATE_ENDPOINT)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .context("视频生成任务提交失败")?;

    let status = response.status();
    let text = response.text().await.context("读取视频生成提交响应失败")?;
    if !status.is_success() {
        bail!("视频生成提交失败: HTTP {} - {}", status, text);
    }

    let submit: VideoGenerationResponse = serde_json::from_str(&text)
        .with_context(|| format!("解析视频生成提交响应失败: {}", text))?;

    if submit.status == "failed" {
        bail!("视频生成任务提交即失败");
    }

    let deadline = tokio::time::Instant::now() + POLL_TIMEOUT;
    loop {
        if tokio::time::Instant::now() > deadline {
            bail!("视频生成任务轮询超时");
        }
        tokio::time::sleep(POLL_INTERVAL).await;

        let query = client
            .get(POLL_ENDPOINT)
            .query(&[("video_id", submit.video_id.as_str())])
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await
            .context("查询视频生成结果失败")?;

        let status = query.status();
        let text = query.text().await.context("读取视频生成结果失败")?;
        if !status.is_success() {
            bail!("查询视频生成结果失败: HTTP {} - {}", status, text);
        }

        let result: VideoResultResponse = serde_json::from_str(&text)
            .with_context(|| format!("解析视频生成结果失败: {}", text))?;

        match result.status.as_str() {
            "completed" => {
                return result.video_url.context("视频生成成功但无 URL");
            }
            "failed" => {
                let err = result.error.map(|e| e.to_string()).unwrap_or_default();
                bail!("视频生成任务失败: {}", err);
            }
            _ => {}
        }
    }
}
