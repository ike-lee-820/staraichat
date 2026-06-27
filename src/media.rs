use anyhow::{bail, Context, Result};
use reqwest::Client;
use std::fs;
use std::path::{Path, PathBuf};

fn extension_from_content_type(content_type: &str) -> &str {
    if content_type.contains("image/png") {
        "png"
    } else if content_type.contains("image/jpeg") || content_type.contains("image/jpg") {
        "jpg"
    } else if content_type.contains("image/webp") {
        "webp"
    } else if content_type.contains("video/mp4") {
        "mp4"
    } else if content_type.contains("video/webm") {
        "webm"
    } else {
        "bin"
    }
}

pub async fn download_file(url: &str, dir: &Path, file_name: &str) -> Result<PathBuf> {
    fs::create_dir_all(dir)
        .with_context(|| format!("无法创建媒体目录: {}", dir.display()))?;

    let client = Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("下载文件失败: {}", url))?;

    let status = response.status();
    if !status.is_success() {
        bail!("下载文件失败: HTTP {} - {}", status, url);
    }

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream");
    let ext = extension_from_content_type(content_type);

    let path = dir.join(format!("{}.{}", file_name, ext));
    let bytes = response.bytes().await
        .with_context(|| format!("读取下载内容失败: {}", url))?;
    fs::write(&path, bytes)
        .with_context(|| format!("写入媒体文件失败: {}", path.display()))?;

    Ok(path)
}
