//! `web.mermaid`：mermaid 源码画成 SVG（蓝图 `web.md`「mermaid 图」）。顶替核心网页模块里的画图（旧版网页是后端画的，
//! `POST /api/mermaid`）；和终端界面同一个 crate、同一个版本（`mermaid-rs-renderer` 0.3.1），两个头画出来才一样。
//!
//! 样子照终端界面（`tui-demo/src/figures/mermaid.rs`）：从渲染器的暗色主题改起，底和框都不填色，融进她的正文。
//! 字、线、连线标签的垫底先填几个用不到的记号色，画完换成页面的 CSS 变量（`resources/mermaid.json`）：SVG 内联进页面，
//! 变量穿得进去，换主题不用重画（旧版网页的做法）。直接把 `var(…)` 填进主题不行：渲染器要照颜色算深浅。

use std::path::Path;
use std::sync::Mutex;

use mermaid_rs_renderer::{LayoutConfig, RenderOptions, Theme};
use serde_json::Value;

/// 字、线、连线标签垫底的记号色：画完照 `mermaid.json` 换掉。挑的是图里不会自己出现的颜色。
const TEXT: &str = "#010203";
const LINE: &str = "#040506";
const LABEL: &str = "#070809";

/// 画图用的字体、颜色、上限（`resources/mermaid.json`），和记着的几张。
pub struct Mermaid {
    font_family: String,
    text: String,
    line: String,
    label_background: String,
    max_source: usize,
    keep: usize,
    /// 画好的：源码 → SVG，先进先出。
    cache: Mutex<Vec<(String, String)>>,
}

impl Mermaid {
    /// 读页面目录下的 `resources/mermaid.json`。
    ///
    /// # Errors
    ///
    /// 读不到、不是 JSON、少了哪一格：说是哪一样。
    pub fn load(dir: &Path) -> Result<Mermaid, String> {
        let file = dir.join("resources/mermaid.json");
        let text = std::fs::read_to_string(&file).map_err(|e| format!("读不了 {}：{e}", file.display()))?;
        let v: Value = serde_json::from_str(&text).map_err(|e| format!("{} 不是 JSON：{e}", file.display()))?;
        let s = |k: &str| v[k].as_str().map(str::to_string).ok_or(format!("{} 少了 {k}", file.display()));
        let n = |k: &str| v[k].as_u64().and_then(|n| usize::try_from(n).ok()).ok_or(format!("{} 少了 {k}", file.display()));
        Ok(Mermaid {
            font_family: s("font_family")?,
            text: s("text")?,
            line: s("line")?,
            label_background: s("label_background")?,
            max_source: n("max_source")?,
            keep: n("keep")?,
            cache: Mutex::new(Vec::new()),
        })
    }

    /// 画一张：源码是空的、太长、读不懂的交回原因；同一份源码画过的直接给。
    ///
    /// # Errors
    ///
    /// 源码是空的、超过上限，或者渲染器说读不懂（照它的原话）。
    pub fn render(&self, source: &str) -> Result<String, String> {
        let source = source.trim();
        if source.is_empty() {
            return Err("mermaid 源码是空的".to_string());
        }
        if source.len() > self.max_source {
            return Err(format!("mermaid 源码超过 {} 字节", self.max_source));
        }
        if let Ok(cache) = self.cache.lock()
            && let Some((_, svg)) = cache.iter().find(|(s, _)| s == source)
        {
            return Ok(svg.clone());
        }
        let options = RenderOptions { theme: self.theme(), layout: LayoutConfig::default() };
        let svg = mermaid_rs_renderer::render_with_options(source, options).map_err(|e| e.to_string())?;
        let svg = svg
            .replace(TEXT, &self.text)
            .replace(LINE, &self.line)
            .replace(LABEL, &self.label_background);
        if let Ok(mut cache) = self.cache.lock() {
            if cache.len() >= self.keep {
                cache.remove(0);
            }
            cache.push((source.to_string(), svg.clone()));
        }
        Ok(svg)
    }

    /// 渲染器的主题：暗色主题改起，底和框不填色，字和线用记号色（照 TUI 演示 `figures/mermaid.rs` 的 `detail`）。
    fn theme(&self) -> Theme {
        let mut theme = Theme::dark();
        theme.background = "none".to_string();
        // 连线上的标签垫一块底把线挡住，不然线从字中间穿过去
        theme.edge_label_background = LABEL.to_string();
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
            *color = TEXT.to_string();
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
            *color = LINE.to_string();
        }
        theme.font_family.clone_from(&self.font_family);
        theme
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mermaid() -> Mermaid {
        #[allow(clippy::unwrap_used)]
        Mermaid::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("..")).unwrap()
    }

    #[test]
    fn colours_become_page_variables_and_nothing_is_filled() {
        let m = mermaid();
        let svg = m.render("flowchart LR\n  A[开始] -->|走| B[结束]").unwrap_or_default();
        assert!(svg.starts_with("<svg"), "{svg}");
        for mark in [TEXT, LINE, LABEL] {
            assert!(!svg.contains(mark), "记号色 {mark} 没换掉");
        }
        assert!(svg.contains(&m.text) && svg.contains(&m.line));
        // 同一份源码第二次直接给记着的
        assert_eq!(m.render("flowchart LR\n  A[开始] -->|走| B[结束]").unwrap_or_default(), svg);
    }

    #[test]
    fn empty_too_long_and_broken_sources_say_why() {
        let m = mermaid();
        assert!(m.render("  \n").is_err());
        assert!(m.render(&"x".repeat(m.max_source + 1)).is_err());
        assert!(m.render("flowchart LR\n  A --> ").is_err() || m.render("not a diagram at all").is_err());
    }
}
