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
use crate::types::{Attachment, AttachmentKind, Conversation, ConversationKind, ModelKind};
use chrono::Local;
use iced::border::Radius;
use iced::font;
use iced::widget::{
    button, center, column, container, image, pick_list, row, scrollable, text, text_input, Column,
    Space,
};
use iced::window;
use iced::{Alignment, Element, Length, Padding, Size, Subscription, Task, Theme};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq)]
enum Screen {
    MainMenu,
    Mode(ConversationKind),
}

#[derive(Debug, Clone)]
pub enum Message {
    Ignore,
    FontLoaded(Result<(), String>),
    BackToMenu,
    SelectMode(ConversationKind),
    CreateConversation(ConversationKind, String),
    SelectConversation(String),
    ToggleSidebar,
    DeleteConversation(String),
    DeleteMessage(String, usize),
    RegenerateMessage(String, usize),
    InputChanged(String),
    SendMessage,
    GenerateImage,
    UploadFile,
    StreamChunk(String, String),
    StreamDone(String, Result<(), String>),
    FormulaReady(String, Result<markdown::FormulaTexture, String>),
    ModelChanged(String),
    WindowResized(Size),
    CopyMessage(String),
    CloseError,
    AttachmentRemove(usize),
    FileUploaded(PendingAttachment),
    FileDownloaded(String, Result<(), String>),
    DownloadAttachment(String),
    ImageGenerated(String, Result<Vec<Attachment>, String>),
}

#[derive(Debug, Clone)]
pub enum PendingAttachment {
    Text { name: String, content: String },
    Image { name: String, path: String },
}

pub struct App {
    storage: Storage,
    screen: Screen,
    conversations: Vec<Conversation>,
    selected_id: Option<String>,
    input: String,
    generating_ids: HashSet<String>,
    error: Option<String>,
    formula_textures: HashMap<String, markdown::FormulaTexture>,
    pending_formulas: HashSet<String>,
    pending_attachments: Vec<PendingAttachment>,
    narrow_sidebar_open: bool,
    window_size: Size,
    narrow: bool,
    image_size: String,
    image_n: u32,
}

fn load_icon() -> Option<window::Icon> {
    let embedded = include_bytes!("../favicon.ico").as_slice();
    if let Ok(icon) = window::icon::from_file_data(embedded, None) {
        return Some(icon);
    }

    for path in [
        std::path::PathBuf::from("favicon.ico"),
        std::path::PathBuf::from("icon.png"),
    ] {
        if let Ok(bytes) = std::fs::read(&path) {
            if let Ok(icon) = window::icon::from_file_data(&bytes, None) {
                return Some(icon);
            }
        }
    }
    None
}

fn app_theme(_: &App) -> Theme {
    Theme::Light
}

fn app_subscription(_: &App) -> Subscription<Message> {
    iced::window::resize_events().map(|(_id, size)| Message::WindowResized(size))
}

pub fn run_app() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("Star AI Chat")
        .theme(app_theme)
        .default_font(markdown::FONT_BODY)
        .antialiasing(true)
        .subscription(app_subscription)
        .window(window::Settings {
            icon: load_icon(),
            ..window::Settings::default()
        })
        .run()
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let (storage_dir, media_dir) = Storage::default_dirs().expect("获取存储目录失败");
        let storage = Storage::new(&storage_dir, &media_dir).expect("初始化存储失败");
        let conversations = storage.list().unwrap_or_default();

        let load_fonts = Task::batch([
            font::load(include_bytes!("../fonts/a.otf").as_slice())
                .map(|r| Message::FontLoaded(r.map_err(|e| format!("{:?}", e)))),
            font::load(include_bytes!("../fonts/b.otf").as_slice())
                .map(|r| Message::FontLoaded(r.map_err(|e| format!("{:?}", e)))),
            font::load(include_bytes!("../fonts/dk.ttf").as_slice())
                .map(|r| Message::FontLoaded(r.map_err(|e| format!("{:?}", e)))),
        ]);

        (
            Self {
                storage,
                screen: Screen::MainMenu,
                conversations,
                selected_id: None,
                input: String::new(),
                generating_ids: HashSet::new(),
                error: None,
                formula_textures: HashMap::new(),
                pending_formulas: HashSet::new(),
                pending_attachments: Vec::new(),
                narrow_sidebar_open: false,
                window_size: Size::new(0.0, 0.0),
                narrow: cfg!(target_os = "android"),
                image_size: "1024x1024".to_string(),
                image_n: 1,
            },
            load_fonts,
        )
    }

    fn selected_index(&self) -> Option<usize> {
        self.selected_id
            .as_ref()
            .and_then(|id| self.conversations.iter().position(|c| &c.id == id))
    }

    fn selected_conversation(&self) -> Option<&Conversation> {
        self.selected_index().map(|idx| &self.conversations[idx])
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

    fn prefetch_formulas(&mut self) -> Task<Message> {
        let mut work = Vec::new();
        if let Some(idx) = self.selected_index() {
            let conv = &self.conversations[idx];
            for msg in &conv.messages {
                for (latex, display) in markdown::extract_formulas(&msg.content) {
                    let key = markdown::formula_key(&latex, display);
                    if !self.formula_textures.contains_key(&key)
                        && self.pending_formulas.insert(key.clone())
                    {
                        work.push((key, latex, display));
                    }
                }
            }
        }

        Task::batch(work.into_iter().map(|(key, latex, display)| {
            Task::perform(
                async move {
                    let texture = tokio::task::spawn_blocking(move || {
                        markdown::render_formula_texture(&latex, display)
                    })
                    .await
                    .ok()
                    .flatten();
                    (key, texture)
                },
                |(key, texture)| {
                    Message::FormulaReady(
                        key,
                        texture.ok_or_else(|| "公式渲染失败".to_string()),
                    )
                },
            )
        }))
    }

    fn filtered_indices(&self, kind: ConversationKind) -> Vec<usize> {
        self.conversations
            .iter()
            .enumerate()
            .filter(|(_, c)| c.kind == kind)
            .map(|(i, _)| i)
            .collect()
    }

    fn create_conversation(&mut self, kind: ConversationKind, model: &str) {
        let conv = Conversation::new(kind, model);
        let id = conv.id.clone();
        let _ = self.storage.save(&conv);
        self.conversations.push(conv);
        self.selected_id = Some(id);
    }

    fn delete_conversation(&mut self, id: &str) {
        if let Some(idx) = self.conversations.iter().position(|c| c.id == id) {
            let _ = self.storage.delete(id);
            self.conversations.remove(idx);
            if self.selected_id.as_deref() == Some(id) {
                self.selected_id = None;
            }
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Ignore => Task::none(),
            Message::FontLoaded(result) => {
                if let Err(e) = result {
                    eprintln!("字体加载失败: {}", e);
                }
                Task::none()
            }
            Message::BackToMenu => {
                self.screen = Screen::MainMenu;
                self.selected_id = None;
                self.error = None;
                Task::none()
            }
            Message::SelectMode(kind) => {
                self.screen = Screen::Mode(kind);
                self.error = None;
                let indices = self.filtered_indices(kind);
                if indices.is_empty() {
                    let model = match kind {
                        ConversationKind::Chat => ModelKind::Agnes.as_str().to_string(),
                        ConversationKind::Image => "agnes-image-2.0-flash".to_string(),
                        ConversationKind::Video => return Task::none(),
                    };
                    self.create_conversation(kind, &model);
                } else {
                    self.selected_id = Some(self.conversations[indices[0]].id.clone());
                }
                self.prefetch_formulas()
            }
            Message::CreateConversation(kind, model) => {
                self.create_conversation(kind, &model);
                Task::none()
            }
            Message::SelectConversation(id) => {
                self.selected_id = Some(id);
                self.narrow_sidebar_open = false;
                self.prefetch_formulas()
            }
            Message::ToggleSidebar => {
                self.narrow_sidebar_open = !self.narrow_sidebar_open;
                Task::none()
            }
            Message::DeleteConversation(id) => {
                self.delete_conversation(&id);
                if let Screen::Mode(kind) = self.screen {
                    let indices = self.filtered_indices(kind);
                    if indices.is_empty() {
                        let model = match kind {
                            ConversationKind::Chat => ModelKind::Agnes.as_str().to_string(),
                            ConversationKind::Image => "agnes-image-2.0-flash".to_string(),
                            ConversationKind::Video => return Task::none(),
                        };
                        self.create_conversation(kind, &model);
                    } else if self.selected_id.is_none() {
                        self.selected_id = Some(self.conversations[indices[0]].id.clone());
                    }
                }
                Task::none()
            }
            Message::DeleteMessage(conv_id, idx) => {
                if let Some(i) = self.conversations.iter().position(|c| c.id == conv_id) {
                    if idx < self.conversations[i].messages.len() {
                        self.conversations[i].messages.remove(idx);
                        self.conversations[i].updated_at = Local::now();
                        let _ = self.storage.save(&self.conversations[i]);
                    }
                }
                Task::none()
            }
            Message::RegenerateMessage(conv_id, _idx) => {
                let task = self.regenerate_message(conv_id);
                Task::batch([task, self.prefetch_formulas()])
            }
            Message::InputChanged(value) => {
                self.input = value;
                Task::none()
            }
            Message::SendMessage => {
                let task = if let Screen::Mode(kind) = self.screen {
                    match kind {
                        ConversationKind::Chat => self.send_chat_message(),
                        ConversationKind::Image => self.generate_image(),
                        ConversationKind::Video => Task::none(),
                    }
                } else {
                    Task::none()
                };
                Task::batch([task, self.prefetch_formulas()])
            }
            Message::GenerateImage => self.generate_image(),
            Message::UploadFile => self.upload_file(),
            Message::StreamChunk(conv_id, content) => {
                if let Some(i) = self.conversations.iter().position(|c| c.id == conv_id) {
                    let msgs = &mut self.conversations[i].messages;
                    if let Some(last) = msgs.last_mut() {
                        if last.role == "assistant" {
                            last.content.push_str(&content);
                        }
                    }
                }
                Task::none()
            }
            Message::StreamDone(conv_id, result) => {
                self.generating_ids.remove(&conv_id);
                if let Err(e) = result {
                    self.error = Some(e);
                }
                if let Some(i) = self.conversations.iter().position(|c| c.id == conv_id) {
                    self.conversations[i].updated_at = Local::now();
                    let _ = self.storage.save(&self.conversations[i]);
                }
                self.prefetch_formulas()
            }
            Message::FormulaReady(key, result) => {
                self.pending_formulas.remove(&key);
                match result {
                    Ok(texture) => {
                        self.formula_textures.insert(key, texture);
                    }
                    Err(e) => {
                        eprintln!("公式加载失败: {}", e);
                    }
                }
                Task::none()
            }
            Message::ModelChanged(model) => {
                if let Some(idx) = self.selected_index() {
                    self.conversations[idx].model = model;
                    let _ = self.storage.save(&self.conversations[idx]);
                }
                Task::none()
            }
            Message::WindowResized(size) => {
                self.window_size = size;
                self.narrow = size.width < 600.0;
                Task::none()
            }
            Message::CopyMessage(content) => {
                let _ = cli_clipboard::set_contents(content);
                Task::none()
            }
            Message::CloseError => {
                self.error = None;
                Task::none()
            }
            Message::AttachmentRemove(i) => {
                if i < self.pending_attachments.len() {
                    self.pending_attachments.remove(i);
                }
                Task::none()
            }
            Message::FileUploaded(att) => {
                self.pending_attachments.push(att);
                Task::none()
            }
            Message::FileDownloaded(name, result) => {
                if let Err(e) = result {
                    self.error = Some(format!("下载 {} 失败: {}", name, e));
                }
                Task::none()
            }
            Message::DownloadAttachment(path) => self.download_attachment(path),
            Message::ImageGenerated(conv_id, result) => {
                self.generating_ids.remove(&conv_id);
                if let Some(i) = self.conversations.iter().position(|c| c.id == conv_id) {
                    match result {
                        Ok(attachments) => {
                            let summary = format!("生成 {} 张图片", attachments.len());
                            self.conversations[i].add_message_with_attachments(
                                "assistant",
                                summary,
                                attachments,
                            );
                            self.conversations[i].updated_at = Local::now();
                            let _ = self.storage.save(&self.conversations[i]);
                        }
                        Err(e) => {
                            self.error = Some(e);
                        }
                    }
                }
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        match &self.screen {
            Screen::MainMenu => self.view_main_menu(),
            Screen::Mode(kind) => self.view_mode(*kind),
        }
    }
}

// Backend helpers
impl App {
    fn send_chat_message(&mut self) -> Task<Message> {
        let input = self.input.trim();
        if input.is_empty() && self.pending_attachments.is_empty() {
            return Task::none();
        }
        if self.selected_generating() {
            return Task::none();
        }

        let idx = match self.selected_index() {
            Some(i) => i,
            None => return Task::none(),
        };

        let mut attachments = Vec::new();
        std::mem::swap(&mut attachments, &mut self.pending_attachments);

        let mut content = input.to_string();
        let mut real_attachments: Vec<Attachment> = Vec::new();
        for att in attachments {
            match att {
                PendingAttachment::Text { name, content: txt } => {
                    content.push_str(&format!("\n\n[附件: {}]\n{}", name, txt));
                }
                PendingAttachment::Image { name, path } => {
                    real_attachments.push(Attachment {
                        file_name: name,
                        local_path: path,
                        kind: AttachmentKind::Image,
                        source_url: String::new(),
                    });
                }
            }
        }

        self.conversations[idx].add_message_with_attachments("user", content, real_attachments);
        self.conversations[idx].updated_at = Local::now();
        self.input.clear();

        let model = ModelKind::from_str(&self.conversations[idx].model).unwrap_or(ModelKind::Agnes);
        let messages = self.conversations[idx].messages.clone();
        let conv_id = self.conversations[idx].id.clone();
        self.conversations[idx].add_assistant_placeholder();
        self.generating_ids.insert(conv_id.clone());

        if let Err(e) = self.storage.save(&self.conversations[idx]) {
            self.error = Some(format!("保存失败: {}", e));
        }

        let stream = async_stream::stream! {
            let (chunk_tx, mut chunk_rx) =
                tokio::sync::mpsc::channel::<std::result::Result<String, anyhow::Error>>(128);
            let stream_task = tokio::spawn(async move {
                models::request_stream(model, &messages, chunk_tx).await
            });

            while let Some(result) = chunk_rx.recv().await {
                match result {
                    Ok(content) => yield Message::StreamChunk(conv_id.clone(), content),
                    Err(e) => {
                        yield Message::StreamDone(conv_id.clone(), Err(e.to_string()));
                        return;
                    }
                }
            }

            let result = stream_task
                .await
                .unwrap_or_else(|e| Err(anyhow::anyhow!("流式任务异常: {}", e)))
                .map_err(|e| e.to_string());
            yield Message::StreamDone(conv_id, result.map(|_| ()));
        };

        Task::run(stream, |msg| msg)
    }

    fn regenerate_message(&mut self, conv_id: String) -> Task<Message> {
        if self.is_generating(&conv_id) {
            return Task::none();
        }
        let idx = match self.conversations.iter().position(|c| c.id == conv_id) {
            Some(i) => i,
            None => return Task::none(),
        };

        self.conversations[idx].add_message("user", "重新生成");
        self.conversations[idx].updated_at = Local::now();

        let model = ModelKind::from_str(&self.conversations[idx].model).unwrap_or(ModelKind::Agnes);
        let messages = self.conversations[idx].messages.clone();

        self.conversations[idx].add_assistant_placeholder();
        self.generating_ids.insert(conv_id.clone());

        if let Err(e) = self.storage.save(&self.conversations[idx]) {
            self.error = Some(format!("保存失败: {}", e));
        }

        let stream = async_stream::stream! {
            let (chunk_tx, mut chunk_rx) =
                tokio::sync::mpsc::channel::<std::result::Result<String, anyhow::Error>>(128);
            let stream_task = tokio::spawn(async move {
                models::request_stream(model, &messages, chunk_tx).await
            });

            while let Some(result) = chunk_rx.recv().await {
                match result {
                    Ok(content) => yield Message::StreamChunk(conv_id.clone(), content),
                    Err(e) => {
                        yield Message::StreamDone(conv_id.clone(), Err(e.to_string()));
                        return;
                    }
                }
            }

            let result = stream_task
                .await
                .unwrap_or_else(|e| Err(anyhow::anyhow!("流式任务异常: {}", e)))
                .map_err(|e| e.to_string());
            yield Message::StreamDone(conv_id, result.map(|_| ()));
        };

        Task::run(stream, |msg| msg)
    }

    fn generate_image(&mut self) -> Task<Message> {
        let input = self.input.trim();
        if input.is_empty() {
            return Task::none();
        }
        if self.selected_generating() {
            return Task::none();
        }
        let idx = match self.selected_index() {
            Some(i) => i,
            None => return Task::none(),
        };

        let conv_id = self.conversations[idx].id.clone();
        let model = self.conversations[idx].model.clone();
        let prompt = input.to_string();
        let size = self.image_size.clone();
        let n = self.image_n;
        let media_dir = self.storage.conversation_media_dir(&conv_id);

        self.conversations[idx].add_message("user", prompt.clone());
        self.conversations[idx].updated_at = Local::now();
        self.input.clear();
        self.generating_ids.insert(conv_id.clone());
        if let Err(e) = self.storage.save(&self.conversations[idx]) {
            self.error = Some(format!("保存失败: {}", e));
        }

        Task::perform(
            async move {
                match models::image_generation::request(&model, &prompt, &size, n, None).await {
                    Ok(urls) => {
                        let mut attachments = Vec::new();
                        for (i, url) in urls.iter().enumerate() {
                            let file_name = format!("image_{}", i + 1);
                            match media::download_file(url, &media_dir, &file_name).await {
                                Ok(path) => {
                                    attachments.push(Attachment {
                                        kind: AttachmentKind::Image,
                                        file_name: format!("{}.png", file_name),
                                        local_path: path.to_string_lossy().to_string(),
                                        source_url: url.clone(),
                                    });
                                }
                                Err(e) => {
                                    return (conv_id, Err(format!("下载图片失败: {}", e)));
                                }
                            }
                        }
                        (conv_id, Ok(attachments))
                    }
                    Err(e) => (conv_id, Err(format!("图像生成失败: {}", e))),
                }
            },
            |(conv_id, result)| Message::ImageGenerated(conv_id, result),
        )
    }

    #[cfg(not(target_os = "android"))]
    fn download_attachment(&self, path: String) -> Task<Message> {
        let path_for_msg = path.clone();
        Task::perform(
            async move {
                let result = tokio::task::spawn_blocking(move || {
                    let src = std::path::Path::new(&path);
                    let name = src
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "download".to_string());
                    if let Some(dest) = rfd::FileDialog::new().set_file_name(&name).save_file() {
                        match std::fs::copy(src, &dest) {
                            Ok(_) => Ok(()),
                            Err(e) => Err(format!("复制文件失败: {}", e)),
                        }
                    } else {
                        Ok(())
                    }
                })
                .await;
                match result {
                    Ok(r) => r,
                    Err(e) => Err(format!("下载任务异常: {}", e)),
                }
            },
            move |result| Message::FileDownloaded(path_for_msg.clone(), result),
        )
    }

    #[cfg(target_os = "android")]
    fn download_attachment(&self, path: String) -> Task<Message> {
        Task::perform(
            async move {
                // Android 上暂不支持选择保存位置，直接返回路径
                Ok(())
            },
            move |_| Message::FileDownloaded(path.clone(), Ok(())),
        )
    }

    #[cfg(not(target_os = "android"))]
    fn upload_file(&mut self) -> Task<Message> {
        Task::perform(
            async {
                // 在 blocking 线程中运行 rfd
                tokio::task::spawn_blocking(|| {
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        let path_str = path.to_string_lossy().to_string();
                        let name = path
                            .file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| "file".to_string());
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            Some(PendingAttachment::Text { name, content })
                        } else if ::image::open(&path).is_ok() {
                            Some(PendingAttachment::Image { name, path: path_str })
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .await
                .ok()
                .flatten()
            },
            |att| {
                if let Some(att) = att {
                    Message::FileUploaded(att)
                } else {
                    Message::CloseError
                }
            },
        )
    }

    #[cfg(target_os = "android")]
    fn upload_file(&mut self) -> Task<Message> {
        Task::none()
    }
}

// Views
impl App {
    fn view_main_menu(&self) -> Element<'_, Message> {
        let content = column![
            Space::new().height(Length::Fill),
            text("Star AI Chat")
                .size(32)
                .color(iced::Color::from_rgb(0.118, 0.259, 0.686)),
            text("文字对话 · 图像生成")
                .size(14)
                .color(iced::Color::from_rgb(0.392, 0.455, 0.518)),
            Space::new().height(24),
            button(text("文字对话").size(16).center())
                .on_press(Message::SelectMode(ConversationKind::Chat))
                .padding(Padding::from([10, 20]))
                .width(200),
            Space::new().height(12),
            button(text("图像生成").size(16).center())
                .on_press(Message::SelectMode(ConversationKind::Image))
                .padding(Padding::from([10, 20]))
                .width(200),
            Space::new().height(Length::Fill),
        ]
        .align_x(Alignment::Center)
        .spacing(8);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(|_| container::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgb(
                    0.961, 0.969, 0.980,
                ))),
                ..container::Style::default()
            })
            .into()
    }

    fn view_mode(&self, kind: ConversationKind) -> Element<'_, Message> {
        let top = self.view_top_bar(kind);
        let sidebar_visible = !self.narrow || self.narrow_sidebar_open;
        let narrow_open = self.narrow && sidebar_visible;

        let sidebar_width = if sidebar_visible {
            if self.narrow {
                Length::Fill
            } else {
                Length::Fixed(260.0)
            }
        } else {
            Length::Fixed(0.0)
        };
        let main_width = if narrow_open {
            Length::Fixed(0.0)
        } else {
            Length::Fill
        };

        let sidebar: Element<'_, Message> = if sidebar_visible {
            container(self.view_sidebar(kind))
                .width(sidebar_width)
                .height(Length::Fill)
                .into()
        } else {
            Space::new().width(0).height(Length::Fill).into()
        };
        let main: Element<'_, Message> = if narrow_open {
            Space::new().width(0).height(Length::Fill).into()
        } else {
            container(self.view_main(kind))
                .width(main_width)
                .height(Length::Fill)
                .into()
        };

        let body = row![sidebar, main];

        column![top, body.height(Length::Fill)]
            .height(Length::Fill)
            .into()
    }

    fn view_top_bar(&self, kind: ConversationKind) -> Element<'_, Message> {
        let title = text(kind.as_str()).size(if self.narrow { 16 } else { 18 });
        let back_label = if self.narrow { "<" } else { "← 返回" };
        let back = button(text(back_label).size(14))
            .on_press(Message::BackToMenu)
            .style(button::secondary);
        let new_label = if self.narrow { "+" } else { "新建对话" };
        let new_conv = button(text(new_label).size(14))
            .on_press(Message::CreateConversation(
                kind,
                match kind {
                    ConversationKind::Chat => ModelKind::Agnes.as_str().to_string(),
                    ConversationKind::Image => "agnes-image-2.0-flash".to_string(),
                    ConversationKind::Video => "agnes-video-v2.0".to_string(),
                },
            ))
            .style(button::primary);

        let mut top = row![back].spacing(8).align_y(Alignment::Center);
        if self.narrow {
            top = top.push(
                button(text("历史").size(14))
                    .on_press(Message::ToggleSidebar)
                    .style(button::secondary),
            );
        }
        top = top.push(title).push(Space::new().width(Length::Fill)).push(new_conv);

        top.padding(if self.narrow { 8 } else { 12 }).into()
    }

    fn view_sidebar(&self, kind: ConversationKind) -> Element<'_, Message> {
        let header = row![
            text("历史对话").size(18),
            Space::new().width(Length::Fill),
            button(text("×").size(16))
                .on_press(Message::ToggleSidebar)
                .style(button::secondary),
        ]
        .padding(12);

        let items: Vec<Element<Message>> = self
            .filtered_indices(kind)
            .iter()
            .map(|&idx| {
                let conv = &self.conversations[idx];
                let id = conv.id.clone();
                let is_selected = self.selected_id.as_deref() == Some(&id);

                let title = button(text(&conv.title).size(14))
                    .on_press(Message::SelectConversation(id.clone()))
                    .style(if is_selected {
                        button::primary
                    } else {
                        button::secondary
                    });

                let del = button(text("×").size(14).color(iced::Color::from_rgb(0.937, 0.267, 0.267)))
                    .on_press(Message::DeleteConversation(id))
                    .style(button::text);

                row![title.width(Length::Fill), del]
                    .spacing(4)
                    .padding(8)
                    .into()
            })
            .collect();

        let list = Column::with_children(items).spacing(4).padding(8);

        container(scrollable(column![header, list]))
            .width(if self.narrow { Length::Fill } else { Length::Fixed(260.0) })
            .height(Length::Fill)
            .style(|_| container::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgb(
                    0.973, 0.976, 0.980,
                ))),
                ..container::Style::default()
            })
            .into()
    }

    fn view_main(&self, kind: ConversationKind) -> Element<'_, Message> {
        if let Some(conv) = self.selected_conversation() {
            let header = self.view_conversation_header(conv, kind);
            let messages = container(self.view_messages(conv))
                .height(Length::Fill)
                .width(Length::Fill);
            let input = self.view_input(kind);

            column![header, messages, input]
                .padding(12)
                .spacing(8)
                .height(Length::Fill)
                .into()
        } else {
            center(text("请选择或创建一个对话").size(16).color(iced::Color::from_rgb(
                0.580, 0.639, 0.718,
            )))
            .into()
        }
    }

    fn view_conversation_header(
        &self,
        conv: &Conversation,
        kind: ConversationKind,
    ) -> Element<'static, Message> {
        let title = text(conv.title.clone()).size(16);
        let model_text = format!("[{}]", conv.model);
        let model = text(model_text)
            .size(13)
            .color(iced::Color::from_rgb(0.392, 0.455, 0.518));

        if kind != ConversationKind::Chat {
            return container(row![title, Space::new().width(8), model])
                .padding(if self.narrow { 8 } else { 12 })
                .style(|_| container::Style {
                    background: Some(iced::Background::Color(iced::Color::WHITE)),
                    border: iced::Border {
                        radius: Radius::from(10.0),
                        width: 1.0,
                        color: iced::Color::from_rgb(0.902, 0.922, 0.941),
                    },
                    ..container::Style::default()
                })
                .into();
        }

        let models: Vec<String> = ModelKind::all().iter().map(|m| m.as_str().to_string()).collect();
        let selector = pick_list(models, Some(conv.model.clone()), Message::ModelChanged)
            .width(Length::Shrink)
            .placeholder("选择模型");

        let content: Element<'static, Message> = if self.narrow {
            column![
                row![title, Space::new().width(8), model],
                row![selector.width(Length::Fill)].spacing(4),
            ]
            .spacing(4)
            .into()
        } else {
            row![title, Space::new().width(8), model, Space::new().width(Length::Fill), selector].into()
        };

        container(content)
            .padding(if self.narrow { 8 } else { 12 })
            .style(|_| container::Style {
                background: Some(iced::Background::Color(iced::Color::WHITE)),
                border: iced::Border {
                    radius: Radius::from(10.0),
                    width: 1.0,
                    color: iced::Color::from_rgb(0.902, 0.922, 0.941),
                },
                ..container::Style::default()
            })
            .into()
    }

    fn view_messages<'a>(&'a self, conv: &'a Conversation) -> Element<'a, Message> {
        let mut msgs = Column::new().spacing(12).padding(8);

        for (idx, msg) in conv.messages.iter().enumerate() {
            if msg.role == "system" {
                continue;
            }
            let is_user = msg.role == "user";
            let content = msg.content.clone();
            let conv_id = conv.id.clone();

            let rendered = markdown::render_message(&msg.content, &self.formula_textures, self.narrow);
            let body: Element<'a, Message> = if is_user {
                container(rendered)
                    .padding(10)
                    .style(move |_| container::Style {
                        background: Some(iced::Background::Color(iced::Color::from_rgb(
                            0.878, 0.949, 0.996,
                        ))),
                        border: iced::Border {
                            radius: Radius::from(12.0),
                            width: 0.0,
                            color: iced::Color::TRANSPARENT,
                        },
                        ..container::Style::default()
                    })
                    .into()
            } else {
                container(rendered)
                    .padding(8)
                    .width(Length::Fill)
                    .into()
            };

            let row_content: Element<'a, Message> = if is_user {
                row![Space::new().width(Length::Fill), body].into()
            } else {
                body
            };

            let mut actions: iced::widget::Row<'_, Message> = row![];
            actions = actions.push(
                button(text("复制").size(11))
                    .on_press(Message::CopyMessage(content.clone()))
                    .style(button::text),
            );
            if !is_user && conv.kind == ConversationKind::Chat {
                actions = actions.push(
                    button(text("重新生成").size(11))
                        .on_press(Message::RegenerateMessage(conv_id.clone(), idx))
                        .style(button::text),
                );
            }
            actions = actions.push(
                button(text("删除").size(11))
                    .on_press(Message::DeleteMessage(conv_id.clone(), idx))
                    .style(button::text),
            );

            let aligned_actions: Element<'a, Message> = if is_user {
                row![Space::new().width(Length::Fill), actions].into()
            } else {
                row![actions].into()
            };

            let mut msg_column = column![row_content, aligned_actions].spacing(4);

            // 图片附件预览与下载
            if !msg.attachments.is_empty() {
                let mut att_col = Column::new().spacing(8);
                for att in &msg.attachments {
                    let att_elem = self.view_attachment(att);
                    att_col = att_col.push(att_elem);
                }
                msg_column = msg_column.push(att_col);
            }

            msgs = msgs.push(msg_column.align_x(if is_user {
                Alignment::End
            } else {
                Alignment::Start
            }));
        }

        if self.is_generating(&conv.id) {
            msgs = msgs.push(text("处理中...").size(14).color(iced::Color::from_rgb(
                0.392, 0.455, 0.518,
            )));
        }

        scrollable(msgs).into()
    }

    fn view_attachment(&self, att: &Attachment) -> Element<'_, Message> {
        let path = att.local_path.clone();
        let download = button(text("下载").size(11))
            .on_press(Message::DownloadAttachment(path.clone()))
            .style(button::text);

        if att.kind == AttachmentKind::Image && std::path::Path::new(&path).exists() {
            let preview = image(image::Handle::from_path(&path))
                .width(Length::Fill)
                .height(Length::Shrink);
            column![preview, row![download].spacing(4)]
                .spacing(4)
                .into()
        } else {
            container(row![text(att.file_name.clone()).size(13), download].spacing(8))
                .padding(8)
                .style(|_| container::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgb(0.96, 0.96, 0.96))),
                    border: iced::Border {
                        radius: Radius::from(6.0),
                        width: 1.0,
                        color: iced::Color::from_rgb(0.90, 0.90, 0.90),
                    },
                    ..container::Style::default()
                })
                .into()
        }
    }

    fn view_input(&self, kind: ConversationKind) -> Element<'_, Message> {
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

        let input = text_input(hint, &self.input)
            .on_input(Message::InputChanged)
            .on_submit(Message::SendMessage)
            .padding(12)
            .width(Length::Fill);

        let send = button(text(button_text).size(14).center())
            .on_press_maybe(if generating { None } else { Some(Message::SendMessage) })
            .padding(Padding::from([10, 16]))
            .style(button::primary);

        let upload = button(text("上传").size(14))
            .on_press_maybe(if generating { None } else { Some(Message::UploadFile) })
            .padding(Padding::from([10, 12]))
            .style(button::secondary);

        row![upload, input, send]
            .spacing(8)
            .align_y(Alignment::Center)
            .padding(12)
            .into()
    }
}
