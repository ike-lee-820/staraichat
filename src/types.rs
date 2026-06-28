use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConversationKind {
    Chat,
    Image,
    #[serde(other)]
    #[allow(dead_code)]
    Video,
}

impl ConversationKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ConversationKind::Chat => "文字对话",
            ConversationKind::Image => "图像生成",
            ConversationKind::Video => "视频生成",
        }
    }
}

impl Default for ConversationKind {
    fn default() -> Self {
        ConversationKind::Chat
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttachmentKind {
    Image,
    Video,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Attachment {
    pub kind: AttachmentKind,
    pub file_name: String,
    pub local_path: String,
    pub source_url: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Message {
    pub role: String,
    pub content: String,
    #[serde(default = "now")]
    pub timestamp: DateTime<Local>,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
}

fn now() -> DateTime<Local> {
    Local::now()
}

impl Message {
    pub fn new(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
            timestamp: Local::now(),
            attachments: Vec::new(),
        }
    }

    pub fn with_attachments(
        role: impl Into<String>,
        content: impl Into<String>,
        attachments: Vec<Attachment>,
    ) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
            timestamp: Local::now(),
            attachments,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub model: String,
    #[serde(default)]
    pub kind: ConversationKind,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
    pub messages: Vec<Message>,
}

impl Conversation {
    pub fn new(kind: ConversationKind, model: impl Into<String>) -> Self {
        let now = Local::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title: "新对话".to_string(),
            model: model.into(),
            kind,
            created_at: now,
            updated_at: now,
            messages: Vec::new(),
        }
    }

    pub fn add_message(&mut self, role: impl Into<String>, content: impl Into<String>) {
        self.messages.push(Message::new(role, content));
        self.updated_at = Local::now();
        self.ensure_title();
    }

    pub fn add_message_with_attachments(
        &mut self,
        role: impl Into<String>,
        content: impl Into<String>,
        attachments: Vec<Attachment>,
    ) {
        self.messages
            .push(Message::with_attachments(role, content, attachments));
        self.updated_at = Local::now();
        self.ensure_title();
    }

    pub fn add_assistant_placeholder(&mut self) {
        self.messages.push(Message::new("assistant", ""));
        self.updated_at = Local::now();
    }

    pub fn append_to_last_assistant(&mut self, content: &str) {
        if let Some(last) = self.messages.last_mut() {
            if last.role == "assistant" {
                last.content.push_str(content);
                self.updated_at = Local::now();
            }
        }
    }

    fn ensure_title(&mut self) {
        if self.title == "新对话" {
            if let Some(first) = self.messages.iter().find(|m| m.role == "user") {
                let text = first.content.trim();
                if !text.is_empty() {
                    let title: String = text.chars().take(30).collect();
                    self.title = title;
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelKind {
    Gpt4_1,
    DeepseekV3_0324,
    Llama4Scout,
    XunfeiSpark,
    Agnes,
}

impl ModelKind {
    pub fn all() -> &'static [ModelKind] {
        &[
            ModelKind::Gpt4_1,
            ModelKind::DeepseekV3_0324,
            ModelKind::Llama4Scout,
            ModelKind::XunfeiSpark,
            ModelKind::Agnes,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ModelKind::Gpt4_1 => "GPT-4.1",
            ModelKind::DeepseekV3_0324 => "DeepSeek-V3-0324",
            ModelKind::Llama4Scout => "Llama-4-Scout",
            ModelKind::XunfeiSpark => "Spark Lite",
            ModelKind::Agnes => "Agnes-2.0-Flash",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "GPT-4.1" => Some(ModelKind::Gpt4_1),
            "DeepSeek-V3-0324" => Some(ModelKind::DeepseekV3_0324),
            "Llama-4-Scout" => Some(ModelKind::Llama4Scout),
            "Spark Lite" | "讯飞星火" => Some(ModelKind::XunfeiSpark),
            "Agnes-2.0-Flash" => Some(ModelKind::Agnes),
            _ => None,
        }
    }
}
