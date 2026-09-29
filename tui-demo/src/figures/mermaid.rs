//! mermaid 图（蓝图 `tui.md`「图片、公式和 mermaid 图」第 4 条）：出 SVG，再照算好的格子一次渲到位；
//! 另存一份带底的 SVG，给图下面那一行「点开看大图」。
//!
//! 出 SVG 这一半本该是核心的 `view.detail`（设计 13 第九节，排在 M8）；核心有了以后，
//! 只把 [`detail`] 换成协议调用，栅格化照旧。

use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};

use image::RgbaImage;
use mermaid_rs_renderer::{LayoutConfig, RenderOptions, Theme};
use resvg::usvg;

use super::cells::{Cell, Fit};
use super::svg;
use crate::theme::DiagramColors;

/// 出图的样子：颜色、字体。核心出 SVG 时照主题定，现在由头给。
pub struct Look<'a> {
    /// 字、线、大图的底。
    pub colors: DiagramColors,
    /// 中文字体，照先后找。
    pub fonts: &'a [String],
}

/// 顶替核心的 `view.detail`：给 mermaid 源码，交回 SVG。
///
/// 正文里的图底透明、框不填色，字和线用主题的颜色；`backdrop` 为真时铺上大图的底（点开看的那份）。
///
/// # Errors
///
/// 源码读不懂时返回 mermaid-rs-renderer 说的原因。
pub fn detail(source: &str, look: &Look, backdrop: bool) -> Result<String, String> {
    // 从渲染器的暗色主题改起：它的线和字本来就是给深色底配的。
    let mut theme = Theme::dark();
    let colors = look.colors;
    let text = hex(colors.text);
    let line = hex(colors.line);
    theme.background = if backdrop {
        hex(colors.backdrop)
    } else {
        "none".to_string()
    };
    // 连线上的标签垫一块暗底把线挡住，不然线从字中间穿过去（节点照旧透明、不填色）。
    theme.edge_label_background = hex(colors.backdrop);
    for fill in [
        &mut theme.primary_color,
        &mut theme.secondary_color,
        &mut theme.tertiary_color,
        &mut theme.cluster_background,
        &mut theme.sequence_actor_fill,
        &mut theme.sequence_note_fill,
        &mut theme.sequence_activation_fill,
    ] {
        *fill = "none".to_string();
    }
    for color in [
        &mut theme.primary_text_color,
        &mut theme.text_color,
        &mut theme.pie_title_text_color,
        &mut theme.pie_section_text_color,
        &mut theme.pie_legend_text_color,
    ] {
        color.clone_from(&text);
    }
    for color in [
        &mut theme.primary_border_color,
        &mut theme.line_color,
        &mut theme.cluster_border,
        &mut theme.sequence_actor_border,
        &mut theme.sequence_actor_line,
        &mut theme.sequence_note_border,
        &mut theme.sequence_activation_border,
    ] {
        color.clone_from(&line);
    }
    // 中文字体打头：渲染器主题的字体表里没有中文字体，不然由 resvg 随手找一款（旧版踩过：挑中等宽字体，一字一格）。
    let mut families: Vec<String> = look.fonts.iter().map(|f| format!("\"{f}\"")).collect();
    families.push(theme.font_family.clone());
    theme.font_family = families.join(", ");
    let options = RenderOptions {
        theme,
        layout: LayoutConfig::default(),
    };
    mermaid_rs_renderer::render_with_options(source, options).map_err(|e| e.to_string())
}

/// 出 SVG、算格子、一次渲到那个尺寸；底透明。
///
/// # Errors
///
/// 源码读不懂、SVG 读不懂时返回原因。
pub fn draw(
    source: &str,
    look: &Look,
    cell: Cell,
    max_cols: u16,
    max_rows: u16,
) -> Result<(RgbaImage, Fit), String> {
    let svg = detail(source, look, false)?;
    let options = usvg::Options {
        font_family: look.fonts.first().cloned().unwrap_or_default(),
        fontdb: svg::font_db(),
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_str(&svg, &options).map_err(|e| e.to_string())?;
    svg::raster(&tree, cell, max_cols, max_rows)
}

/// 点开看的大图：带底的 SVG 写进 `dir`（同一张图同样的颜色只写一次），交回文件的路径；
/// 目录里多过 `keep` 张时删最早写的。
///
/// # Errors
///
/// 源码读不懂、写不进目录时返回原因。
pub fn zoom(source: &str, look: &Look, dir: &Path, keep: usize) -> Result<PathBuf, String> {
    let mut hasher = DefaultHasher::new();
    (
        source,
        look.colors.text,
        look.colors.line,
        look.colors.backdrop,
    )
        .hash(&mut hasher);
    let file = dir.join(format!("{:016x}.svg", hasher.finish()));
    if !file.is_file() {
        let svg = detail(source, look, true)?;
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        fs::write(&file, svg).map_err(|e| e.to_string())?;
        prune(dir, keep);
    }
    Ok(file)
}

/// 只留最近写的 `keep` 张 SVG。删不掉的不管：下次再删。
fn prune(dir: &Path, keep: usize) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "svg"))
        .filter_map(|p| Some((fs::metadata(&p).ok()?.modified().ok()?, p)))
        .collect();
    files.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
    for (_, old) in files.into_iter().skip(keep.max(1)) {
        if fs::remove_file(&old).is_err() {
            // 删不掉（权限、正被占着）：留着，下次写图时再试。
        }
    }
}

fn hex((r, g, b): (u8, u8, u8)) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

#[cfg(test)]
mod tests;
