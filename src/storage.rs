use crate::types::Conversation;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub struct Storage {
    dir: PathBuf,
    media_dir: PathBuf,
}

impl Storage {
    pub fn new(dir: impl AsRef<Path>, media_dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        let media_dir = media_dir.as_ref().to_path_buf();
        fs::create_dir_all(&dir).with_context(|| format!("无法创建存储目录: {}", dir.display()))?;
        fs::create_dir_all(&media_dir)
            .with_context(|| format!("无法创建媒体目录: {}", media_dir.display()))?;
        Ok(Self { dir, media_dir })
    }

    pub fn default_dirs() -> Result<(PathBuf, PathBuf)> {
        let data_dir = dirs::data_local_dir().context("无法获取本地数据目录")?;
        let base = data_dir.join("ai_chat");
        Ok((base.join("conversations"), base.join("media")))
    }

    pub fn conversation_media_dir(&self, conversation_id: &str) -> PathBuf {
        self.media_dir.join(conversation_id)
    }

    pub fn list(&self) -> Result<Vec<Conversation>> {
        let mut conversations = Vec::new();
        if !self.dir.exists() {
            return Ok(conversations);
        }
        for entry in fs::read_dir(&self.dir)
            .with_context(|| format!("无法读取存储目录: {}", self.dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let content = fs::read_to_string(&path)
                    .with_context(|| format!("无法读取文件: {}", path.display()))?;
                let conv: Conversation = serde_json::from_str(&content)
                    .with_context(|| format!("JSON解析失败: {}", path.display()))?;
                conversations.push(conv);
            }
        }
        conversations.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(conversations)
    }

    pub fn save(&self, conv: &Conversation) -> Result<()> {
        let path = self.dir.join(format!("{}.json", conv.id));
        let content = serde_json::to_string_pretty(conv).context("序列化对话失败")?;
        fs::write(&path, content).with_context(|| format!("无法写入文件: {}", path.display()))?;
        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        let path = self.dir.join(format!("{}.json", id));
        if path.exists() {
            fs::remove_file(&path).with_context(|| format!("无法删除文件: {}", path.display()))?;
        }
        let media = self.conversation_media_dir(id);
        if media.exists() {
            fs::remove_dir_all(&media)
                .with_context(|| format!("无法删除媒体目录: {}", media.display()))?;
        }
        Ok(())
    }
}
