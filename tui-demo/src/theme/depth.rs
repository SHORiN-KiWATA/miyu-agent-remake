//! 色深（蓝图 `tui.md`「主题」第 5、6 条）：启动时照环境变量看一次终端显示得了几种颜色，画完一帧、交给终端
//! 之前统一换色。主题、流光、吉祥物、Markdown 画的时候都不管色深，只在这一处换。
//!
//! 探测照旧版 `terminal/palette.rs`；256 色找最近的照 tmux（`colour_find_rgb`）：色立方、灰阶各找一个，取更近的。

use ratatui::buffer::Buffer;
use ratatui::style::Color;

/// 终端显示得了几种颜色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Depth {
    /// 真彩色。
    True,
    /// 256 色。
    X256,
    /// 16 色。
    Ansi16,
    /// 不上色：只留加粗、斜体这些修饰。
    Mono,
}

/// `TERM` 里带着这些的，没设 `COLORTERM` 也是真彩色。
const TRUECOLOR_TERMS: [&str; 5] = ["kitty", "wezterm", "alacritty", "ghostty", "foot"];

/// `TERM_PROGRAM` 是这些的，真彩色。
const TRUECOLOR_PROGRAMS: [&str; 4] = ["iTerm.app", "WezTerm", "ghostty", "vscode"];

impl Depth {
    /// 照环境变量看；`var` 取一个变量，没有的给 `None`。
    pub fn detect(var: impl Fn(&str) -> Option<String>) -> Self {
        if var("NO_COLOR").is_some_and(|v| !v.is_empty()) {
            return Self::Mono;
        }
        if let Some(forced) = var("MIYU_COLOR") {
            return match forced.trim().to_ascii_lowercase().as_str() {
                "truecolor" | "24bit" => Self::True,
                "16" => Self::Ansi16,
                "none" => Self::Mono,
                _ => Self::X256,
            };
        }
        let colorterm = var("COLORTERM").unwrap_or_default().to_ascii_lowercase();
        if colorterm == "truecolor" || colorterm == "24bit" {
            return Self::True;
        }
        let term = var("TERM").unwrap_or_default().to_ascii_lowercase();
        if term.is_empty() || term == "dumb" {
            return Self::Mono;
        }
        let program = var("TERM_PROGRAM").unwrap_or_default();
        if TRUECOLOR_TERMS.iter().any(|t| term.contains(t))
            || TRUECOLOR_PROGRAMS.contains(&program.as_str())
        {
            return Self::True;
        }
        if term.contains("256color") {
            return Self::X256;
        }
        Self::Ansi16
    }
}

/// 一个颜色换成这一档显示得了的。
pub fn fit(color: Color, depth: Depth) -> Color {
    match (depth, color) {
        (Depth::True, _) | (_, Color::Reset) => color,
        (Depth::Mono, _) => Color::Reset,
        (Depth::X256, Color::Rgb(r, g, b)) => Color::Indexed(nearest_256(r, g, b)),
        (Depth::X256, _) => color,
        (Depth::Ansi16, Color::Rgb(r, g, b)) => named(nearest_16((r, g, b))),
        (Depth::Ansi16, Color::Indexed(n)) if n < 16 => named(n),
        (Depth::Ansi16, Color::Indexed(n)) => named(nearest_16(indexed_rgb(n))),
        (Depth::Ansi16, _) => color,
    }
}

/// 画完的一帧统一换色：每一格的前景、背景、下划线的颜色。真彩色的不用走一遍。
pub fn degrade(buf: &mut Buffer, depth: Depth) {
    if depth == Depth::True {
        return;
    }
    for cell in &mut buf.content {
        cell.fg = fit(cell.fg, depth);
        cell.bg = fit(cell.bg, depth);
        cell.underline_color = fit(cell.underline_color, depth);
    }
}

/// 色立方每一级的亮度。
const CUBE: [u8; 6] = [0x00, 0x5f, 0x87, 0xaf, 0xd7, 0xff];

/// 一个分量落在色立方的第几级（tmux 的 `colour_to_6cube`）。
fn cube_level(v: u8) -> usize {
    match v {
        0..48 => 0,
        48..115 => 1,
        _ => usize::from((v - 35) / 40),
    }
}

fn distance(a: (u8, u8, u8), b: (u8, u8, u8)) -> u32 {
    let d = |x: u8, y: u8| u32::from(x.abs_diff(y)).pow(2);
    d(a.0, b.0) + d(a.1, b.1) + d(a.2, b.2)
}

/// 256 色里最近的：色立方、灰阶各找一个，取更近的。
fn nearest_256(r: u8, g: u8, b: u8) -> u8 {
    let (qr, qg, qb) = (cube_level(r), cube_level(g), cube_level(b));
    let cube = (CUBE[qr], CUBE[qg], CUBE[qb]);
    // 这几个都小于 6，算出来的号在 16..=231。
    let cube_index = u8::try_from(16 + 36 * qr + 6 * qg + qb).unwrap_or(231);
    if cube == (r, g, b) {
        return cube_index;
    }
    let average = (u32::from(r) + u32::from(g) + u32::from(b)) / 3;
    let step = if average > 238 {
        23
    } else {
        average.saturating_sub(3) / 10
    };
    let grey_value = u8::try_from(8 + 10 * step).unwrap_or(238);
    let grey = (grey_value, grey_value, grey_value);
    if distance(grey, (r, g, b)) < distance(cube, (r, g, b)) {
        232 + u8::try_from(step).unwrap_or(23)
    } else {
        cube_index
    }
}

/// 16 色找最近的用的参照：VGA 那一套（Linux 控制台的默认，旧版也用它）。xterm 默认的红是纯红，粉红的出错色
/// 照它算会落到暗灰，丢了「红」。
const ANSI16: [(u8, u8, u8); 16] = [
    (0, 0, 0),
    (170, 0, 0),
    (0, 170, 0),
    (170, 85, 0),
    (0, 0, 170),
    (170, 0, 170),
    (0, 170, 170),
    (170, 170, 170),
    (85, 85, 85),
    (255, 85, 85),
    (85, 255, 85),
    (255, 255, 85),
    (85, 85, 255),
    (255, 85, 255),
    (85, 255, 255),
    (255, 255, 255),
];

/// 16 色里最近的是第几号。
fn nearest_16(rgb: (u8, u8, u8)) -> u8 {
    (0u8..16)
        .min_by_key(|&i| distance(ANSI16[usize::from(i)], rgb))
        .unwrap_or(7)
}

/// 256 色的第 `n` 号（16 号以后）是什么颜色。
fn indexed_rgb(n: u8) -> (u8, u8, u8) {
    match n {
        0..16 => ANSI16[usize::from(n)],
        16..232 => {
            let i = usize::from(n - 16);
            (CUBE[i / 36], CUBE[i / 6 % 6], CUBE[i % 6])
        }
        _ => {
            let v = 8 + 10 * (n - 232);
            (v, v, v)
        }
    }
}

/// 16 色写成名字：`SGR 31` 这类，认不得 256 色写法的终端也认。
fn named(n: u8) -> Color {
    const NAMES: [Color; 16] = [
        Color::Black,
        Color::Red,
        Color::Green,
        Color::Yellow,
        Color::Blue,
        Color::Magenta,
        Color::Cyan,
        Color::Gray,
        Color::DarkGray,
        Color::LightRed,
        Color::LightGreen,
        Color::LightYellow,
        Color::LightBlue,
        Color::LightMagenta,
        Color::LightCyan,
        Color::White,
    ];
    NAMES[usize::from(n % 16)]
}

#[cfg(test)]
mod tests;
