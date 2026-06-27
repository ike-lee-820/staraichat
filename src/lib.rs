pub mod config;
pub mod latex;
pub mod markdown;
pub mod media;
pub mod models;
pub mod secrets;
pub mod storage;
pub mod types;

#[cfg(target_os = "android")]
pub mod android_file_picker;

use crate::storage::Storage;
use crate::types::{
    Attachment, AttachmentKind, Conversation, ConversationKind, ModelKind,
};
use anyhow::Result;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chrono::Local;
use eframe::NativeOptions;
use egui::{CentralPanel, ColorImage, Context, RichText, SidePanel, TextureHandle, TopBottomPanel, Ui, Vec2, ViewportBuilder, Widget};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Command;
use tokio::sync::mpsc;

enum Screen {
    MainMenu,
    Mode(ConversationKind),
}

#[allow(dead_code)]
enum AsyncEvent {
    ChatReply { conv_id: String, result: Result<String> },
    Generated { conv_id: String, result: Result<Vec<Attachment>> },
    FileUploaded(PendingAttachment),
    FileDownloaded { file_name: String, result: Result<()> },
    StreamChunk { conv_id: String, content: String },
    StreamDone { conv_id: String, result: Result<()> },
}

enum MessageAction {
    Delete { conv_id: String, idx: usize },
    Regenerate { conv_id: String, idx: usize },
}

#[derive(Clone)]
enum PendingAttachment {
    Text { name: String, content: String },
    Image { name: String, path: String },
}

pub struct App {
    storage: Storage,
    runtime: tokio::runtime::Runtime,
    screen: Screen,
    conversations: Vec<Conversation>,
    selected_id: Option<String>,
    input: String,
    generating_ids: HashSet<String>,
    error: Option<String>,
    image_textures: HashMap<String, TextureHandle>,
    to_delete: Option<String>,
    pending_attachments: Vec<PendingAttachment>,
    narrow_sidebar_open: bool,
    image_model: String,
    image_size: String,
    image_n: u32,
    video_width: i32,
    video_height: i32,
    video_num_frames: i32,
    video_frame_rate: i32,
    video_mode: String,
    preview_attachment: Option<Attachment>,
    pending_copy: Option<String>,
    pending_message_action: Option<MessageAction>,
    rx: mpsc::Receiver<AsyncEvent>,
    tx: mpsc::Sender<AsyncEvent>,
}

fn load_font_data(filename: &str) -> Vec<u8> {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();

    // 1. 与 exe 同级的 fonts 子目录
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
    {
        candidates.push(dir.join("fonts").join(filename));
        candidates.push(dir.join(filename));
    }

    // 2. 当前工作目录（cargo run 时通常是项目根目录）
    if let Ok(dir) = std::env::current_dir() {
        candidates.push(dir.join("fonts").join(filename));
        candidates.push(dir.join(filename));
    }

    for path in &candidates {
        if let Ok(bytes) = std::fs::read(path) {
            return bytes;
        }
    }

    // 回退到编译时嵌入的版本（字体位于 crate 内的 fonts/ 目录）
    match filename {
        "a.otf" => include_bytes!("../fonts/a.otf").to_vec(),
        "b.otf" => include_bytes!("../fonts/b.otf").to_vec(),
        "dk.ttf" => include_bytes!("../fonts/dk.ttf").to_vec(),
        _ => Vec::new(),
    }
}

fn setup_fonts(ctx: &Context) {
    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        "Regular".to_owned(),
        egui::FontData::from_owned(load_font_data("a.otf")),
    );
    fonts.font_data.insert(
        "Bold".to_owned(),
        egui::FontData::from_owned(load_font_data("b.otf")),
    );
    fonts.font_data.insert(
        "Code".to_owned(),
        egui::FontData::from_owned(load_font_data("dk.ttf")),
    );

    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "Regular".to_owned());
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, "Code".to_owned());
    fonts
        .families
        .entry(egui::FontFamily::Name("Bold".into()))
        .or_default()
        .insert(0, "Bold".to_owned());

    ctx.set_fonts(fonts);
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_fonts(&cc.egui_ctx);

        let mut style = (*cc.egui_ctx.style()).clone();
        style.text_styles = [
            (
                egui::TextStyle::Heading,
                egui::FontId::new(28.0, egui::FontFamily::Name("Bold".into())),
            ),
            (
                egui::TextStyle::Body,
                egui::FontId::new(18.0, egui::FontFamily::Proportional),
            ),
            (
                egui::TextStyle::Monospace,
                egui::FontId::new(16.0, egui::FontFamily::Monospace),
            ),
            (
                egui::TextStyle::Button,
                egui::FontId::new(19.0, egui::FontFamily::Name("Bold".into())),
            ),
            (
                egui::TextStyle::Small,
                egui::FontId::new(15.0, egui::FontFamily::Proportional),
            ),
        ]
        .into();
        cc.egui_ctx.set_style(style);

        let runtime = tokio::runtime::Runtime::new().expect("创建 tokio runtime 失败");
        let (storage_dir, media_dir) = Storage::default_dirs().expect("获取存储目录失败");
        let storage = Storage::new(&storage_dir, &media_dir).expect("初始化存储失败");
        let conversations = storage.list().unwrap_or_default();
        let (tx, rx) = mpsc::channel(100);

        Self {
            screen: Screen::MainMenu,
            selected_id: None,
            conversations,
            storage,
            runtime,
            input: String::new(),
            generating_ids: HashSet::new(),
            error: None,
            image_textures: HashMap::new(),
            to_delete: None,
            pending_attachments: Vec::new(),
            narrow_sidebar_open: false,
            image_model: "agnes-image-2.0-flash".to_string(),
            image_size: "1024x1024".to_string(),
            image_n: 1,
            video_width: 1152,
            video_height: 768,
            video_num_frames: 121,
            video_frame_rate: 24,
            video_mode: String::new(),
            preview_attachment: None,
            pending_copy: None,
            pending_message_action: None,
            rx,
            tx,
        }
    }

    fn selected_index(&self) -> Option<usize> {
        self.selected_id.as_ref().and_then(|id| {
            self.conversations.iter().position(|c| &c.id == id)
        })
    }

    fn selected_conversation(&self) -> Option<&Conversation> {
        self.selected_index().map(|idx| &self.conversations[idx])
    }

    #[cfg(not(target_os = "android"))]
    fn download_attachment(&self, att: &Attachment) {
        if let Some(dest) = rfd::FileDialog::new().set_file_name(&att.file_name).save_file() {
            if let Err(e) = std::fs::copy(&att.local_path, dest) {
                eprintln!("下载文件失败: {}", e);
            }
        }
    }
    #[cfg(target_os = "android")]
    fn download_attachment(&self, att: &Attachment) {
        let local_path = att.local_path.clone();
        let file_name = att.file_name.clone();
        let file_name_for_event = file_name.clone();
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let result: Result<()> = async {
                tokio::task::spawn_blocking(move || {
                    use anyhow::Context;
                    let uri = android_file_picker::save_file(&file_name)?
                        .context("用户取消保存")?;
                    let data = std::fs::read(&local_path)?;
                    android_file_picker::write_uri(&uri, &data)?;
                    Ok(())
                })
                .await
                .map_err(|e| anyhow::anyhow!("下载任务失败: {}", e))?
            }
            .await;
            let _ = tx.send(AsyncEvent::FileDownloaded { file_name: file_name_for_event, result }).await;
        });
    }

    fn open_with_system(&self, path: &str) {
        let _ = Command::new("cmd")
            .args(["/c", "start", "", path])
            .spawn();
    }

    fn render_preview_window(&mut self, ctx: &Context) {
        let mut open = self.preview_attachment.is_some();
        let att = self.preview_attachment.clone();
        if let Some(att) = att {
            let screen = ctx.screen_rect();
            let preview_size = (screen.size() * 0.85).min(Vec2::new(600.0, 600.0));
            egui::Window::new("预览")
                .open(&mut open)
                .default_size(preview_size)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("下载").clicked() {
                            self.download_attachment(&att);
                        }
                        if ui.button("打开").clicked() {
                            self.open_with_system(&att.local_path);
                        }
                    });
                    ui.separator();
                    if att.kind == AttachmentKind::Image {
                        if let Some(texture) = self.load_texture(ctx, &att.local_path) {
                            let size = texture.size_vec2();
                            let available = ui.available_size();
                            let scale = (available.x / size.x).min(available.y / size.y).min(1.0);
                            ui.image((texture.id(), size * scale));
                        } else {
                            ui.label("图片加载失败");
                        }
                    } else {
                        ui.label(format!("视频文件: {}", att.file_name));
                        if ui.button("播放视频").clicked() {
                            self.open_with_system(&att.local_path);
                        }
                    }
                });
        }
        if !open {
            self.preview_attachment = None;
        }
    }

    fn is_generating(&self, conv_id: &str) -> bool {
        self.generating_ids.contains(conv_id)
    }

    fn selected_generating(&self) -> bool {
        self.selected_id
            .as_ref()
            .map(|id| self.is_generating(id))
            .unwrap_or(false)
    }

    fn filtered_indices(&self, kind: ConversationKind) -> Vec<usize> {
        self.conversations
            .iter()
            .enumerate()
            .filter(|(_, c)| c.kind == kind)
            .map(|(i, _)| i)
            .collect()
    }

    fn create_conversation(&mut self, kind: ConversationKind, model: impl Into<String>) {
        let mut conv = Conversation::new(kind, model);
        if kind == ConversationKind::Chat {
            conv.add_message("system", "You are a helpful assistant.");
        }
        if let Err(e) = self.storage.save(&conv) {
            self.error = Some(format!("保存失败: {}", e));
            return;
        }
        self.selected_id = Some(conv.id.clone());
        self.conversations.push(conv);
        self.conversations.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    }

    fn delete_conversation(&mut self, id: &str) {
        if let Err(e) = self.storage.delete(id) {
            self.error = Some(format!("删除失败: {}", e));
            return;
        }
        self.conversations.retain(|c| c.id != id);
        if self.selected_id.as_deref() == Some(id) {
            self.selected_id = None;
        }
    }

    fn enter_mode(&mut self, kind: ConversationKind) {
        self.screen = Screen::Mode(kind);
        let indices = self.filtered_indices(kind);
        if indices.is_empty() {
            match kind {
                ConversationKind::Chat => self.create_conversation(kind, ModelKind::Agnes.as_str()),
                ConversationKind::Image => self.create_conversation(kind, "agnes-image-2.0-flash"),
                ConversationKind::Video => self.create_conversation(kind, "agnes-video-v2.0"),
            }
        } else {
            self.selected_id = Some(self.conversations[indices[0]].id.clone());
        }
        self.input.clear();
        self.pending_attachments.clear();
        self.error = None;
    }

    fn split_think_content(content: &str) -> (Option<String>, String) {
        let start_tag = "<think>";
        let end_tag = "</think>";
        if let Some(start) = content.find(start_tag) {
            if let Some(end) = content.find(end_tag) {
                let think = content[start + start_tag.len()..end].trim().to_string();
                let before = &content[..start];
                let after = &content[end + end_tag.len()..];
                let main = format!("{}{}", before, after).trim().to_string();
                return (Some(think), main);
            }
        }
        (None, content.to_string())
    }

    fn file_to_data_uri(path: &str) -> Result<String, String> {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
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

    fn send_chat_message(&mut self) {
        let input = self.input.trim().to_string();
        if input.is_empty() && self.pending_attachments.is_empty() {
            return;
        }
        let idx = match self.selected_index() {
            Some(i) => i,
            None => return,
        };

        let model = ModelKind::from_str(&self.conversations[idx].model).unwrap_or(ModelKind::Agnes);
        let mut content = input.clone();
        let mut image_attachments: Vec<Attachment> = Vec::new();

        for att in self.pending_attachments.drain(..) {
            match att {
                PendingAttachment::Text { name, content: text } => {
                    content.push_str(&format!("\n\n[文件 {} 内容]:\n{}", name, text));
                }
                PendingAttachment::Image { name, path } => {
                    if model == ModelKind::Agnes {
                        image_attachments.push(Attachment {
                            kind: AttachmentKind::Image,
                            file_name: name,
                            local_path: path,
                            source_url: String::new(),
                        });
                    } else {
                        content.push_str(&format!("\n\n[图片 {} 已附加，但当前模型不支持图片输入]", name));
                        image_attachments.push(Attachment {
                            kind: AttachmentKind::Image,
                            file_name: name,
                            local_path: path,
                            source_url: String::new(),
                        });
                    }
                }
            }
        }

        self.conversations[idx].add_message_with_attachments("user", content, image_attachments);
        let messages = self.conversations[idx].messages.clone();
        let conv_id = self.conversations[idx].id.clone();
        if let Err(e) = self.storage.save(&self.conversations[idx]) {
            self.error = Some(format!("保存失败: {}", e));
            return;
        }

        let tx = self.tx.clone();
        self.generating_ids.insert(conv_id.clone());
        self.input.clear();

        if model == ModelKind::Agnes {
            self.conversations[idx].add_assistant_placeholder();

            self.runtime.spawn(async move {
                let (chunk_tx, mut chunk_rx) = tokio::sync::mpsc::channel::<Result<String>>(128);
                let stream_future = async move {
                    models::agnes::request_stream(&messages, chunk_tx).await
                };
                let stream_task = tokio::spawn(stream_future);

                while let Some(result) = chunk_rx.recv().await {
                    match result {
                        Ok(content) => {
                            let _ = tx
                                .send(AsyncEvent::StreamChunk {
                                    conv_id: conv_id.clone(),
                                    content,
                                })
                                .await;
                        }
                        Err(e) => {
                            let _ = tx
                                .send(AsyncEvent::StreamDone {
                                    conv_id: conv_id.clone(),
                                    result: Err(e),
                                })
                                .await;
                            return;
                        }
                    }
                }

                let result = stream_task
                    .await
                    .unwrap_or_else(|e| Err(anyhow::anyhow!("流式任务异常: {}", e)));
                let _ = tx
                    .send(AsyncEvent::StreamDone { conv_id, result })
                    .await;
            });
        } else {
            self.runtime.spawn(async move {
                let result = models::request(model, &messages).await;
                let _ = tx.send(AsyncEvent::ChatReply { conv_id, result }).await;
            });
        }
    }

    fn generate_image(&mut self) {
        let prompt = self.input.trim().to_string();
        if prompt.is_empty() {
            return;
        }
        let idx = match self.selected_index() {
            Some(i) => i,
            None => return,
        };

        let model = self.image_model.clone();
        let size = self.image_size.clone();
        let n = self.image_n;
        let mut image_urls: Option<Vec<String>> = None;
        let mut display_attachments: Vec<Attachment> = Vec::new();
        for att in self.pending_attachments.drain(..) {
            if let PendingAttachment::Image { name, path } = att {
                display_attachments.push(Attachment {
                    kind: AttachmentKind::Image,
                    file_name: name,
                    local_path: path.clone(),
                    source_url: String::new(),
                });
                if let Ok(uri) = Self::file_to_data_uri(&path) {
                    image_urls.get_or_insert_with(Vec::new).push(uri);
                }
            }
        }

        self.conversations[idx].add_message_with_attachments("user", &prompt, display_attachments);
        self.conversations[idx].model = model.clone();
        let conv_id = self.conversations[idx].id.clone();
        let media_dir = self.storage.conversation_media_dir(&conv_id);
        if let Err(e) = self.storage.save(&self.conversations[idx]) {
            self.error = Some(format!("保存失败: {}", e));
            return;
        }

        let tx = self.tx.clone();
        self.generating_ids.insert(conv_id.clone());
        self.input.clear();

        self.runtime.spawn(async move {
            let result: Result<Vec<Attachment>> = async {
                let urls = models::image_generation::request(&model, &prompt, &size, n, image_urls).await?;
                let mut attachments = Vec::new();
                let ts = Local::now().format("%Y%m%d%H%M%S").to_string();
                for (idx, url) in urls.iter().enumerate() {
                    let file_name = format!("{}_{}", ts, idx);
                    let local_path = media::download_file(url, &media_dir, &file_name).await?;
                    attachments.push(Attachment {
                        kind: AttachmentKind::Image,
                        file_name: local_path.file_name().unwrap().to_string_lossy().to_string(),
                        local_path: local_path.to_string_lossy().to_string(),
                        source_url: url.clone(),
                    });
                }
                Ok(attachments)
            }.await;
            let _ = tx.send(AsyncEvent::Generated { conv_id, result }).await;
        });
    }

    fn generate_video(&mut self) {
        let prompt = self.input.trim().to_string();
        if prompt.is_empty() {
            return;
        }
        let idx = match self.selected_index() {
            Some(i) => i,
            None => return,
        };

        let width = self.video_width;
        let height = self.video_height;
        let num_frames = self.video_num_frames;
        let frame_rate = self.video_frame_rate;
        let mut mode = if self.video_mode.is_empty() {
            None
        } else {
            Some(self.video_mode.clone())
        };

        let mut image_url: Option<String> = None;
        let mut display_attachments: Vec<Attachment> = Vec::new();
        for att in self.pending_attachments.drain(..) {
            if let PendingAttachment::Image { name, path } = att {
                display_attachments.push(Attachment {
                    kind: AttachmentKind::Image,
                    file_name: name,
                    local_path: path.clone(),
                    source_url: String::new(),
                });
                if image_url.is_none() {
                    if let Ok(uri) = Self::file_to_data_uri(&path) {
                        image_url = Some(uri);
                    }
                }
            }
        }

        // 纯文本生成视频时不应发送需要参考图的 mode
        if image_url.is_none() {
            mode = None;
        }

        self.conversations[idx].add_message_with_attachments("user", &prompt, display_attachments);
        let conv_id = self.conversations[idx].id.clone();
        let media_dir = self.storage.conversation_media_dir(&conv_id);
        if let Err(e) = self.storage.save(&self.conversations[idx]) {
            self.error = Some(format!("保存失败: {}", e));
            return;
        }

        let tx = self.tx.clone();
        self.generating_ids.insert(conv_id.clone());
        self.input.clear();

        self.runtime.spawn(async move {
            let result: Result<Vec<Attachment>> = async {
                let url = models::video_generation::request(
                    &prompt,
                    width,
                    height,
                    num_frames,
                    frame_rate,
                    image_url,
                    mode,
                )
                .await?;
                let mut attachments = Vec::new();
                let ts = Local::now().format("%Y%m%d%H%M%S").to_string();
                let local_path = media::download_file(&url, &media_dir, &ts).await?;
                attachments.push(Attachment {
                    kind: AttachmentKind::Video,
                    file_name: local_path.file_name().unwrap().to_string_lossy().to_string(),
                    local_path: local_path.to_string_lossy().to_string(),
                    source_url: url,
                });
                Ok(attachments)
            }.await;
            let _ = tx.send(AsyncEvent::Generated { conv_id, result }).await;
        });
    }

    fn process_events(&mut self) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                AsyncEvent::FileUploaded(att) => {
                    self.pending_attachments.push(att);
                }
                AsyncEvent::FileDownloaded { file_name, result } => {
                    if let Err(e) = result {
                        self.error = Some(format!("下载 {} 失败: {}", file_name, e));
                    }
                }
                AsyncEvent::ChatReply { conv_id, result } => {
                    self.generating_ids.remove(&conv_id);
                    if let Some(idx) = self.conversations.iter().position(|c| c.id == conv_id) {
                        match result {
                            Ok(reply) => {
                                self.conversations[idx].add_message("assistant", reply);
                                if let Err(e) = self.storage.save(&self.conversations[idx]) {
                                    self.error = Some(format!("保存失败: {}", e));
                                }
                            }
                            Err(e) => self.error = Some(format!("请求失败: {}", e)),
                        }
                    }
                }
                AsyncEvent::Generated { conv_id, result } => {
                    self.generating_ids.remove(&conv_id);
                    if let Some(idx) = self.conversations.iter().position(|c| c.id == conv_id) {
                        match result {
                            Ok(attachments) => {
                                let content = if self.conversations[idx].kind == ConversationKind::Image {
                                    "图像生成完成"
                                } else {
                                    "视频生成完成"
                                };
                                self.conversations[idx].add_message_with_attachments("assistant", content, attachments);
                                if let Err(e) = self.storage.save(&self.conversations[idx]) {
                                    self.error = Some(format!("保存失败: {}", e));
                                }
                            }
                            Err(e) => self.error = Some(format!("生成失败: {}", e)),
                        }
                    }
                }
                AsyncEvent::StreamChunk { conv_id, content } => {
                    if let Some(idx) = self.conversations.iter().position(|c| c.id == conv_id) {
                        self.conversations[idx].append_to_last_assistant(&content);
                    }
                }
                AsyncEvent::StreamDone { conv_id, result } => {
                    self.generating_ids.remove(&conv_id);
                    if let Some(idx) = self.conversations.iter().position(|c| c.id == conv_id) {
                        if let Err(e) = result {
                            self.error = Some(format!("请求失败: {}", e));
                        } else if let Err(e) = self.storage.save(&self.conversations[idx]) {
                            self.error = Some(format!("保存失败: {}", e));
                        }
                    }
                }
            }
            self.conversations.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        }
    }

    fn process_pending_actions(&mut self, ctx: &Context) {
        if let Some(text) = self.pending_copy.take() {
            ctx.copy_text(text);
        }

        if let Some(action) = self.pending_message_action.take() {
            match action {
                MessageAction::Delete { conv_id, idx } => {
                    if let Some(i) = self.conversations.iter().position(|c| c.id == conv_id) {
                        if idx < self.conversations[i].messages.len() {
                            self.conversations[i].messages.remove(idx);
                            self.conversations[i].updated_at = Local::now();
                            if let Err(e) = self.storage.save(&self.conversations[i]) {
                                self.error = Some(format!("保存失败: {}", e));
                            }
                        }
                    }
                }
                MessageAction::Regenerate { conv_id, idx } => {
                    self.regenerate_message(conv_id, idx);
                }
            }
        }
    }

    fn regenerate_message(&mut self, conv_id: String, assistant_idx: usize) {
        if self.is_generating(&conv_id) {
            return;
        }
        let idx = match self.conversations.iter().position(|c| c.id == conv_id) {
            Some(i) => i,
            None => return,
        };
        if assistant_idx >= self.conversations[idx].messages.len() {
            return;
        }
        if self.conversations[idx].messages[assistant_idx].role != "assistant" {
            return;
        }

        self.conversations[idx].messages.truncate(assistant_idx);
        self.conversations[idx].updated_at = Local::now();

        let model = ModelKind::from_str(&self.conversations[idx].model)
            .unwrap_or(ModelKind::Agnes);
        let messages = self.conversations[idx].messages.clone();
        let tx = self.tx.clone();

        self.conversations[idx].add_assistant_placeholder();
        self.generating_ids.insert(conv_id.clone());

        if let Err(e) = self.storage.save(&self.conversations[idx]) {
            self.error = Some(format!("保存失败: {}", e));
            return;
        }

        if model == ModelKind::Agnes {
            self.runtime.spawn(async move {
                let (chunk_tx, mut chunk_rx) = tokio::sync::mpsc::channel::<Result<String>>(128);
                let stream_future = async move {
                    models::agnes::request_stream(&messages, chunk_tx).await
                };
                let stream_task = tokio::spawn(stream_future);

                while let Some(result) = chunk_rx.recv().await {
                    match result {
                        Ok(content) => {
                            let _ = tx
                                .send(AsyncEvent::StreamChunk {
                                    conv_id: conv_id.clone(),
                                    content,
                                })
                                .await;
                        }
                        Err(e) => {
                            let _ = tx
                                .send(AsyncEvent::StreamDone {
                                    conv_id: conv_id.clone(),
                                    result: Err(e),
                                })
                                .await;
                            return;
                        }
                    }
                }

                let result = stream_task
                    .await
                    .unwrap_or_else(|e| Err(anyhow::anyhow!("流式任务异常: {}", e)));
                let _ = tx
                    .send(AsyncEvent::StreamDone { conv_id, result })
                    .await;
            });
        } else {
            self.runtime.spawn(async move {
                let result = models::request(model, &messages).await;
                let _ = tx.send(AsyncEvent::ChatReply { conv_id, result }).await;
            });
        }
    }

    fn load_texture(&mut self, ctx: &Context, path: &str) -> Option<&TextureHandle> {
        if !self.image_textures.contains_key(path) {
            let image = image::open(path).ok()?;
            let image = image.to_rgba8();
            let size = [image.width() as _, image.height() as _];
            let pixels = image.as_raw();
            let color_image = ColorImage::from_rgba_unmultiplied(size, pixels);
            let texture = ctx.load_texture(path, color_image, Default::default());
            self.image_textures.insert(path.to_string(), texture);
        }
        self.image_textures.get(path)
    }

    #[cfg(not(target_os = "android"))]
    fn upload_file(&mut self) {
        if self.selected_id.is_none() {
            return;
        }

        let task = rfd::AsyncFileDialog::new()
            .add_filter("图片", &["png", "jpg", "jpeg", "webp", "gif"])
            .add_filter("文本", &["txt", "md", "rs", "py", "js", "ts", "json", "csv"])
            .add_filter("所有文件", &["*"])
            .pick_file();

        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            if let Some(handle) = task.await {
                let path = handle.path().to_string_lossy().to_string();
                let mut name = handle.file_name();
                if name.is_empty() {
                    name = Path::new(&path)
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                }
                let ext = Path::new(&path)
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase();

                let attachment = if ["png", "jpg", "jpeg", "webp", "gif"].contains(&ext.as_str()) {
                    PendingAttachment::Image { name, path }
                } else {
                    match tokio::fs::read_to_string(&path).await {
                        Ok(content) => PendingAttachment::Text { name, content },
                        Err(_) => PendingAttachment::Text {
                            name,
                            content: "[无法读取文件内容]".to_string(),
                        },
                    }
                };

                let _ = tx.send(AsyncEvent::FileUploaded(attachment)).await;
            }
        });
    }
    #[cfg(target_os = "android")]
    fn upload_file(&mut self) {
        if self.selected_id.is_none() {
            return;
        }
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            match tokio::task::spawn_blocking(|| android_file_picker::pick_file()).await {
                Ok(Ok(Some((name, path)))) => {
                    let ext = std::path::Path::new(&path)
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    let attachment = if ["png", "jpg", "jpeg", "webp", "gif"].contains(&ext.as_str()) {
                        PendingAttachment::Image { name, path }
                    } else {
                        match tokio::fs::read_to_string(&path).await {
                            Ok(content) => PendingAttachment::Text { name, content },
                            Err(_) => PendingAttachment::Text {
                                name,
                                content: "[无法读取文件内容]".to_string(),
                            },
                        }
                    };
                    let _ = tx.send(AsyncEvent::FileUploaded(attachment)).await;
                }
                Ok(Ok(None)) => {}
                Ok(Err(e)) => eprintln!("选择文件失败: {}", e),
                Err(e) => eprintln!("选择文件任务失败: {}", e),
            }
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.process_events();

        match &self.screen {
            Screen::MainMenu => self.render_main_menu(ctx),
            Screen::Mode(kind) => {
                let kind = *kind;
                self.render_mode(ctx, kind);
            }
        }

        self.render_preview_window(ctx);
        self.process_pending_actions(ctx);
    }
}

impl App {
    fn render_main_menu(&mut self, ctx: &Context) {
        CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                let narrow = ctx.screen_rect().width() < 600.0;
                let title_size = if narrow { 28.0 } else { 36.0 };
                let top_space = if narrow { 60.0 } else { 100.0 };
                let mid_space = if narrow { 48.0 } else { 80.0 };

                ui.add_space(top_space);
                ui.heading(RichText::new("AI 创作助手").size(title_size));
                ui.add_space(12.0);
                ui.label("文字对话 · 图像生成 · 视频生成");
                ui.add_space(mid_space);

                let button_width = (ui.available_width() - 48.0).clamp(160.0, 280.0);
                let button_size = Vec2::new(button_width, 70.0);
                if ui.add_sized(button_size, egui::Button::new(RichText::new("文字对话").size(20.0))).clicked() {
                    self.enter_mode(ConversationKind::Chat);
                }
                ui.add_space(24.0);
                if ui.add_sized(button_size, egui::Button::new(RichText::new("图像生成").size(20.0))).clicked() {
                    self.enter_mode(ConversationKind::Image);
                }
                ui.add_space(24.0);
                if ui.add_sized(button_size, egui::Button::new(RichText::new("视频生成").size(20.0))).clicked() {
                    self.enter_mode(ConversationKind::Video);
                }
                ui.add_space(48.0);
                if ui.add_sized(Vec2::new(120.0, 40.0), egui::Button::new("退出")).clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
        });
    }

    fn render_mode(&mut self, ctx: &Context, kind: ConversationKind) {
        let screen_width = ctx.screen_rect().width();
        let narrow = screen_width < 600.0;

        TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("← 返回").clicked() {
                    self.screen = Screen::MainMenu;
                    self.selected_id = None;
                    self.error = None;
                }
                if narrow {
                    if ui.button("历史").clicked() {
                        self.narrow_sidebar_open = !self.narrow_sidebar_open;
                    }
                }
                ui.heading(kind.as_str());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("新建对话").clicked() {
                        match kind {
                            ConversationKind::Chat => self.create_conversation(kind, ModelKind::Agnes.as_str()),
                            ConversationKind::Image => self.create_conversation(kind, "agnes-image-2.0-flash"),
                            ConversationKind::Video => self.create_conversation(kind, "agnes-video-v2.0"),
                        }
                    }
                });
            });
            ui.add_space(4.0);
        });

        if !narrow || self.narrow_sidebar_open {
            let sidebar_width = if narrow { screen_width } else { 240.0 };
            SidePanel::left("sidebar")
                .resizable(false)
                .default_width(sidebar_width)
                .max_width(screen_width)
                .show(ctx, |ui| {
                    if narrow {
                        ui.horizontal(|ui| {
                            ui.heading("历史对话");
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button("×").clicked() {
                                    self.narrow_sidebar_open = false;
                                }
                            });
                        });
                    } else {
                        ui.heading("历史对话");
                    }
                    ui.separator();
                    let indices = self.filtered_indices(kind);
                    let items: Vec<_> = indices
                        .iter()
                        .map(|&i| (i, self.conversations[i].title.clone(), self.conversations[i].updated_at))
                        .collect();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.add_space(4.0);
                        for (idx, title, updated) in items {
                            let id = self.conversations[idx].id.clone();
                            let is_selected = self.selected_id.as_deref() == Some(&id);
                            ui.horizontal(|ui| {
                                let available = ui.available_width();
                                let delete_width = 44.0;
                                let spacing = ui.spacing().item_spacing.x;
                                ui.vertical(|ui| {
                                    ui.set_max_width((available - delete_width - spacing).max(60.0));
                                    let response = ui.selectable_label(is_selected, &title);
                                    if response.clicked() {
                                        self.selected_id = Some(id.clone());
                                        if narrow {
                                            self.narrow_sidebar_open = false;
                                        }
                                    }
                                    ui.small(updated.format("%m-%d %H:%M").to_string());
                                });
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.small_button("删除").clicked() {
                                        self.to_delete = Some(id);
                                    }
                                });
                            });
                            ui.add_space(4.0);
                            ui.separator();
                            ui.add_space(4.0);
                        }
                    });
                });
        }

        CentralPanel::default().show(ctx, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                if let Some(error) = &self.error {
                    ui.colored_label(egui::Color32::RED, format!("错误: {}", error));
                    ui.add_space(8.0);
                }

                if let Some(conv) = self.selected_conversation() {
                    let conv_clone = conv.clone();
                    if kind != ConversationKind::Chat {
                        self.render_generation_settings(ui, kind);
                    }
                    self.render_conversation_header(ui, &conv_clone, kind);
                    self.render_messages(ctx, ui, &conv_clone);
                } else {
                    ui.centered_and_justified(|ui| {
                        ui.label("请选择或创建一个对话");
                    });
                }
            });
        });

        let input_margin = if narrow {
            egui::Margin::symmetric(6.0, 6.0)
        } else {
            egui::Margin::symmetric(12.0, 10.0)
        };
        TopBottomPanel::bottom("input_panel")
            .frame(
                egui::Frame::none()
                    .inner_margin(input_margin)
                    .fill(egui::Color32::from_rgb(250, 250, 250)),
            )
            .show(ctx, |ui| {
                if self.selected_conversation().is_some() {
                    self.render_pending_attachments(ctx, ui);

                    let generating = self.selected_generating();
                    let button_text = if generating {
                        "处理中..."
                    } else {
                        match kind {
                            ConversationKind::Chat => "发送",
                            _ => "生成",
                        }
                    };
                    let hint = match kind {
                        ConversationKind::Chat => "输入消息...",
                        ConversationKind::Image => "输入图像生成提示词...",
                        ConversationKind::Video => "输入视频生成提示词...",
                    };

                    if narrow {
                        ui.vertical(|ui| {
                            let text_width = ui.available_width();
                            ui.add(
                                egui::TextEdit::multiline(&mut self.input)
                                    .desired_rows(2)
                                    .desired_width(text_width)
                                    .hint_text(hint),
                            );
                            ui.horizontal(|ui| {
                                if ui.button("上传").clicked() && !generating {
                                    self.upload_file();
                                }
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.button(button_text).clicked() && !generating {
                                        match kind {
                                            ConversationKind::Chat => self.send_chat_message(),
                                            ConversationKind::Image => self.generate_image(),
                                            ConversationKind::Video => self.generate_video(),
                                        }
                                    }
                                });
                            });
                        });
                    } else {
                        ui.horizontal(|ui| {
                            let upload_width = 70.0;
                            let button_width = 80.0;
                            let button_height = 50.0;
                            let spacing = ui.spacing().item_spacing.x;
                            let text_width = (ui.available_width() - upload_width - button_width - spacing * 2.0)
                                .max(100.0);

                            if ui
                                .add_sized(Vec2::new(upload_width, button_height), egui::Button::new("上传"))
                                .clicked()
                                && !generating
                            {
                                self.upload_file();
                            }

                            ui.add_sized(
                                Vec2::new(text_width, button_height),
                                egui::TextEdit::multiline(&mut self.input)
                                    .desired_rows(2)
                                    .hint_text(hint),
                            );

                            if ui
                                .add_sized(Vec2::new(button_width, button_height), egui::Button::new(button_text))
                                .clicked()
                                && !generating
                            {
                                match kind {
                                    ConversationKind::Chat => self.send_chat_message(),
                                    ConversationKind::Image => self.generate_image(),
                                    ConversationKind::Video => self.generate_video(),
                                }
                            }
                        });
                    }
                }
            });

        if let Some(id) = self.to_delete.take() {
            self.delete_conversation(&id);
            let indices = self.filtered_indices(kind);
            if indices.is_empty() {
                match kind {
                    ConversationKind::Chat => self.create_conversation(kind, ModelKind::Agnes.as_str()),
                    ConversationKind::Image => self.create_conversation(kind, "agnes-image-2.0-flash"),
                    ConversationKind::Video => self.create_conversation(kind, "agnes-video-v2.0"),
                }
            } else if self.selected_id.is_none() {
                self.selected_id = Some(self.conversations[indices[0]].id.clone());
            }
        }
    }

    fn render_generation_settings(&mut self, ui: &mut Ui, kind: ConversationKind) {
        let narrow = ui.ctx().screen_rect().width() < 600.0;
        egui::CollapsingHeader::new("生成参数")
            .default_open(false)
            .show(ui, |ui| {
                match kind {
                    ConversationKind::Image => {
                        if narrow {
                            ui.vertical(|ui| {
                                ui.label("模型:");
                                egui::ComboBox::from_id_salt("image_model_setting")
                                    .selected_text(&self.image_model)
                                    .width(ui.available_width().min(180.0))
                                    .show_ui(ui, |ui| {
                                        for text in ["agnes-image-2.0-flash", "agnes-image-2.1-flash"] {
                                            if ui.selectable_label(self.image_model == text, text).clicked() {
                                                self.image_model = text.to_string();
                                                if let Some(idx) = self.selected_index() {
                                                    self.conversations[idx].model = self.image_model.clone();
                                                    let _ = self.storage.save(&self.conversations[idx]);
                                                }
                                            }
                                        }
                                    });
                                ui.label("尺寸:");
                                egui::ComboBox::from_id_salt("image_size_setting")
                                    .selected_text(&self.image_size)
                                    .width(ui.available_width().min(120.0))
                                    .show_ui(ui, |ui| {
                                        for text in ["1024x1024", "512x512", "768x768", "1024x768", "768x1024"] {
                                            if ui.selectable_label(self.image_size == text, text).clicked() {
                                                self.image_size = text.to_string();
                                            }
                                        }
                                    });
                                ui.label("数量:");
                                ui.add(egui::Slider::new(&mut self.image_n, 1..=4));
                            });
                        } else {
                            egui::Grid::new("image_settings")
                                .num_columns(2)
                                .show(ui, |ui| {
                                    ui.label("模型:");
                                    egui::ComboBox::from_id_salt("image_model_setting")
                                        .selected_text(&self.image_model)
                                        .width(180.0)
                                        .show_ui(ui, |ui| {
                                            for text in ["agnes-image-2.0-flash", "agnes-image-2.1-flash"] {
                                                if ui.selectable_label(self.image_model == text, text).clicked() {
                                                    self.image_model = text.to_string();
                                                    if let Some(idx) = self.selected_index() {
                                                        self.conversations[idx].model = self.image_model.clone();
                                                        let _ = self.storage.save(&self.conversations[idx]);
                                                    }
                                                }
                                            }
                                        });
                                    ui.end_row();

                                    ui.label("尺寸:");
                                    egui::ComboBox::from_id_salt("image_size_setting")
                                        .selected_text(&self.image_size)
                                        .width(120.0)
                                        .show_ui(ui, |ui| {
                                            for text in ["1024x1024", "512x512", "768x768", "1024x768", "768x1024"] {
                                                if ui.selectable_label(self.image_size == text, text).clicked() {
                                                    self.image_size = text.to_string();
                                                }
                                            }
                                        });
                                    ui.end_row();

                                    ui.label("数量:");
                                    ui.add(egui::Slider::new(&mut self.image_n, 1..=4));
                                    ui.end_row();
                                });
                        }
                    }
                    ConversationKind::Video => {
                        if narrow {
                            ui.vertical(|ui| {
                                ui.label("宽度:");
                                ui.add(
                                    egui::DragValue::new(&mut self.video_width)
                                        .speed(16)
                                        .range(256..=1920),
                                );
                                ui.label("高度:");
                                ui.add(
                                    egui::DragValue::new(&mut self.video_height)
                                        .speed(16)
                                        .range(256..=1920),
                                );
                                ui.label("帧数:");
                                ui.add(
                                    egui::DragValue::new(&mut self.video_num_frames)
                                        .speed(8)
                                        .range(9..=441),
                                );
                                ui.label("帧率:");
                                ui.add(
                                    egui::DragValue::new(&mut self.video_frame_rate)
                                        .speed(1)
                                        .range(1..=60),
                                );
                                ui.label("模式:");
                                egui::ComboBox::from_id_salt("video_mode_setting")
                                    .selected_text(&self.video_mode)
                                    .width(ui.available_width().min(120.0))
                                    .show_ui(ui, |ui| {
                                        for text in ["ti2vid", "keyframes"] {
                                            if ui.selectable_label(self.video_mode == text, text).clicked() {
                                                self.video_mode = text.to_string();
                                            }
                                        }
                                    });
                            });
                        } else {
                            egui::Grid::new("video_settings")
                                .num_columns(2)
                                .show(ui, |ui| {
                                    ui.label("宽度:");
                                    ui.add(
                                        egui::DragValue::new(&mut self.video_width)
                                            .speed(16)
                                            .range(256..=1920),
                                    );
                                    ui.end_row();
                                    ui.label("高度:");
                                    ui.add(
                                        egui::DragValue::new(&mut self.video_height)
                                            .speed(16)
                                            .range(256..=1920),
                                    );
                                    ui.end_row();
                                    ui.label("帧数:");
                                    ui.add(
                                        egui::DragValue::new(&mut self.video_num_frames)
                                            .speed(8)
                                            .range(9..=441),
                                    );
                                    ui.end_row();
                                    ui.label("帧率:");
                                    ui.add(
                                        egui::DragValue::new(&mut self.video_frame_rate)
                                            .speed(1)
                                            .range(1..=60),
                                    );
                                    ui.end_row();
                                    ui.label("模式:");
                                    egui::ComboBox::from_id_salt("video_mode_setting")
                                        .selected_text(&self.video_mode)
                                        .width(120.0)
                                        .show_ui(ui, |ui| {
                                            for text in ["ti2vid", "keyframes"] {
                                                if ui.selectable_label(self.video_mode == text, text).clicked() {
                                                    self.video_mode = text.to_string();
                                                }
                                            }
                                        });
                                    ui.end_row();
                                });
                        }
                    }
                    _ => {}
                }
            });
        ui.separator();
    }

    fn render_conversation_header(&mut self, ui: &mut Ui, conv: &Conversation, kind: ConversationKind) {
        let narrow = ui.ctx().screen_rect().width() < 600.0;
        if narrow {
            ui.vertical(|ui| {
                ui.label(RichText::new(&conv.title).strong());
                if kind == ConversationKind::Chat {
                    let selected = conv.model.clone();
                    egui::ComboBox::from_id_salt("model_select")
                        .selected_text(&selected)
                        .width(ui.available_width().min(200.0))
                        .show_ui(ui, |ui| {
                            for m in ModelKind::all() {
                                let text = m.as_str();
                                if ui.selectable_label(selected == text, text).clicked() {
                                    if let Some(idx) = self.selected_index() {
                                        self.conversations[idx].model = text.to_string();
                                        let _ = self.storage.save(&self.conversations[idx]);
                                    }
                                }
                            }
                        });
                } else {
                    ui.label(format!("[{}]", conv.model));
                }
            });
        } else {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&conv.title).strong());
                ui.label(format!("[{}]", conv.model));
                if kind == ConversationKind::Chat {
                    let selected = conv.model.clone();
                    egui::ComboBox::from_id_salt("model_select")
                        .selected_text(&selected)
                        .show_ui(ui, |ui| {
                            for m in ModelKind::all() {
                                let text = m.as_str();
                                if ui.selectable_label(selected == text, text).clicked() {
                                    if let Some(idx) = self.selected_index() {
                                        self.conversations[idx].model = text.to_string();
                                        let _ = self.storage.save(&self.conversations[idx]);
                                    }
                                }
                            }
                        });
                }
            });
        }
        ui.separator();
    }

    fn render_pending_attachments(&mut self, ctx: &Context, ui: &mut Ui) {
        if self.pending_attachments.is_empty() {
            return;
        }

        let image_paths: Vec<String> = self
            .pending_attachments
            .iter()
            .filter_map(|att| match att {
                PendingAttachment::Image { path, .. } => Some(path.clone()),
                _ => None,
            })
            .collect();
        for path in image_paths {
            let _ = self.load_texture(ctx, &path);
        }

        let attachments: Vec<(usize, PendingAttachment)> =
            self.pending_attachments.iter().cloned().enumerate().collect();

        ui.horizontal_wrapped(|ui| {
            ui.label("已上传:");
            let mut to_remove = None;
            for (i, att) in attachments {
                match att {
                    PendingAttachment::Text { name, .. } => {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(format!("📄 {}", name));
                                if ui.small_button("×").clicked() {
                                    to_remove = Some(i);
                                }
                            });
                        });
                    }
                    PendingAttachment::Image { name, path } => {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                if let Some(texture) = self.image_textures.get(&path) {
                                    ui.image((texture.id(), Vec2::new(40.0, 40.0)));
                                }
                                ui.label(format!("🖼 {}", name));
                                if ui.small_button("×").clicked() {
                                    to_remove = Some(i);
                                }
                            });
                        });
                    }
                }
            }
            if let Some(i) = to_remove {
                self.pending_attachments.remove(i);
            }
        });
        ui.add_space(8.0);
    }

    fn render_messages(&mut self, ctx: &Context, ui: &mut Ui, conv: &Conversation) {
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                ui.add_space(8.0);
                for (msg_idx, msg) in conv.messages.iter().enumerate() {
                    if msg.role == "system" {
                        continue;
                    }
                    let is_user = msg.role == "user";
                    let bg = if is_user {
                        egui::Color32::from_rgb(200, 230, 255)
                    } else {
                        egui::Color32::from_rgb(240, 240, 240)
                    };

                    let align = if is_user {
                        egui::Layout::right_to_left(egui::Align::TOP)
                    } else {
                        egui::Layout::left_to_right(egui::Align::TOP)
                    };

                    ui.with_layout(align, |ui| {
                        let narrow = ctx.screen_rect().width() < 600.0;
                        let inner_margin = if narrow {
                            egui::Margin::symmetric(8.0, 8.0)
                        } else {
                            egui::Margin::same(10.0)
                        };
                        egui::Frame::group(ui.style())
                            .fill(bg)
                            .rounding(8.0)
                            .stroke(egui::Stroke::NONE)
                            .inner_margin(inner_margin)
                            .show(ui, |ui| {
                                let max_width = ui.available_width() * if narrow { 0.95 } else { 0.75 };
                                ui.set_max_width(max_width);

                                let (think, main_content) = Self::split_think_content(&msg.content);
                                if let Some(think) = think {
                                    egui::CollapsingHeader::new("💭 思考过程")
                                        .default_open(false)
                                        .show(ui, |ui| {
                                            egui::Frame::group(ui.style())
                                                .fill(egui::Color32::from_rgb(245, 245, 245))
                                                .show(ui, |ui| {
                                                    egui::Label::new(
                                                        RichText::new(think)
                                                            .color(egui::Color32::DARK_GRAY)
                                                            .monospace(),
                                                    )
                                                    .wrap()
                                                    .ui(ui);
                                                });
                                        });
                                    ui.add_space(6.0);
                                }
                                markdown::render_markdown(ui, &main_content);
                                for att in &msg.attachments {
                                    match att.kind {
                                        AttachmentKind::Image => {
                                            if let Some(texture) = self.load_texture(ctx, &att.local_path) {
                                                let size = texture.size_vec2();
                                                let max_img_width = max_width.min(400.0);
                                                let scale = (max_img_width / size.x).min(1.0);
                                                let display_size = size * scale;
                                                let response =
                                                    ui.add(egui::ImageButton::new((texture.id(), display_size)));
                                                if response.clicked() {
                                                    self.preview_attachment = Some(att.clone());
                                                }
                                                ui.horizontal(|ui| {
                                                    if ui.small_button("预览").clicked() {
                                                        self.preview_attachment = Some(att.clone());
                                                    }
                                                    if ui.small_button("下载").clicked() {
                                                        self.download_attachment(att);
                                                    }
                                                });
                                            } else {
                                                ui.label(format!("图片加载失败: {}", att.local_path));
                                            }
                                        }
                                        AttachmentKind::Video => {
                                            ui.horizontal(|ui| {
                                                ui.label(format!("视频: {}", att.file_name));
                                                if ui.small_button("播放").clicked() {
                                                    self.open_with_system(&att.local_path);
                                                }
                                                if ui.small_button("下载").clicked() {
                                                    self.download_attachment(att);
                                                }
                                            });
                                        }
                                    }
                                }

                                ui.add_space(6.0);
                                let conv_id = conv.id.clone();
                                ui.horizontal(|ui| {
                                    if ui.small_button("📋").clicked() {
                                        self.pending_copy = Some(msg.content.clone());
                                    }
                                    if msg.role == "assistant" && conv.kind == ConversationKind::Chat {
                                        if ui.small_button("🔄").clicked() {
                                            self.pending_message_action = Some(MessageAction::Regenerate {
                                                conv_id: conv_id.clone(),
                                                idx: msg_idx,
                                            });
                                        }
                                    }
                                    if ui.small_button("🗑").clicked() {
                                        self.pending_message_action = Some(MessageAction::Delete {
                                            conv_id: conv_id.clone(),
                                            idx: msg_idx,
                                        });
                                    }
                                });
                            });
                    });
                    ui.add_space(12.0);
                }
                if self.is_generating(&conv.id) {
                    ui.label("处理中...");
                    ui.add_space(8.0);
                }
            });
    }
}

pub fn run_app() -> eframe::Result {
    let options = NativeOptions {
        viewport: ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "AI 创作助手",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: android_activity::AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Debug),
    );

    android_file_picker::set_vm(app.vm_as_ptr() as *mut _);

    let mut options = NativeOptions {
        viewport: ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0]),
        ..Default::default()
    };

    options.event_loop_builder = Some(Box::new(move |builder| {
        use winit::platform::android::EventLoopBuilderExtAndroid;
        builder.with_android_app(app);
    }));

    eframe::run_native(
        "AI 创作助手",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
    .unwrap();
}
