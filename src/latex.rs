/// 使用 RaTeX 把 LaTeX 渲染为 PNG 字节。
/// display 为 true 时使用展示样式，否则使用行内样式。
pub fn render_formula_png(input: &str, display: bool) -> Option<Vec<u8>> {
    let input = normalize_unicode_math(input);
    let nodes = ratex_parser::parse(&input).ok()?;

    use ratex_layout::{layout, to_display_list, LayoutOptions};
    use ratex_render::{render_to_png, RenderOptions};
    use ratex_types::color::Color;
    use ratex_types::math_style::MathStyle;

    let layout_opts = LayoutOptions {
        style: if display {
            MathStyle::Display
        } else {
            MathStyle::Text
        },
        color: Color::BLACK,
        ..LayoutOptions::default()
    };
    let layout_box = layout(&nodes, &layout_opts);
    let display_list = to_display_list(&layout_box);

    let render_opts = RenderOptions {
        font_size: if display { 20.0 } else { 16.0 },
        padding: 4.0,
        background_color: Color::new(0.0, 0.0, 0.0, 0.0),
        ..RenderOptions::default()
    };

    render_to_png(&display_list, &render_opts).ok()
}

pub fn render_display_latex(input: &str) -> Option<String> {
    let input = normalize_unicode_math(input);
    tui_math::render_latex(&input).ok()
}

/// 把常见的 Unicode 数学符号统一转成 LaTeX 命令，避免发送到渲染服务或
/// 本地 Unicode 回退时出现乱码/缺字。
pub fn normalize_unicode_math(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '≈' => out.push_str("\\approx "),
            '×' => out.push_str("\\times "),
            '÷' => out.push_str("\\div "),
            '≤' => out.push_str("\\leq "),
            '≥' => out.push_str("\\geq "),
            '≠' => out.push_str("\\neq "),
            '∞' => out.push_str("\\infty "),
            '∈' => out.push_str("\\in "),
            '∉' => out.push_str("\\notin "),
            '⊂' => out.push_str("\\subset "),
            '⊆' => out.push_str("\\subseteq "),
            '∪' => out.push_str("\\cup "),
            '∩' => out.push_str("\\cap "),
            '∅' => out.push_str("\\emptyset "),
            '∀' => out.push_str("\\forall "),
            '∃' => out.push_str("\\exists "),
            '∂' => out.push_str("\\partial "),
            '∇' => out.push_str("\\nabla "),
            '·' => out.push_str("\\cdot "),
            '…' => out.push_str("\\ldots "),
            '±' => out.push_str("\\pm "),
            '∓' => out.push_str("\\mp "),
            '→' => out.push_str("\\rightarrow "),
            '←' => out.push_str("\\leftarrow "),
            '⇒' => out.push_str("\\Rightarrow "),
            '⇐' => out.push_str("\\Leftarrow "),
            '↔' => out.push_str("\\leftrightarrow "),
            '√' => out.push_str("\\sqrt "),
            'π' => out.push_str("\\pi "),
            'Σ' => out.push_str("\\Sigma "),
            'Π' => out.push_str("\\Pi "),
            'α' => out.push_str("\\alpha "),
            'β' => out.push_str("\\beta "),
            'γ' => out.push_str("\\gamma "),
            'δ' => out.push_str("\\delta "),
            'θ' => out.push_str("\\theta "),
            'λ' => out.push_str("\\lambda "),
            'μ' => out.push_str("\\mu "),
            'σ' => out.push_str("\\sigma "),
            'τ' => out.push_str("\\tau "),
            'φ' => out.push_str("\\phi "),
            'ω' => out.push_str("\\omega "),
            _ => out.push(c),
        }
    }
    out
}

/// 将行内 LaTeX 渲染为紧凑的 Unicode 字符串，避免纵向排列破坏排版。
/// 遇到无法转换的符号时回退到线性写法（如 `^(...)` / `_(...)`），保证始终有可读输出。
pub fn render_inline_latex(input: &str) -> String {
    let input = normalize_unicode_math(input);
    let chars: Vec<char> = input.chars().collect();
    let mut result = String::new();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            '\\' => {
                i += 1;
                if i >= chars.len() {
                    result.push('\\');
                    break;
                }
                // 转义字符 \{ \} 等
                if chars[i] == '{' || chars[i] == '}' {
                    result.push(chars[i]);
                    i += 1;
                    continue;
                }

                let start = i;
                while i < chars.len() && chars[i].is_alphabetic() {
                    i += 1;
                }
                let cmd: String = chars[start..i].iter().collect();

                match cmd.as_str() {
                    "" => {
                        // 单独的 \\ 在行内当作空格处理
                        if i < chars.len() && chars[i] == '\\' {
                            i += 1;
                            result.push(' ');
                        } else {
                            result.push('\\');
                        }
                    }
                    "alpha" => result.push('α'),
                    "beta" => result.push('β'),
                    "gamma" => result.push('γ'),
                    "Gamma" => result.push('Γ'),
                    "delta" => result.push('δ'),
                    "Delta" => result.push('Δ'),
                    "epsilon" => result.push('ε'),
                    "varepsilon" => result.push('ε'),
                    "zeta" => result.push('ζ'),
                    "eta" => result.push('η'),
                    "theta" => result.push('θ'),
                    "Theta" => result.push('Θ'),
                    "vartheta" => result.push('ϑ'),
                    "iota" => result.push('ι'),
                    "kappa" => result.push('κ'),
                    "lambda" => result.push('λ'),
                    "Lambda" => result.push('Λ'),
                    "mu" => result.push('μ'),
                    "nu" => result.push('ν'),
                    "xi" => result.push('ξ'),
                    "Xi" => result.push('Ξ'),
                    "pi" => result.push('π'),
                    "Pi" => result.push('Π'),
                    "rho" => result.push('ρ'),
                    "sigma" => result.push('σ'),
                    "Sigma" => result.push('Σ'),
                    "tau" => result.push('τ'),
                    "upsilon" => result.push('υ'),
                    "phi" => result.push('φ'),
                    "Phi" => result.push('Φ'),
                    "varphi" => result.push('φ'),
                    "chi" => result.push('χ'),
                    "psi" => result.push('ψ'),
                    "Psi" => result.push('Ψ'),
                    "omega" => result.push('ω'),
                    "Omega" => result.push('Ω'),
                    "pm" => result.push('±'),
                    "times" => result.push('×'),
                    "cdot" => result.push('·'),
                    "div" => result.push('÷'),
                    "infty" | "infinity" => result.push('∞'),
                    "int" => result.push('∫'),
                    "sum" => result.push('Σ'),
                    "prod" => result.push('Π'),
                    "sqrt" => {
                        while i < chars.len() && chars[i].is_whitespace() {
                            i += 1;
                        }
                        if let Some(inner) = parse_single_or_group(&chars, &mut i) {
                            result.push('√');
                            result.push('(');
                            result.push_str(&render_inline_latex(&inner));
                            result.push(')');
                        }
                    }
                    "frac" => {
                        if let Some(num) = parse_single_or_group(&chars, &mut i) {
                            while i < chars.len() && chars[i].is_whitespace() {
                                i += 1;
                            }
                            if let Some(den) = parse_single_or_group(&chars, &mut i) {
                                result.push('(');
                                result.push_str(&render_inline_latex(&num));
                                result.push(')');
                                result.push('/');
                                result.push('(');
                                result.push_str(&render_inline_latex(&den));
                                result.push(')');
                            }
                        }
                    }
                    "binom" => {
                        if let Some(n) = parse_single_or_group(&chars, &mut i) {
                            while i < chars.len() && chars[i].is_whitespace() {
                                i += 1;
                            }
                            if let Some(k) = parse_single_or_group(&chars, &mut i) {
                                result.push('C');
                                result.push('(');
                                result.push_str(&render_inline_latex(&n));
                                result.push(',');
                                result.push_str(&render_inline_latex(&k));
                                result.push(')');
                            }
                        }
                    }
                    "leq" | "le" => result.push('≤'),
                    "geq" | "ge" => result.push('≥'),
                    "neq" => result.push('≠'),
                    "approx" => result.push('≈'),
                    "equiv" => result.push('≡'),
                    "sim" => result.push('∼'),
                    "simeq" => result.push('≃'),
                    "cong" => result.push('≅'),
                    "propto" => result.push('∝'),
                    "rightarrow" | "to" => result.push('→'),
                    "leftarrow" => result.push('←'),
                    "Rightarrow" => result.push('⇒'),
                    "Leftarrow" => result.push('⇐'),
                    "leftrightarrow" => result.push('↔'),
                    "mapsto" => result.push('↦'),
                    "in" => result.push('∈'),
                    "notin" => result.push('∉'),
                    "subset" => result.push('⊂'),
                    "subseteq" => result.push('⊆'),
                    "supset" => result.push('⊃'),
                    "supseteq" => result.push('⊇'),
                    "cup" => result.push('∪'),
                    "cap" => result.push('∩'),
                    "setminus" => result.push('\\'),
                    "emptyset" => result.push('∅'),
                    "forall" => result.push('∀'),
                    "exists" => result.push('∃'),
                    "nexists" => result.push('∄'),
                    "partial" => result.push('∂'),
                    "nabla" => result.push('∇'),
                    "ldots" => result.push('…'),
                    "cdots" => result.push('⋯'),
                    "vdots" => result.push('⋮'),
                    "ddots" => result.push('⋱'),
                    "quad" | "qquad" => result.push(' '),
                    " " | "," | ";" | ":" | "!" => result.push(' '),
                    "&" => result.push(' '),
                    "angle" => result.push('∠'),
                    "perp" => result.push('⊥'),
                    "parallel" => result.push('∥'),
                    "bullet" => result.push('•'),
                    "circ" => result.push('∘'),
                    "star" => result.push('⋆'),
                    "oplus" => result.push('⊕'),
                    "otimes" => result.push('⊗'),
                    "cdotp" => result.push('·'),
                    "text" | "mathrm" | "mathit" | "mathbf" | "boldsymbol" | "mathsf"
                    | "mathtt" | "operatorname" | "mbox" | "hbox" => {
                        while i < chars.len() && chars[i].is_whitespace() {
                            i += 1;
                        }
                        if let Some(inner) = parse_single_or_group(&chars, &mut i) {
                            result.push_str(&inner);
                        } else {
                            result.push_str(&cmd);
                        }
                    }
                    "mathbb" => {
                        while i < chars.len() && chars[i].is_whitespace() {
                            i += 1;
                        }
                        if let Some(inner) = parse_single_or_group(&chars, &mut i) {
                            result.push_str(&render_blackboard_bold(&inner));
                        }
                    }
                    "mathcal" => {
                        while i < chars.len() && chars[i].is_whitespace() {
                            i += 1;
                        }
                        if let Some(inner) = parse_single_or_group(&chars, &mut i) {
                            result.push_str(&render_calligraphic(&inner));
                        }
                    }
                    "mathfrak" => {
                        while i < chars.len() && chars[i].is_whitespace() {
                            i += 1;
                        }
                        if let Some(inner) = parse_single_or_group(&chars, &mut i) {
                            result.push_str(&inner);
                        }
                    }
                    "overline" | "underline" | "hat" | "bar" | "vec" | "dot" | "ddot" | "tilde"
                    | "widehat" | "widetilde" => {
                        while i < chars.len() && chars[i].is_whitespace() {
                            i += 1;
                        }
                        if let Some(inner) = parse_single_or_group(&chars, &mut i) {
                            result.push_str(&render_inline_latex(&inner));
                        }
                    }
                    "left" | "right" => {
                        if i < chars.len() {
                            let c = chars[i];
                            if c == '.' {
                                i += 1;
                            } else if !c.is_alphabetic() {
                                result.push(c);
                                i += 1;
                            }
                        }
                    }
                    "lim" | "log" | "ln" | "lg" | "sin" | "cos" | "tan" | "cot" | "sec" | "csc"
                    | "arcsin" | "arccos" | "arctan" | "sinh" | "cosh" | "tanh" | "coth"
                    | "det" | "dim" | "arg" | "gcd" | "max" | "min" | "sup" | "inf" | "exp"
                    | "Pr" | "ker" | "hom" | "deg" | "bmod" | "pmod" => {
                        result.push_str(&cmd);
                    }
                    "begin" | "end" => {
                        while i < chars.len() && chars[i].is_whitespace() {
                            i += 1;
                        }
                        let _ = parse_single_or_group(&chars, &mut i);
                        result.push(' ');
                    }
                    _ => {
                        // 未知命令：如果有紧跟的分组则渲染分组内容，否则输出命令名
                        if let Some(inner) = try_parse_group_only(&chars, &mut i) {
                            result.push_str(&render_inline_latex(&inner));
                        } else {
                            result.push_str(&cmd);
                        }
                    }
                }
            }
            '^' => {
                i += 1;
                if let Some(sup) = parse_single_or_group(&chars, &mut i) {
                    let rendered = render_inline_latex(&sup);
                    if let Some(sup_str) = to_superscript(&rendered) {
                        result.push_str(&sup_str);
                    } else {
                        result.push('^');
                        result.push('(');
                        result.push_str(&rendered);
                        result.push(')');
                    }
                } else {
                    result.push('^');
                }
            }
            '_' => {
                i += 1;
                if let Some(sub) = parse_single_or_group(&chars, &mut i) {
                    let rendered = render_inline_latex(&sub);
                    if let Some(sub_str) = to_subscript(&rendered) {
                        result.push_str(&sub_str);
                    } else {
                        result.push('_');
                        result.push('(');
                        result.push_str(&rendered);
                        result.push(')');
                    }
                } else {
                    result.push('_');
                }
            }
            '{' => {
                if let Some(inner) = parse_group(&chars, &mut i) {
                    result.push_str(&render_inline_latex(&inner));
                }
            }
            '}' => {
                // 孤立右花括号，跳过
                i += 1;
            }
            c => {
                result.push(c);
                i += 1;
            }
        }
    }

    result
}

fn parse_group(chars: &[char], i: &mut usize) -> Option<String> {
    if *i >= chars.len() || chars[*i] != '{' {
        return None;
    }
    *i += 1;
    let start = *i;
    let mut depth = 1;
    while *i < chars.len() && depth > 0 {
        match chars[*i] {
            '{' => depth += 1,
            '}' => depth -= 1,
            _ => {}
        }
        *i += 1;
    }
    if depth != 0 {
        return None;
    }
    Some(chars[start..*i - 1].iter().collect())
}

fn parse_single_or_group(chars: &[char], i: &mut usize) -> Option<String> {
    if *i >= chars.len() {
        return None;
    }
    if chars[*i] == '{' {
        parse_group(chars, i)
    } else {
        let c = chars[*i];
        *i += 1;
        Some(c.to_string())
    }
}

fn try_parse_group_only(chars: &[char], i: &mut usize) -> Option<String> {
    let mut j = *i;
    while j < chars.len() && chars[j].is_whitespace() {
        j += 1;
    }
    if j < chars.len() && chars[j] == '{' {
        *i = j;
        parse_group(chars, i)
    } else {
        None
    }
}

fn render_blackboard_bold(input: &str) -> String {
    input
        .chars()
        .map(|c| match c {
            'A' => '𝔸',
            'B' => '𝔹',
            'C' => 'ℂ',
            'D' => '𝔻',
            'E' => '𝔼',
            'F' => '𝔽',
            'G' => '𝔾',
            'H' => 'ℍ',
            'I' => '𝕀',
            'J' => '𝕁',
            'K' => '𝕂',
            'L' => '𝕃',
            'M' => '𝕄',
            'N' => 'ℕ',
            'O' => '𝕆',
            'P' => 'ℙ',
            'Q' => 'ℚ',
            'R' => 'ℝ',
            'S' => '𝕊',
            'T' => '𝕋',
            'U' => '𝕌',
            'V' => '𝕍',
            'W' => '𝕎',
            'X' => '𝕏',
            'Y' => '𝕐',
            'Z' => 'ℤ',
            'a' => '𝕒',
            'b' => '𝕓',
            'c' => '𝕔',
            'd' => '𝕕',
            'e' => '𝕖',
            'f' => '𝕗',
            'g' => '𝕘',
            'h' => '𝕙',
            'i' => '𝕚',
            'j' => '𝕛',
            'k' => '𝕜',
            'l' => '𝕝',
            'm' => '𝕞',
            'n' => '𝕟',
            'o' => '𝕠',
            'p' => '𝕡',
            'q' => '𝕢',
            'r' => '𝕣',
            's' => '𝕤',
            't' => '𝕥',
            'u' => '𝕦',
            'v' => '𝕧',
            'w' => '𝕨',
            'x' => '𝕩',
            'y' => '𝕪',
            'z' => '𝕫',
            '0' => '𝟘',
            '1' => '𝟙',
            '2' => '𝟚',
            '3' => '𝟛',
            '4' => '𝟜',
            '5' => '𝟝',
            '6' => '𝟞',
            '7' => '𝟟',
            '8' => '𝟠',
            '9' => '𝟡',
            _ => c,
        })
        .collect()
}

fn render_calligraphic(input: &str) -> String {
    input
        .chars()
        .map(|c| match c {
            'A' => '𝒜',
            'B' => 'ℬ',
            'C' => '𝒞',
            'D' => '𝒟',
            'E' => 'ℰ',
            'F' => 'ℱ',
            'G' => '𝒢',
            'H' => 'ℋ',
            'I' => 'ℐ',
            'J' => '𝒥',
            'K' => '𝒦',
            'L' => 'ℒ',
            'M' => 'ℳ',
            'N' => '𝒩',
            'O' => '𝒪',
            'P' => '𝒫',
            'Q' => '𝒬',
            'R' => 'ℛ',
            'S' => '𝒮',
            'T' => '𝒯',
            'U' => '𝒰',
            'V' => '𝒱',
            'W' => '𝒲',
            'X' => '𝒳',
            'Y' => '𝒴',
            'Z' => '𝒵',
            'a' => '𝒶',
            'b' => '𝒷',
            'c' => '𝒸',
            'd' => '𝒹',
            'e' => 'ℯ',
            'f' => '𝒻',
            'g' => 'ℊ',
            'h' => '𝒽',
            'i' => '𝒾',
            'j' => '𝒿',
            'k' => '𝓀',
            'l' => '𝓁',
            'm' => '𝓂',
            'n' => '𝓃',
            'o' => 'ℴ',
            'p' => '𝓅',
            'q' => '𝓆',
            'r' => '𝓇',
            's' => '𝓈',
            't' => '𝓉',
            'u' => '𝓊',
            'v' => '𝓋',
            'w' => '𝓌',
            'x' => '𝓍',
            'y' => '𝓎',
            'z' => '𝓏',
            _ => c,
        })
        .collect()
}

fn to_superscript(s: &str) -> Option<String> {
    s.chars()
        .map(|c| match c {
            '0' => Some('⁰'),
            '1' => Some('¹'),
            '2' => Some('²'),
            '3' => Some('³'),
            '4' => Some('⁴'),
            '5' => Some('⁵'),
            '6' => Some('⁶'),
            '7' => Some('⁷'),
            '8' => Some('⁸'),
            '9' => Some('⁹'),
            '+' => Some('⁺'),
            '-' => Some('⁻'),
            '=' => Some('⁼'),
            '(' => Some('⁽'),
            ')' => Some('⁾'),
            'a' => Some('ᵃ'),
            'b' => Some('ᵇ'),
            'c' => Some('ᶜ'),
            'd' => Some('ᵈ'),
            'e' => Some('ᵉ'),
            'f' => Some('ᶠ'),
            'g' => Some('ᵍ'),
            'h' => Some('ʰ'),
            'i' => Some('ⁱ'),
            'j' => Some('ʲ'),
            'k' => Some('ᵏ'),
            'l' => Some('ˡ'),
            'm' => Some('ᵐ'),
            'n' => Some('ⁿ'),
            'o' => Some('ᵒ'),
            'p' => Some('ᵖ'),
            'r' => Some('ʳ'),
            's' => Some('ˢ'),
            't' => Some('ᵗ'),
            'u' => Some('ᵘ'),
            'v' => Some('ᵛ'),
            'w' => Some('ʷ'),
            'x' => Some('ˣ'),
            'y' => Some('ʸ'),
            'z' => Some('ᶻ'),
            'A' => Some('ᴬ'),
            'B' => Some('ᴮ'),
            'D' => Some('ᴰ'),
            'E' => Some('ᴱ'),
            'G' => Some('ᴳ'),
            'H' => Some('ᴴ'),
            'I' => Some('ᴵ'),
            'J' => Some('ᴶ'),
            'K' => Some('ᴷ'),
            'L' => Some('ᴸ'),
            'M' => Some('ᴹ'),
            'N' => Some('ᴺ'),
            'O' => Some('ᴼ'),
            'P' => Some('ᴾ'),
            'R' => Some('ᴿ'),
            'T' => Some('ᵀ'),
            'U' => Some('ᵁ'),
            'V' => Some('ⱽ'),
            'W' => Some('ᵂ'),
            ' ' => Some(' '),
            _ => None,
        })
        .collect::<Option<String>>()
}

fn to_subscript(s: &str) -> Option<String> {
    s.chars()
        .map(|c| match c {
            '0' => Some('₀'),
            '1' => Some('₁'),
            '2' => Some('₂'),
            '3' => Some('₃'),
            '4' => Some('₄'),
            '5' => Some('₅'),
            '6' => Some('₆'),
            '7' => Some('₇'),
            '8' => Some('₈'),
            '9' => Some('₉'),
            '+' => Some('₊'),
            '-' => Some('₋'),
            '=' => Some('₌'),
            '(' => Some('₍'),
            ')' => Some('₎'),
            'a' => Some('ₐ'),
            'e' => Some('ₑ'),
            'h' => Some('ₕ'),
            'i' => Some('ᵢ'),
            'j' => Some('ⱼ'),
            'k' => Some('ₖ'),
            'l' => Some('ₗ'),
            'm' => Some('ₘ'),
            'n' => Some('ₙ'),
            'o' => Some('ₒ'),
            'p' => Some('ₚ'),
            'r' => Some('ᵣ'),
            's' => Some('ₛ'),
            't' => Some('ₜ'),
            'u' => Some('ᵤ'),
            'v' => Some('ᵥ'),
            'x' => Some('ₓ'),
            ' ' => Some(' '),
            _ => None,
        })
        .collect::<Option<String>>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inline_render() {
        let cases = [
            (r"x^2", "x²"),
            (r"\frac{a}{b}", "(a)/(b)"),
            (r"\sqrt{x}", "√(x)"),
            (r"\alpha + \beta", "α + β"),
            (r"\sum_{i=1}^{n}", "Σᵢ₌₁ⁿ"),
            (r"\text{abc}", "abc"),
            (r"\mathbb{R}", "ℝ"),
            (r"x^{2k}", "x²ᵏ"),
            (r"\frac{x+1}{y-1}", "(x+1)/(y-1)"),
            (r"\sin^2\theta", "sin²θ"),
        ];
        for (input, expected) in cases {
            assert_eq!(render_inline_latex(input), expected);
        }
    }

    #[test]
    fn test_unknown_command_fallback() {
        assert_eq!(render_inline_latex(r"\foo{x}"), "x");
        assert_eq!(render_inline_latex(r"\foo"), "foo");
    }
}
