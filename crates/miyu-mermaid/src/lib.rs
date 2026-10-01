//! 可选软件包 `mermaid`（设计 04 第五节、13 第九节，`mermaid.md`，施工 W-4）：mermaid 源码画成 SVG，核心经
//! `mermaid.render` 把这件事做一次，终端和网页看的是同一张图。
//!
//! - 第一次调 [`Mermaid::render`] 才读 `style.json`、探一次系统的字体库；读不懂、找不到字体的，记成「没法画」，
//!   这个核心的生命周期里不再重试（`Mermaid::style`）。
//! - 源码照 SHA-256 缓存，最多记 `style.json` 里 `keep` 张，满了丢最早画的那一张（FIFO，不是真的「最久没用」：
//!   画完就不会再变，谁先进去谁先出去足够了）。
//! - 字、线、连线标签垫底先填 `style.json` 里的三种记号色（图里不会自己出现的颜色），回应里把它们报出来，
//!   调它的头自己换成想要的颜色——SVG 只画一次，换主题不用重画。
//! - 画图的库 (`mermaid-rs-renderer`) 崩了（panic）按「画不出」处理，不往上冒：它是别人家的代码，源码来自
//!   聊天记录、可能很怪，崩一次不该拖垮核心。

mod fonts;
mod style;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, PoisonError};

use mermaid_rs_renderer::{LayoutConfig, RenderOptions, Theme};
use sha2::{Digest, Sha256};

use style::Style;

/// 运行日志的目标。
const TARGET: &str = "miyu::mermaid";

/// 源码的 SHA-256：缓存的键。
type Hash = [u8; 32];

/// 画好的一张：SVG 的字，和里面用的三种记号色。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    /// SVG 的字：底和框不填色，字、线、连线标签垫底是 [`Marks`] 里的三种记号色。
    pub svg: String,
    /// SVG 里用的三种记号色，和 `marks` 字段一一对应。
    pub marks: Marks,
}

/// 字、线、连线标签垫底用的三种记号色：图里不会自己出现的颜色，调的头照它们换成自己的颜色。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marks {
    /// 字的颜色。
    pub text: String,
    /// 线、框的颜色。
    pub line: String,
    /// 连线标签垫底的颜色。
    pub label: String,
}

/// 画不出的原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderError {
    /// 源码去掉前后空白是空的。
    Empty,
    /// 源码超过 `style.json` 里的 `max_source`（字节数）。
    TooLong {
        /// 最多多少字节。
        max: usize,
    },
    /// 画不出：库的原话（解析错误的原话，或者它崩了时带的话）。
    Failed(String),
    /// 没法画：`style.json` 读不懂，或者这台机器上一种字体都读不到。原话记运行日志，
    /// 协议上这是 `internal_error`，不是这几个字本身。
    NotReady,
}

/// 画 mermaid 用的家底：资源目录在哪、懒读好的 `style.json`、画好的缓存。一个核心一份，一直留着到核心退出。
pub struct Mermaid {
    resources: PathBuf,
    ready: OnceLock<Result<Style, ()>>,
    cache: Mutex<Vec<(Hash, Rendered)>>,
}

impl Mermaid {
    /// 一份家底：只记资源目录在哪，不碰磁盘、不读字体——真正的初始化等第一次 [`Mermaid::render`]。
    pub fn new(resources: &Path) -> Mermaid {
        Mermaid {
            resources: resources.to_path_buf(),
            ready: OnceLock::new(),
            cache: Mutex::new(Vec::new()),
        }
    }

    /// 画一张：源码去掉前后空白，是空的、超过上限、库说读不懂（或者崩了）的交回原因；同一份源码（照
    /// SHA-256）第二次不重画，直接给缓存里那一份。
    ///
    /// # Errors
    ///
    /// 源码去掉前后空白是空的（[`RenderError::Empty`]）；超过 `style.json` 的 `max_source`
    /// （[`RenderError::TooLong`]）；画图的库读不懂这份源码，或者它崩了（[`RenderError::Failed`]）；`style.json`
    /// 读不懂，或者这台机器上一种字体都读不到（[`RenderError::NotReady`]，原话记运行日志）。
    ///
    /// # Panics
    ///
    /// 不会：画图的库自己崩了，这里接住当「画不出」，不会往上冒。
    pub fn render(&self, source: &str) -> Result<Rendered, RenderError> {
        let source = source.trim();
        if source.is_empty() {
            return Err(RenderError::Empty);
        }
        let style = self.style()?;
        if source.len() > style.max_source {
            return Err(RenderError::TooLong {
                max: style.max_source,
            });
        }
        let hash: Hash = Sha256::digest(source.as_bytes()).into();
        // 一次画一张：这把锁从查缓存到画完、存进缓存，全程拿着，查缓存和画图不会在两个请求之间插着走。
        let mut cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((_, rendered)) = cache.iter().find(|(h, _)| *h == hash) {
            return Ok(rendered.clone());
        }
        // 探字体要扫一遍系统的字体库，Windows 上能到几秒：空的、太长的、缓存里有的都不用等它，真要画才探。
        if !fonts::available() {
            return Err(RenderError::NotReady);
        }
        let rendered = draw(source, style)?;
        if cache.len() >= style.keep {
            cache.remove(0);
        }
        cache.push((hash, rendered.clone()));
        Ok(rendered)
    }

    /// 懒读好的 `style.json`：第一次调才读、才探字体，读不懂、探不到的记一条 `WARN`，这个核心的生命周期里
    /// 不再重试、也不再记第二条。
    fn style(&self) -> Result<&Style, RenderError> {
        self.ready
            .get_or_init(|| {
                let loaded = style::load(&self.resources).map_err(|error| error.to_string());
                match loaded {
                    Ok(style) => Ok(style),
                    Err(error) => {
                        tracing::warn!(target: TARGET, error = %error, "not ready");
                        Err(())
                    }
                }
            })
            .as_ref()
            .map_err(|()| RenderError::NotReady)
    }
}

/// 从渲染器的暗色主题改起：底和框不填色，字、线、连线标签垫底填记号色（照 tui-demo `figures/mermaid.rs`、
/// web-demo 桥 `mermaid.rs` 的样子，施工 W-4）。画图的库崩了（panic）当画不出，不往上冒。
fn draw(source: &str, style: &Style) -> Result<Rendered, RenderError> {
    let options = RenderOptions {
        theme: theme(style),
        layout: LayoutConfig::default(),
    };
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        mermaid_rs_renderer::render_with_options(source, options)
    }));
    let svg = match outcome {
        Ok(Ok(svg)) => svg,
        Ok(Err(error)) => return Err(RenderError::Failed(error.to_string())),
        Err(panic) => return Err(RenderError::Failed(panic_message(&panic))),
    };
    Ok(Rendered {
        svg,
        marks: style.marks.clone(),
    })
}

/// 渲染器的主题：暗色主题改起，底和框不填色，字、线、连线标签垫底是记号色，字体照 `style.json` 的一串。
fn theme(style: &Style) -> Theme {
    let mut theme = Theme::dark();
    theme.background = "none".to_string();
    theme.edge_label_background.clone_from(&style.marks.label);
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
        color.clone_from(&style.marks.text);
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
        color.clone_from(&style.marks.line);
    }
    let families: Vec<String> = style
        .fonts
        .iter()
        .map(|font| format!("\"{font}\""))
        .collect();
    theme.font_family = families.join(", ");
    theme
}

/// panic 带的话：是 `&str`、`String` 的照抄，别的（数字、自定义类型……）写一句泛泛的。
///
/// 参数特地写成 `&Box<dyn Any + Send>`、不写成 `&(dyn Any + Send)`：`Box<dyn Any>`自己也实现 `Any`，
/// 写成后一种时 `&payload` 会把方法调用点之外的那层 `Box` 整体当成被查的类型，`downcast_ref` 永远查不中
/// 里面真正装的 `&str`、`String`（踩过一次坑）。
fn panic_message(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "the drawing library panicked".to_string()
    }
}
