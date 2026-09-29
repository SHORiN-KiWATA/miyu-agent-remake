//! 颜色和修饰。颜色都从当前主题取（蓝图 `tui.md`「主题」）；加粗、斜体、变暗这些修饰写在这里。
//!
//! 当前主题放在一个全局的读写锁里：各处画的时候直接取，不用把主题一层层传下去；启动时照配置设一次，
//! `/theme` 换的也是它。

mod palette;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, RwLock};

use ratatui::style::{Color, Modifier, Style};

pub use palette::{Palette, builtin};

use crate::config::Shimmer;
use crate::core::Level;

#[cfg(test)]
mod test_support;
#[cfg(test)]
pub use test_support::hold;

/// 当前主题。启动前是出厂的第一套。
static CURRENT: LazyLock<RwLock<Palette>> = LazyLock::new(|| {
    let first = palette::BUILTIN[0].1;
    RwLock::new(serde_json::from_str(first).unwrap_or_else(|e| panic!("出厂主题读不懂：{e}")))
});

/// 换过几次主题。排好存着的东西（回答的 Markdown）把它算进键里，换了主题就重排。
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// 换主题。
pub fn set(palette: Palette) {
    match CURRENT.write() {
        Ok(mut current) => *current = palette,
        Err(poisoned) => *poisoned.into_inner() = palette,
    }
    GENERATION.fetch_add(1, Ordering::Relaxed);
}

/// 换过几次主题，见 [`GENERATION`]。
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Relaxed)
}

/// 取当前主题里的一个颜色。
fn pick(get: impl Fn(&Palette) -> palette::Tone) -> Color {
    match CURRENT.read() {
        Ok(current) => get(&current).0,
        Err(poisoned) => get(&poisoned.into_inner()).0,
    }
}

fn fg(get: impl Fn(&Palette) -> palette::Tone) -> Style {
    Style::new().fg(pick(get))
}

/// 侧边栏各段的标题：蓝色、不加粗（`tui.md`「后台命令、子代理和侧边栏」第 7 条：加粗太抢眼）。
pub fn side_title() -> Style {
    fg(|p| p.side_title)
}

/// 悬停：比暗色亮一档，不是原色（`tui.md`「时间线」第 6 条）。
pub fn hover() -> Style {
    fg(|p| p.hover)
}

/// 首页吉祥物的一块（`tui.md`「空会话的首页」第 5 条）。
pub fn mascot(part: crate::mascot::Part) -> Style {
    use crate::mascot::Part;
    fg(move |p| match part {
        Part::Head => p.mascot_head,
        Part::Ear => p.mascot_ear,
        Part::Eye => p.mascot_eye,
        Part::Fin => p.mascot_fin,
    })
}

/// 边框、提示这类退到后面的东西。
pub fn dim() -> Style {
    fg(|p| p.dim)
}

/// 选中的字：反色，任何主题下都看得出来。
pub fn selected() -> Style {
    Style::new().add_modifier(Modifier::REVERSED)
}

/// 正文里你说的话前面那根竖线：跟着权限级别的颜色，不加粗。
pub fn user_bar(level: Level) -> Style {
    Style::new().fg(level_color(level))
}

/// 提示小框的边框：好消息（复制成了）绿，别的暗（`tui.md`「提示」）。
pub fn notice_border(good: bool) -> Style {
    if good { fg(|p| p.good) } else { dim() }
}

/// 出错、连不上。
pub fn error() -> Style {
    fg(|p| p.error)
}

/// 重试这类要留意、但还没坏的。
pub fn warn() -> Style {
    fg(|p| p.warn)
}

/// 权限级别的颜色，加粗：框外左下角的级别和输入框的提示符都用它。只读暖金，开放权限红，工作区蓝（照旧版）。
pub fn level(level: Level) -> Style {
    Style::new()
        .fg(level_color(level))
        .add_modifier(Modifier::BOLD)
}

/// 权限级别对应的颜色：提示符、左下角、你说的话的竖线三处共用。
fn level_color(level: Level) -> Color {
    match level {
        Level::ReadOnly => pick(|p| p.read_only),
        Level::Full => pick(|p| p.full),
        Level::Workspace => pick(|p| p.workspace),
    }
}

/// 斜杠命令列表里选中的那一条：品红加粗。
pub fn picked() -> Style {
    fg(|p| p.picked).add_modifier(Modifier::BOLD)
}

/// 底栏的模型名：加粗，比端点亮。
pub fn model() -> Style {
    Style::new().add_modifier(Modifier::BOLD)
}

/// 思考的字：正体，主题的 `thought`，不叠「变暗」（叠了在深色底上发闷，2026-09-29 项目主人嫌不好看）。
pub fn thought() -> Style {
    fg(|p| p.thought)
}

/// 悬停时思考的字：同色系亮一档（蓝图「时间线」第 5 条）。
pub fn thought_hover() -> Style {
    fg(|p| p.thought_hover)
}

/// 点开的步骤的底色。256 色里的深灰，亮色主题下也还分得出来。
pub fn shade() -> Style {
    Style::new().bg(pick(|p| p.shade))
}

/// 粘贴块这种小块：品红的字铺一层暗紫底（蓝图「输入框」第 11 条）。
pub fn chip() -> Style {
    picked().bg(pick(|p| p.chip_bg))
}

/// 悬停时的小块：底色亮一档。
pub fn chip_hover() -> Style {
    picked().bg(pick(|p| p.chip_hover_bg))
}

/// 展开着的粘贴、悬停时：字不变，铺上小块的底色，看得出哪些是粘的。
pub fn chip_ground() -> Style {
    Style::new().bg(pick(|p| p.chip_bg))
}

/// 加了几行：绿。
pub fn added() -> Style {
    fg(|p| p.added)
}

/// 删了几行：红。
pub fn removed() -> Style {
    fg(|p| p.removed)
}

/// 差异里删掉的行：256 色 210 号字、底色 `rgb(60,41,53)`（照旧版的 `PATCH_DELETE_STYLE`）。
pub fn diff_removed() -> Style {
    fg(|p| p.diff_removed_fg).bg(pick(|p| p.diff_removed_bg))
}

/// 差异里加上的行：256 色 157 号字、底色 `rgb(32,52,67)`（照旧版的 `PATCH_INSERT_STYLE`）。
pub fn diff_added() -> Style {
    fg(|p| p.diff_added_fg).bg(pick(|p| p.diff_added_bg))
}

/// Markdown 的标题：品红加粗（照旧版 `HEADER_STYLE`）。
pub fn md_heading() -> Style {
    fg(|p| p.md_heading).add_modifier(Modifier::BOLD)
}

/// Markdown 的粗体：蓝加粗（照旧版 `BOLD_STYLE`）。
pub fn md_bold() -> Style {
    fg(|p| p.md_bold).add_modifier(Modifier::BOLD)
}

/// Markdown 的斜体：256 色 250 号、斜体（照旧版 `ITALIC_STYLE`）。
pub fn md_italic() -> Style {
    fg(|p| p.md_italic).add_modifier(Modifier::ITALIC)
}

/// 行内代码：青（照旧版 `INLINE_CODE_STYLE`）。
pub fn md_code() -> Style {
    fg(|p| p.md_code)
}

/// 链接的标题：256 色 117 号、加粗（照旧版 `LINK_LABEL_STYLE`）。
pub fn md_link() -> Style {
    fg(|p| p.md_link).add_modifier(Modifier::BOLD)
}

/// 链接的地址：256 色 75 号（照旧版 `URL_STYLE`）。
pub fn md_url() -> Style {
    fg(|p| p.md_url)
}

/// 图片：256 色 183 号（照旧版 `IMAGE_STYLE`）。
pub fn md_image() -> Style {
    fg(|p| p.md_image)
}

/// 引用的字：暗灰蓝、斜体（2026-09-29 项目主人：绿太显眼；改过正体，又嫌不够暗、要斜体）。
pub fn md_quote() -> Style {
    fg(|p| p.md_quote).add_modifier(Modifier::ITALIC)
}

/// 回答里 `<mark>` 高亮的字（蓝图「她的回答：Markdown」第 12 条）。
pub fn md_mark() -> Style {
    fg(|p| p.md_mark)
}

/// 表头：蓝加粗。
pub fn md_table_head() -> Style {
    fg(|p| p.md_table_head).add_modifier(Modifier::BOLD)
}

/// 列表的记号（`•`、数字、`☐`）：强调色，和字分开层次（2026-09-29 项目主人）。
pub fn md_list() -> Style {
    accent()
}

/// 好消息：后台任务完成的 `✓`。
pub fn good() -> Style {
    fg(|p| p.good)
}

/// 强调色：后台面板的横线和标题、在做的待办。
pub fn accent() -> Style {
    fg(|p| p.accent)
}

/// 代码的关键字：`rgb(196,167,231)`（照旧版 `CODE_KEYWORD_STYLE`）。
pub fn code_keyword() -> Style {
    fg(|p| p.code_keyword)
}

/// 代码的函数名：`rgb(156,207,216)`。
pub fn code_function() -> Style {
    fg(|p| p.code_function)
}

/// 代码的字符串：`rgb(166,214,160)`。
pub fn code_string() -> Style {
    fg(|p| p.code_string)
}

/// 代码的数字：`rgb(246,193,119)`。
pub fn code_number() -> Style {
    fg(|p| p.code_number)
}

/// 代码的注释：绿。
pub fn code_comment() -> Style {
    fg(|p| p.code_comment)
}

/// 代码里的类型：内建类型、大写开头的名字。
pub fn code_type() -> Style {
    fg(|p| p.code_type)
}

/// 代码里的常量：`true`、`None`、全大写的名字。
pub fn code_constant() -> Style {
    fg(|p| p.code_constant)
}

/// 代码里的键、属性名：JSON、YAML、TOML、CSS 冒号前、等号前的，HTML 的属性名。
pub fn code_property() -> Style {
    fg(|p| p.code_property)
}

/// 代码里的运算符。
pub fn code_operator() -> Style {
    fg(|p| p.code_operator)
}

/// 代码里的宏、属性、装饰器、shell 变量。
pub fn code_macro() -> Style {
    fg(|p| p.code_macro)
}

/// shell 命令的参数。
pub fn code_parameter() -> Style {
    fg(|p| p.code_parameter)
}

/// 运行状态行第 `index` 个字（一共 `len` 个）在 `t` 秒时的颜色：底色不变；一道亮光从左往右扫过，
/// 没扫到的乘 `dim` 压暗，扫到的往白里偏，正中偏 `lift`。
pub fn shimmer(index: usize, len: usize, t: f64, look: &Shimmer) -> Style {
    let (r, g, b) = accent_rgb();
    // 亮光走一趟要多走两个带宽，头尾都扫得干净。
    let span = len as f64 + 2.0 * look.band;
    let head = (t / look.sweep_seconds.max(0.1)).fract() * span - look.band;
    let near = 1.0 - ((index as f64 - head).abs() / look.band.max(0.1)).min(1.0);
    let scale = look.dim + (1.0 - look.dim) * near;
    let lift = look.lift * near;
    let mix = |c: u8| {
        let v = f64::from(c) * scale;
        (v + (255.0 - v) * lift).round().clamp(0.0, 255.0) as u8
    };
    Style::new()
        .fg(Color::Rgb(mix(r), mix(g), mix(b)))
        .add_modifier(Modifier::BOLD)
}

/// 运行状态行的点和用时：流光的底色压暗（和没扫到的字一样），不跟着流光变。
pub fn shimmer_rest(look: &Shimmer) -> Style {
    let (r, g, b) = accent_rgb();
    let dim = |c: u8| (f64::from(c) * look.dim).round().clamp(0.0, 255.0) as u8;
    Style::new().fg(Color::Rgb(dim(r), dim(g), dim(b)))
}

/// 主题的 `accent` 的三个字节：流光要按字节算明暗；不是真彩色的，退回 tokyonight 的前景色。
fn accent_rgb() -> (u8, u8, u8) {
    rgb_or(pick(|p| p.accent), (192, 202, 245))
}

/// 行内公式。
pub fn md_math() -> Style {
    fg(|p| p.md_math)
}

/// 排成图的公式的字色。图要真颜色，主题里写的是终端调色板的，用一个在深色、浅色底上都看得清的雾蓝。
pub fn math_rgb() -> (u8, u8, u8) {
    rgb_or(pick(|p| p.md_math), (192, 202, 245))
}

/// mermaid 图的颜色：字、框线和连线、点开的大图的底。主题里写的不是真颜色的，用 tokyonight 的。
pub fn diagram_rgb() -> DiagramColors {
    DiagramColors {
        text: rgb_or(pick(|p| p.diagram_text), (169, 177, 214)),
        line: rgb_or(pick(|p| p.diagram_line), (115, 122, 162)),
        backdrop: rgb_or(pick(|p| p.diagram_backdrop), (26, 27, 38)),
    }
}

/// mermaid 图的颜色，真颜色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiagramColors {
    /// 字。
    pub text: (u8, u8, u8),
    /// 框线和连线。
    pub line: (u8, u8, u8),
    /// 点开的大图的底。
    pub backdrop: (u8, u8, u8),
}

fn rgb_or(color: Color, fallback: (u8, u8, u8)) -> (u8, u8, u8) {
    match color {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => fallback,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn thoughts_are_upright_not_dimmed() {
        use ratatui::style::Modifier;
        let style = super::thought();
        assert!(
            !style.add_modifier.contains(Modifier::ITALIC),
            "思考的字不斜"
        );
        assert!(
            !style.add_modifier.contains(Modifier::DIM),
            "叠了变暗在深色底上发闷"
        );
    }

    use super::{accent_rgb, shimmer};
    use crate::config::Config;
    use ratatui::style::Color;

    fn rgb(style: ratatui::style::Style) -> (u8, u8, u8) {
        match style.fg {
            Some(Color::Rgb(r, g, b)) => (r, g, b),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn far_from_the_band_is_the_base_color_dimmed() {
        let look = Config::builtin().unwrap().layout.shimmer;
        let (r, g, b) = accent_rgb();
        // 亮光刚出发、还在最左边外面时，第 3 个字离它最远。
        let far = rgb(shimmer(3, 4, 0.0, &look));
        let dim = |c: u8| (f64::from(c) * look.dim).round() as u8;
        assert_eq!(far, (dim(r), dim(g), dim(b)));
    }

    #[test]
    fn the_band_lights_what_it_passes() {
        let look = Config::builtin().unwrap().layout.shimmer;
        let sum = |c: (u8, u8, u8)| u32::from(c.0) + u32::from(c.1) + u32::from(c.2);
        let bright = |i: usize, t: f64| sum(rgb(shimmer(i, 4, t, &look)));
        let span = 4.0 + 2.0 * look.band;
        let at = |i: f64| (i + look.band) / span * look.sweep_seconds;
        assert!(bright(0, at(0.0)) > bright(3, at(0.0)));
        assert!(bright(3, at(3.0)) > bright(0, at(3.0)));
        let base = sum(accent_rgb());
        assert!(bright(1, at(1.0)) > base, "正中的比底色还亮");
    }
}
