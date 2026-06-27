use crate::types::{Attachment, AttachmentKind, Conversation};
use anyhow::{Context, Result};
use pulldown_cmark::{CowStr, Event, Options, Parser};
use std::sync::mpsc::{channel, Receiver};
use wry::http::Request;
use wry::{WebView, WebViewBuilder};

const SHELL_HTML: &str = include_str!("../webview_shell.html");

pub enum WebViewAction {
    Copy { content: String },
    Regenerate { conv_id: String, idx: usize },
    Delete { conv_id: String, idx: usize },
    Preview { attachment: Attachment },
    Play { local_path: String },
    Download { attachment: Attachment },
}

pub struct MessageWebView {
    webview: WebView,
    rx: Receiver<String>,
}

impl MessageWebView {
    pub fn new(parent: &impl wry::raw_window_handle::HasWindowHandle) -> Result<Self> {
        let (tx, rx) = channel::<String>();
        let webview = WebViewBuilder::new()
            .with_html(SHELL_HTML)
            .with_ipc_handler(move |req: Request<String>| {
                let _ = tx.send(req.body().clone());
            })
            .build_as_child(parent)
            .context("创建 WebView 失败")?;
        Ok(Self { webview, rx })
    }

    pub fn set_visible(&self, visible: bool) -> Result<()> {
        self.webview
            .set_visible(visible)
            .context("设置 WebView 可见性失败")
    }

    pub fn set_bounds(&self, rect: wry::Rect) -> Result<()> {
        self.webview.set_bounds(rect).context("设置 WebView 位置失败")
    }

    pub fn set_content(&self, html: &str) -> Result<()> {
        let script = format!(
            "setContent({});",
            serde_json::Value::String(html.to_string())
        );
        self.webview
            .evaluate_script(&script)
            .context("更新 WebView 内容失败")
    }

    pub fn drain_actions(&mut self) -> Vec<WebViewAction> {
        let mut actions = Vec::new();
        while let Ok(msg) = self.rx.try_recv() {
            if let Ok(action) = parse_action(&msg) {
                actions.push(action);
            }
        }
        actions
    }
}

fn parse_action(json: &str) -> anyhow::Result<WebViewAction> {
    let v: serde_json::Value = serde_json::from_str(json)?;
    let action = v["action"].as_str().unwrap_or("");
    match action {
        "copy" => {
            let content = v["content"].as_str().unwrap_or("").to_string();
            Ok(WebViewAction::Copy { content })
        }
        "regenerate" => {
            let conv_id = v["convId"].as_str().unwrap_or("").to_string();
            let idx = v["idx"].as_u64().unwrap_or(0) as usize;
            Ok(WebViewAction::Regenerate { conv_id, idx })
        }
        "delete" => {
            let conv_id = v["convId"].as_str().unwrap_or("").to_string();
            let idx = v["idx"].as_u64().unwrap_or(0) as usize;
            Ok(WebViewAction::Delete { conv_id, idx })
        }
        "preview" | "download" => {
            let kind = match v["kind"].as_str().unwrap_or("") {
                "video" => AttachmentKind::Video,
                _ => AttachmentKind::Image,
            };
            let file_name = v["fileName"].as_str().unwrap_or("").to_string();
            let local_path = v["localPath"].as_str().unwrap_or("").to_string();
            let attachment = Attachment {
                kind,
                file_name,
                local_path,
                source_url: String::new(),
            };
            if action == "preview" {
                Ok(WebViewAction::Preview { attachment })
            } else {
                Ok(WebViewAction::Download { attachment })
            }
        }
        "play" => {
            let local_path = v["localPath"].as_str().unwrap_or("").to_string();
            Ok(WebViewAction::Play { local_path })
        }
        _ => anyhow::bail!("未知 WebView 动作: {}", action),
    }
}

pub fn conversation_html(conv: &Conversation) -> String {
    let mut messages_html = String::new();
    for (idx, msg) in conv.messages.iter().enumerate() {
        if msg.role == "system" {
            continue;
        }
        let is_user = msg.role == "user";
        let role_class = if is_user { "user" } else { "assistant" };
        let actions_class = if is_user {
            "actions user-actions"
        } else {
            "actions assistant-actions"
        };

        let content_html = markdown_to_html(&msg.content);

        let mut attachments_html = String::new();
        for att in &msg.attachments {
            match att.kind {
                AttachmentKind::Image => {
                    attachments_html.push_str(&format!(
                        r#"<div class="attachment">
  <img src="file://{}" style="max-width:100%;border-radius:6px;cursor:pointer;" onclick="sendAttachmentAction('preview','{}','{}','{}')">
  <div class="attachment-actions">
    <button onclick="sendAttachmentAction('preview','{}','{}','{}')">预览</button>
    <button onclick="sendAttachmentAction('download','{}','{}','{}')">下载</button>
  </div>
</div>"#,
                        html_escape(&att.local_path),
                        att.kind.kind_str(),
                        js_escape(&att.file_name),
                        js_escape(&att.local_path),
                        att.kind.kind_str(),
                        js_escape(&att.file_name),
                        js_escape(&att.local_path),
                        att.kind.kind_str(),
                        js_escape(&att.file_name),
                        js_escape(&att.local_path),
                    ));
                }
                AttachmentKind::Video => {
                    attachments_html.push_str(&format!(
                        r#"<div class="attachment">
  <div>视频: {}</div>
  <div class="attachment-actions">
    <button onclick="sendAttachmentAction('play','{}','{}','{}')">播放</button>
    <button onclick="sendAttachmentAction('download','{}','{}','{}')">下载</button>
  </div>
</div>"#,
                        html_escape(&att.file_name),
                        att.kind.kind_str(),
                        js_escape(&att.file_name),
                        js_escape(&att.local_path),
                        att.kind.kind_str(),
                        js_escape(&att.file_name),
                        js_escape(&att.local_path),
                    ));
                }
            }
        }

        messages_html.push_str(&format!(
            r#"<div class="msg">
  <div class="bubble {role_class}">
    {content_html}
    {attachments_html}
  </div>
  <div class="{actions_class}">
    <button onclick="copyText({})">复制</button>
    {}
    <button onclick="sendAction('delete','{}',{})">删除</button>
  </div>
</div>"#,
            js_escape(&msg.content),
            if msg.role == "assistant" && conv.kind == crate::types::ConversationKind::Chat {
                format!(
                    r#"<button onclick="sendAction('regenerate','{}',{})">重新生成</button>"#,
                    js_escape(&conv.id),
                    idx
                )
            } else {
                String::new()
            },
            js_escape(&conv.id),
            idx,
        ));
    }

    format!(
        r#"<div id="messages" data-conv-id="{}">{}</div>"#,
        html_escape(&conv.id),
        messages_html
    )
}

impl AttachmentKind {
    fn kind_str(&self) -> &'static str {
        match self {
            AttachmentKind::Image => "image",
            AttachmentKind::Video => "video",
        }
    }
}

fn markdown_to_html(text: &str) -> String {
    let text = preprocess(text);
    let parser = Parser::new_ext(
        &text,
        Options::ENABLE_MATH | Options::ENABLE_STRIKETHROUGH,
    );
    let events = parser.map(|event| match event {
        Event::InlineMath(math) => {
            let escaped = html_escape(&math);
            Event::Html(CowStr::Boxed(
                format!("<span class=\"math-inline\">${}${}</span>", escaped, "")
                    .into_boxed_str(),
            ))
        }
        Event::DisplayMath(math) => {
            let escaped = html_escape(&math);
            Event::Html(CowStr::Boxed(
                format!(
                    "<div class=\"math-display\">$${}$$</div>",
                    escaped
                )
                .into_boxed_str(),
            ))
        }
        _ => event,
    });
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, events);
    html
}

fn preprocess(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        if line.starts_with("'''") {
            out.push_str(&line.replacen("'''", "```", 1));
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }

    let re_display = regex::Regex::new(r"\\\[(?s)(.*?)\\\]").unwrap();
    let re_inline = regex::Regex::new(r"\\\((?s)(.*?)\\\)").unwrap();
    let out = re_display.replace_all(&out, |caps: &regex::Captures| {
        format!("$${}$$", &caps[1])
    });
    let out = re_inline.replace_all(&out, |caps: &regex::Captures| {
        format!("${}$", &caps[1])
    });
    out.into_owned()
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn js_escape(s: &str) -> String {
    serde_json::Value::String(s.to_string()).to_string()
}
