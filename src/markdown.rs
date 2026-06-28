use pulldown_cmark::Event;

pub(crate) fn preprocess(text: &str) -> String {
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

pub fn formula_key(latex: &str, display: bool) -> String {
    format!("v2:{}:{}", if display { "d" } else { "i" }, latex)
}

/// 根据 LaTeX 内容计算块级公式相对于正文高度的倍数。
/// 简单公式 1.5 倍；含分数 ×3，含根号 ×1.8，含次方 ×1.8；可叠加，最大 3 倍。
pub fn display_formula_height_multiplier(latex: &str) -> f32 {
    let frac_count = latex.matches(r"\frac").count();
    let sqrt_count = latex.matches(r"\sqrt").count();
    let exp_count = latex.matches('^').count();
    let mut multiplier =
        1.5_f32 * 3.0_f32.powi(frac_count as i32) * 1.8_f32.powi((sqrt_count + exp_count) as i32);
    multiplier = multiplier.min(3.0);
    multiplier
}

pub fn extract_formulas(text: &str) -> Vec<(String, bool)> {
    let text = preprocess(text);
    let parser = pulldown_cmark::Parser::new_ext(
        &text,
        pulldown_cmark::Options::ENABLE_MATH | pulldown_cmark::Options::ENABLE_STRIKETHROUGH,
    );
    let mut formulas = Vec::new();
    for event in parser {
        match event {
            Event::InlineMath(latex) => formulas.push((latex.to_string(), false)),
            Event::DisplayMath(latex) => formulas.push((latex.to_string(), true)),
            _ => {}
        }
    }
    formulas
}

// ---------------------------------------------------------------------------
// Iced 渲染
// ---------------------------------------------------------------------------

use crate::latex;
use crate::Message;
use iced::border::Radius;
use iced::font::Font;
use iced::widget::text::Shaping;
use iced::widget::{container, image, row, scrollable, text, Column, Space};
use iced::{Alignment, Color, Element, Length, Padding};
use std::collections::HashMap;

pub const FONT_BODY: Font = Font::with_name("Source Han Sans SC");
pub const FONT_CODE: Font = Font::with_name("Maple Mono NF CN");

const BODY_SIZE: f32 = 14.0;
const CODE_SIZE: f32 = 13.0;
const MAX_LINE_EMS_WIDE: f32 = 80.0;
const MAX_LINE_EMS_NARROW: f32 = 30.0;
const DISPLAY_MATH_MAX_WIDTH: f32 = 560.0;
const DISPLAY_MATH_MAX_WIDTH_NARROW: f32 = 300.0;
const BODY_LINE_HEIGHT: f32 = BODY_SIZE * 1.3;

#[derive(Clone, Debug)]
pub struct FormulaTexture {
    pub handle: image::Handle,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug)]
enum Inline {
    Text(String),
    Code(String),
    Math(String, bool),
    HardBreak,
}

#[derive(Clone, Debug)]
enum Block {
    Paragraph(Vec<Inline>),
    Heading(u8, Vec<Inline>),
    CodeBlock(String, String),
    DisplayMath(String),
}

/// 渲染整条消息为 Iced 元素。
pub fn render_message<'a>(
    content: &'a str,
    formula_textures: &'a HashMap<String, FormulaTexture>,
    narrow: bool,
) -> Element<'a, Message> {
    let max_line_ems = if narrow {
        MAX_LINE_EMS_NARROW
    } else {
        MAX_LINE_EMS_WIDE
    };
    let blocks = parse_blocks(content);
    let mut children: Vec<Element<'_, Message>> = Vec::with_capacity(blocks.len());
    for block in blocks {
        children.push(render_block(block, formula_textures, max_line_ems, narrow));
    }
    Column::with_children(children).spacing(10).into()
}

fn parse_blocks(input: &str) -> Vec<Block> {
    let input = preprocess(input);
    let parser = pulldown_cmark::Parser::new_ext(
        &input,
        pulldown_cmark::Options::ENABLE_MATH | pulldown_cmark::Options::ENABLE_STRIKETHROUGH,
    );

    let mut blocks: Vec<Block> = Vec::new();
    let mut current: Vec<Inline> = Vec::new();
    let mut in_code = false;
    let mut code = String::new();
    let mut code_lang = String::new();
    let mut heading_level: u8 = 1;

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                pulldown_cmark::Tag::Paragraph => current.clear(),
                pulldown_cmark::Tag::Heading {
                    level, ..
                } => {
                    current.clear();
                    heading_level = level as u8;
                }
                pulldown_cmark::Tag::CodeBlock(lang) => {
                    in_code = true;
                    code.clear();
                    code_lang = match lang {
                        pulldown_cmark::CodeBlockKind::Fenced(l) => l.to_string(),
                        _ => String::new(),
                    };
                }
                pulldown_cmark::Tag::Item => {
                    current.push(Inline::Text("• ".to_string()));
                }
                _ => {}
            },
            Event::End(tag_end) => match tag_end {
                pulldown_cmark::TagEnd::Paragraph => {
                    if !current.is_empty() {
                        blocks.push(Block::Paragraph(current.clone()));
                        current.clear();
                    }
                }
                pulldown_cmark::TagEnd::Heading(_) => {
                    if !current.is_empty() {
                        blocks.push(Block::Heading(heading_level, current.clone()));
                        current.clear();
                    }
                }
                pulldown_cmark::TagEnd::CodeBlock => {
                    in_code = false;
                    blocks.push(Block::CodeBlock(code_lang.clone(), code.clone()));
                    code.clear();
                    code_lang.clear();
                }
                pulldown_cmark::TagEnd::Item => {
                    if !current.is_empty() {
                        blocks.push(Block::Paragraph(current.clone()));
                        current.clear();
                    }
                }
                _ => {}
            },
            Event::Text(t) => {
                if in_code {
                    code.push_str(&t);
                } else {
                    current.push(Inline::Text(t.to_string()));
                }
            }
            Event::Code(c) => {
                current.push(Inline::Code(c.to_string()));
            }
            Event::InlineMath(latex) => {
                current.push(Inline::Math(latex.to_string(), false));
            }
            Event::DisplayMath(latex) => {
                if !current.is_empty() {
                    blocks.push(Block::Paragraph(current.clone()));
                    current.clear();
                }
                blocks.push(Block::DisplayMath(latex.to_string()));
            }
            Event::SoftBreak => {
                if !in_code {
                    current.push(Inline::Text(" ".to_string()));
                } else {
                    code.push('\n');
                }
            }
            Event::HardBreak => {
                if !in_code {
                    current.push(Inline::HardBreak);
                } else {
                    code.push('\n');
                }
            }
            _ => {}
        }
    }

    if in_code && !code.is_empty() {
        blocks.push(Block::CodeBlock(code_lang, code));
    } else if !current.is_empty() {
        blocks.push(Block::Paragraph(current));
    }

    blocks
}

fn render_block(
    block: Block,
    formula_textures: &HashMap<String, FormulaTexture>,
    max_line_ems: f32,
    narrow: bool,
) -> Element<'_, Message> {
    match block {
        Block::Paragraph(inlines) => render_paragraph(inlines, formula_textures, BODY_SIZE, max_line_ems),
        Block::Heading(level, inlines) => {
            let size = match level {
                1 => 20.0,
                2 => 18.0,
                _ => 16.0,
            };
            render_paragraph(inlines, formula_textures, size, max_line_ems)
        }
        Block::CodeBlock(_lang, code) => render_code_block(code),
        Block::DisplayMath(latex) => render_display_math(latex, formula_textures, narrow),
    }
}

fn render_paragraph(
    inlines: Vec<Inline>,
    formula_textures: &HashMap<String, FormulaTexture>,
    font_size: f32,
    max_line_ems: f32,
) -> Element<'_, Message> {
    let lines = wrap_inlines(inlines, font_size, max_line_ems);
    let rows: Vec<Element<'_, Message>> = lines
        .into_iter()
        .map(|line| {
            let mut row = row![];
            for inline in line {
                row = row.push(render_inline(inline, formula_textures, font_size));
            }
            row.spacing(2).align_y(Alignment::Center).into()
        })
        .collect();

    Column::with_children(rows).spacing(2).into()
}

fn render_inline(
    inline: Inline,
    formula_textures: &HashMap<String, FormulaTexture>,
    font_size: f32,
) -> Element<'_, Message> {
    match inline {
        Inline::Text(s) => text(s)
            .font(FONT_BODY)
            .size(font_size)
            .shaping(Shaping::Advanced)
            .into(),
        Inline::Code(s) => container(
            text(s)
                .font(FONT_CODE)
                .size(CODE_SIZE)
                .shaping(Shaping::Advanced),
        )
        .padding(Padding::from([2, 4]))
        .style(|_| container::Style {
            background: Some(iced::Background::Color(Color::from_rgb(0.94, 0.94, 0.94))),
            border: iced::Border {
                radius: Radius::from(4.0),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            ..container::Style::default()
        })
        .into(),
        Inline::Math(latex, display) => {
            if display {
                render_display_math(latex, formula_textures, false)
            } else {
                render_inline_math(latex, formula_textures, font_size)
            }
        }
        Inline::HardBreak => Space::new().height(Length::Fixed(font_size)).into(),
    }
}

fn render_inline_math(
    latex: String,
    formula_textures: &HashMap<String, FormulaTexture>,
    font_size: f32,
) -> Element<'_, Message> {
    let key = formula_key(&latex, false);
    if let Some(tex) = formula_textures.get(&key) {
        // 行内公式高度按当前字号行高（1.3 倍）显示，避免和正文一样高
        let target_height = font_size * 1.3;
        let (w, h) = inline_formula_size(tex.width, tex.height, target_height);
        return image(tex.handle.clone())
            .width(Length::Fixed(w))
            .height(Length::Fixed(h))
            .into();
    }

    // 回退：Unicode 线性近似
    let fallback = latex::render_inline_latex(&latex);
    text(fallback)
        .font(FONT_BODY)
        .size(font_size)
        .shaping(Shaping::Advanced)
        .into()
}

fn render_display_math(
    latex: String,
    formula_textures: &HashMap<String, FormulaTexture>,
    narrow: bool,
) -> Element<'_, Message> {
    let key = formula_key(&latex, true);
    if let Some(tex) = formula_textures.get(&key) {
        let multiplier = display_formula_height_multiplier(&latex);
        let target_height = BODY_LINE_HEIGHT * multiplier;
        let max_width = if narrow {
            DISPLAY_MATH_MAX_WIDTH_NARROW
        } else {
            DISPLAY_MATH_MAX_WIDTH
        };

        // 按目标高度缩放后的实际宽高
        let scale = if tex.height == 0 {
            1.0
        } else {
            target_height / tex.height as f32
        };
        let formula_width = tex.width as f32 * scale;
        let formula_height = target_height;

        // 公式宽度超过允许范围，或处于窄屏模式时，使用横向滚动而非压缩
        if narrow || formula_width > max_width {
            let scroll = scrollable(
                // 底部留出滚动条高度，避免遮挡公式
                container(
                    image(tex.handle.clone())
                        .width(Length::Fixed(formula_width))
                        .height(Length::Fixed(formula_height)),
                )
                .padding(Padding { top: 0.0, right: 0.0, bottom: 12.0, left: 0.0 }),
            )
            .direction(iced::widget::scrollable::Direction::Horizontal(
                iced::widget::scrollable::Scrollbar::default(),
            ))
            .width(Length::Fill)
            .height(Length::Shrink);
            return container(scroll).width(Length::Fill).padding(4).into();
        }

        let (w, h) = (formula_width, formula_height);
        return container(image(tex.handle.clone()).width(Length::Fixed(w)).height(Length::Fixed(h)))
            .width(Length::Fill)
            .align_x(Alignment::Center)
            .padding(4)
            .into();
    }

    let fallback = latex::render_inline_latex(&latex);
    container(
        text(fallback)
            .font(FONT_BODY)
            .size(BODY_SIZE)
            .shaping(Shaping::Advanced),
    )
    .width(Length::Fill)
    .align_x(Alignment::Center)
    .padding(4)
    .into()
}

fn render_code_block(code: String) -> Element<'static, Message> {
    let lines: Vec<Element<'static, Message>> = code
        .lines()
        .map(|line| {
            if line.is_empty() {
                Space::new().height(Length::Fixed(CODE_SIZE)).into()
            } else {
                text(line.to_string())
                    .font(FONT_CODE)
                    .size(CODE_SIZE)
                    .shaping(Shaping::Advanced)
                    .into()
            }
        })
        .collect();

    container(scrollable(Column::with_children(lines).spacing(2)).height(Length::Shrink))
        .padding(10)
        .width(Length::Fill)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(Color::from_rgb(0.96, 0.96, 0.96))),
            border: iced::Border {
                radius: Radius::from(6.0),
                width: 1.0,
                color: Color::from_rgb(0.90, 0.90, 0.90),
            },
            ..container::Style::default()
        })
        .into()
}

// ---------------------------------------------------------------------------
// 公式尺寸
// ---------------------------------------------------------------------------

fn inline_formula_size(width: u32, height: u32, target_height: f32) -> (f32, f32) {
    if height == 0 {
        return (target_height, target_height);
    }
    let scale = target_height / height as f32;
    (width as f32 * scale, target_height)
}

pub fn render_formula_texture(latex: &str, display: bool) -> Option<FormulaTexture> {
    let png = latex::render_formula_png(latex, display)?;
    let img = ::image::load_from_memory(&png).ok()?;
    Some(FormulaTexture {
        handle: image::Handle::from_bytes(png),
        width: img.width(),
        height: img.height(),
    })
}

// ---------------------------------------------------------------------------
// 行内排版换行（按 em 估算）
// ---------------------------------------------------------------------------

fn wrap_inlines(inlines: Vec<Inline>, font_size: f32, max_line_ems: f32) -> Vec<Vec<Inline>> {
    let mut lines: Vec<Vec<Inline>> = vec![vec![]];
    let mut current_width_ems = 0.0;

    for inline in inlines {
        let chunks = split_inline(&inline, font_size, max_line_ems);
        for (chunk, width_ems) in chunks {
            if current_width_ems + width_ems > max_line_ems && !lines.last().unwrap().is_empty() {
                lines.push(vec![]);
                current_width_ems = 0.0;
            }
            current_width_ems += width_ems;
            lines.last_mut().unwrap().push(chunk);
        }
    }

    lines
}

fn split_inline(inline: &Inline, font_size: f32, max_line_ems: f32) -> Vec<(Inline, f32)> {
    match inline {
        Inline::HardBreak => vec![(Inline::HardBreak, 0.0)],
        Inline::Code(s) => {
            let chunks = chunk_text(s, max_line_ems, true);
            chunks.into_iter().map(|c| (Inline::Code(c.0), c.1)).collect()
        }
        Inline::Text(s) => {
            let chunks = chunk_text(s, max_line_ems, false);
            chunks.into_iter().map(|c| (Inline::Text(c.0), c.1)).collect()
        }
        Inline::Math(latex, display) => {
            // 公式不拆分；如果超过行宽就在 wrap_inlines 里单独占一行
            let w = math_fallback_width_ems(latex, *display, font_size, max_line_ems);
            vec![(Inline::Math(latex.clone(), *display), w)]
        }
    }
}

fn chunk_text(s: &str, max_ems: f32, is_code: bool) -> Vec<(String, f32)> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut buf_ems = 0.0;
    let em_scale = if is_code { 0.58 } else { 1.0 };

    for c in s.chars() {
        let cw = char_width_em(c) * em_scale;
        if buf_ems + cw > max_ems && !buf.is_empty() {
            out.push((buf.clone(), buf_ems));
            buf.clear();
            buf_ems = 0.0;
        }
        buf.push(c);
        buf_ems += cw;
    }
    if !buf.is_empty() {
        out.push((buf, buf_ems));
    }
    out
}

fn math_fallback_width_ems(latex: &str, display: bool, _font_size: f32, max_line_ems: f32) -> f32 {
    if display {
        // 块级公式独占一行，宽度视为行宽，触发换行后会单独成行
        max_line_ems
    } else {
        let fallback = latex::render_inline_latex(latex);
        text_width_ems(&fallback, false)
    }
}

fn text_width_ems(s: &str, is_code: bool) -> f32 {
    let em_scale = if is_code { 0.58 } else { 1.0 };
    s.chars().map(|c| char_width_em(c) * em_scale).sum()
}

fn char_width_em(c: char) -> f32 {
    if c.is_whitespace() {
        0.3
    } else if is_cjk(c) {
        1.0
    } else if c.is_ascii() {
        0.5
    } else {
        0.7
    }
}

fn is_cjk(c: char) -> bool {
    matches!(
        c as u32,
        0x4E00..=0x9FFF
            | 0x3400..=0x4DBF
            | 0xF900..=0xFAFF
            | 0x3040..=0x309F
            | 0x30A0..=0x30FF
            | 0xAC00..=0xD7AF
            | 0xFF00..=0xFF60
            | 0x3000..=0x303F
    )
}

