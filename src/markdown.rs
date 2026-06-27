use crate::latex;
use egui::{
    text::{LayoutJob, TextFormat},
    Align, Color32, FontFamily, FontId, RichText, Stroke, TextWrapMode, Ui, Widget,
};
use pulldown_cmark::{CodeBlockKind, Event, Tag, TagEnd};

pub fn render_markdown(ui: &mut Ui, text: &str) {
    let text = preprocess(text);
    let parser = pulldown_cmark::Parser::new_ext(
        &text,
        pulldown_cmark::Options::ENABLE_MATH | pulldown_cmark::Options::ENABLE_STRIKETHROUGH,
    );
    let mut state = MarkdownState::new(ui);
    for event in parser {
        state.handle(event, ui);
    }
    state.flush(ui);
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
    let out = re_display.replace_all(&out, |caps: &regex::Captures| format!("$${}$$", &caps[1]));
    let out = re_inline.replace_all(&out, |caps: &regex::Captures| format!("${}$", &caps[1]));
    out.into_owned()
}

struct ListState {
    ordered: bool,
    next_index: u64,
}

struct MarkdownState {
    list_stack: Vec<ListState>,
    in_blockquote: bool,
    in_code_block: Option<String>,
    code_buffer: String,
    inline_job: LayoutJob,
    strong: bool,
    emphasis: bool,
    code: bool,
    strikethrough: bool,
    link_url: Option<String>,
    heading_level: Option<u8>,
    ui_text_color: Color32,
    body_font: FontId,
    bold_font: FontId,
    code_font: FontId,
}

impl MarkdownState {
    fn new(ui: &Ui) -> Self {
        let body_font = egui::TextStyle::Body.resolve(ui.style());
        let code_size = egui::TextStyle::Small.resolve(ui.style()).size;
        Self {
            list_stack: Vec::new(),
            in_blockquote: false,
            in_code_block: None,
            code_buffer: String::new(),
            inline_job: LayoutJob::default(),
            strong: false,
            emphasis: false,
            code: false,
            strikethrough: false,
            link_url: None,
            heading_level: None,
            ui_text_color: ui.visuals().text_color(),
            body_font: body_font.clone(),
            bold_font: FontId::new(body_font.size, FontFamily::Name("Bold".into())),
            code_font: FontId::new(code_size, FontFamily::Monospace),
        }
    }

    fn heading_font(&self, level: u8) -> FontId {
        let size = match level {
            1 => 28.0,
            2 => 24.0,
            3 => 21.0,
            4 => 19.0,
            5 => 17.0,
            _ => 16.0,
        };
        FontId::new(size, FontFamily::Name("Bold".into()))
    }

    fn current_format(&self) -> TextFormat {
        let mut fmt = TextFormat {
            font_id: self.body_font.clone(),
            color: self.ui_text_color,
            ..Default::default()
        };

        if self.in_blockquote {
            fmt.color = Color32::from_rgb(100, 100, 100);
        }

        if let Some(level) = self.heading_level {
            fmt.font_id = self.heading_font(level);
        } else if self.code || self.in_code_block.is_some() {
            fmt.font_id = self.code_font.clone();
            fmt.background = Color32::from_rgb(240, 240, 240);
        } else if self.strong {
            fmt.font_id = self.bold_font.clone();
        }

        if self.emphasis {
            fmt.italics = true;
        }

        if self.strikethrough {
            fmt.strikethrough = Stroke::new(1.0, fmt.color);
        }

        if let Some(url) = &self.link_url {
            fmt.color = Color32::BLUE;
            fmt.underline = Stroke::new(1.0, Color32::BLUE);
            let _ = url;
        }

        fmt
    }

    fn push_text(&mut self, text: &str) {
        let fmt = self.current_format();
        self.inline_job.append(text, 0.0, fmt);
    }

    fn flush(&mut self, ui: &mut Ui) {
        if !self.inline_job.text.is_empty() {
            self.inline_job.wrap.max_width = ui.available_width();
            self.inline_job.halign = Align::LEFT;
            egui::Label::new(self.inline_job.clone()).wrap().ui(ui);
            self.inline_job = LayoutJob::default();
        }
    }

    fn handle(&mut self, event: Event<'_>, ui: &mut Ui) {
        match event {
            Event::Start(tag) => self.start_tag(tag, ui),
            Event::End(tag) => self.end_tag(tag, ui),
            Event::Text(text) => {
                if self.in_code_block.is_some() {
                    self.code_buffer.push_str(&text);
                } else {
                    self.push_text(&text);
                }
            }
            Event::Code(code) => {
                self.code = true;
                self.push_text(&code);
                self.code = false;
            }
            Event::InlineMath(math) => {
                let rendered = latex::render_inline_latex(&math);
                let saved_strong = self.strong;
                let saved_emphasis = self.emphasis;
                let saved_code = self.code;
                self.strong = false;
                self.emphasis = false;
                self.code = true;
                self.push_text(&rendered);
                self.strong = saved_strong;
                self.emphasis = saved_emphasis;
                self.code = saved_code;
            }
            Event::DisplayMath(math) => {
                self.flush(ui);
                render_math_block(ui, &math);
            }
            Event::SoftBreak => self.push_text(" "),
            Event::HardBreak => self.push_text("\n"),
            Event::Rule => {
                self.flush(ui);
                ui.separator();
            }
            Event::Html(html) | Event::InlineHtml(html) => {
                if self.in_code_block.is_none() {
                    self.push_text(&html);
                }
            }
            Event::FootnoteReference(_) | Event::TaskListMarker(_) => {}
        }
    }

    fn start_tag(&mut self, tag: Tag<'_>, ui: &mut Ui) {
        match tag {
            Tag::Paragraph => {}
            Tag::Heading { level, .. } => {
                self.heading_level = Some(level as u8);
            }
            Tag::BlockQuote(_) => {
                self.in_blockquote = true;
            }
            Tag::CodeBlock(kind) => {
                self.flush(ui);
                let lang = match kind {
                    CodeBlockKind::Fenced(lang) => lang.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                self.in_code_block = Some(lang);
                self.code_buffer.clear();
            }
            Tag::List(start) => {
                self.flush(ui);
                self.list_stack.push(ListState {
                    ordered: start.is_some(),
                    next_index: start.unwrap_or(1),
                });
            }
            Tag::Item => {
                let depth = self.list_stack.len();
                let indent = "  ".repeat(depth.saturating_sub(1));
                let prefix = if let Some(list) = self.list_stack.last_mut() {
                    if list.ordered {
                        let p = format!("{}{}. ", indent, list.next_index);
                        list.next_index += 1;
                        p
                    } else {
                        format!("{}• ", indent)
                    }
                } else {
                    String::new()
                };
                self.push_text(&prefix);
            }
            Tag::Emphasis => self.emphasis = true,
            Tag::Strong => self.strong = true,
            Tag::Strikethrough => self.strikethrough = true,
            Tag::Link { dest_url, .. } => self.link_url = Some(dest_url.to_string()),
            Tag::Image { dest_url, .. } => {
                self.flush(ui);
                ui.label(RichText::new(format!("[图片: {}]", dest_url)).italics().color(Color32::GRAY));
            }
            _ => {}
        }
    }

    fn end_tag(&mut self, tag: TagEnd, ui: &mut Ui) {
        match tag {
            TagEnd::Paragraph => {
                self.flush(ui);
                ui.add_space(4.0);
            }
            TagEnd::Heading(_) => {
                self.flush(ui);
                self.heading_level = None;
                ui.add_space(6.0);
            }
            TagEnd::BlockQuote(_) => {
                self.flush(ui);
                self.in_blockquote = false;
                ui.add_space(4.0);
            }
            TagEnd::CodeBlock => {
                if let Some(lang) = self.in_code_block.take() {
                    let code = std::mem::take(&mut self.code_buffer);
                    render_code_block(ui, &code, &lang);
                }
            }
            TagEnd::List(_) => {
                self.list_stack.pop();
                ui.add_space(4.0);
            }
            TagEnd::Item => {
                self.flush(ui);
            }
            TagEnd::Emphasis => self.emphasis = false,
            TagEnd::Strong => self.strong = false,
            TagEnd::Strikethrough => self.strikethrough = false,
            TagEnd::Link => self.link_url = None,
            _ => {}
        }
    }
}

fn render_code_block(ui: &mut Ui, code: &str, lang: &str) {
    ui.add_space(4.0);
    let display_code = code.to_string();
    let narrow = ui.ctx().screen_rect().width() < 600.0;
    egui::Frame::group(ui.style())
        .fill(Color32::from_rgb(245, 245, 245))
        .inner_margin(egui::Margin::same(8.0))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                if !lang.is_empty() {
                    ui.label(RichText::new(lang).small().color(Color32::DARK_GRAY));
                    ui.separator();
                }
                if narrow {
                    // 窄屏时使用横向滚动，避免代码被强制换行导致纵向排版混乱
                    egui::ScrollArea::horizontal()
                        .max_width(ui.available_width())
                        .show(ui, |ui| {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(display_code)
                                        .font(FontId::new(16.0, FontFamily::Monospace)),
                                )
                                .wrap_mode(TextWrapMode::Extend),
                            );
                        });
                } else {
                    let code_width = ui.available_width();
                    ui.add(
                        egui::TextEdit::multiline(&mut display_code.clone())
                            .font(FontId::new(16.0, FontFamily::Monospace))
                            .code_editor()
                            .interactive(false)
                            .desired_width(code_width),
                    );
                }
            });
        });
    ui.add_space(4.0);
}

fn render_math_block(ui: &mut Ui, latex: &str) {
    let rendered = latex::render_display_latex(latex).unwrap_or_else(|| latex.to_string());
    ui.add_space(4.0);
    let narrow = ui.ctx().screen_rect().width() < 600.0;
    egui::Frame::group(ui.style())
        .fill(Color32::from_rgb(245, 248, 255))
        .inner_margin(egui::Margin::same(10.0))
        .show(ui, |ui| {
            if narrow {
                // 窄屏下公式块横向滚动，避免自动换行破坏排版
                egui::ScrollArea::horizontal()
                    .max_width(ui.available_width())
                    .show(ui, |ui| {
                        ui.add(
                            egui::Label::new(
                                RichText::new(rendered)
                                    .font(FontId::new(16.0, FontFamily::Monospace)),
                            )
                            .wrap_mode(TextWrapMode::Extend),
                        );
                    });
            } else {
                let math_width = ui.available_width();
                ui.add(
                    egui::TextEdit::multiline(&mut rendered.clone())
                        .font(FontId::new(16.0, FontFamily::Monospace))
                        .interactive(false)
                        .desired_width(math_width),
                );
            }
        });
    ui.add_space(4.0);
}
