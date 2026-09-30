//! 模型常写的几样 HTML 标签照 Markdown 的样子画（蓝图 `tui.md`「她的回答：Markdown」第 12 条）：
//! 行内的叠一层样子、成一条链接、转上下标；外壳去掉标签；`<hr>` 是分隔线。没收尾的行内标签到这一块结尾为止。

use std::borrow::Cow;

use ratatui::style::{Modifier, Style};

use super::super::Renderer;
use crate::theme;

/// 开着的一个行内标签。
pub(in crate::markdown) enum Opened {
    /// 叠了一层样子：在 `styles` 里的第几层。
    Style(usize),
    /// 一条链接（`<a href>`）。
    Link,
    /// 上标（真）、下标：之后的字照公式的表转。
    Script(bool),
}

/// 一个认得的标签怎么画。
enum Kind {
    /// 叠一层样子。
    Style(Style),
    /// 链接，带着地址。
    Link(String),
    /// 上标（真）、下标。
    Script(bool),
    /// 外壳，自成一块：去掉标签。
    Block,
    /// 行内的外壳：去掉标签，只留字。
    Strip,
    /// 分隔线。
    Rule,
}

/// 认得的标签；别的交回 `None`，原样写出来。
fn kind(name: &str, attrs: &[(String, String)]) -> Option<Kind> {
    let style = |m: Modifier| Kind::Style(Style::new().add_modifier(m));
    Some(match name {
        "b" | "strong" => Kind::Style(theme::md_bold()),
        "i" | "em" => Kind::Style(theme::md_italic()),
        "u" | "ins" => style(Modifier::UNDERLINED),
        "s" | "del" | "strike" => style(Modifier::CROSSED_OUT),
        "code" | "kbd" | "samp" | "tt" => Kind::Style(theme::md_code()),
        "mark" => Kind::Style(theme::md_mark()),
        "sub" => Kind::Script(false),
        "sup" => Kind::Script(true),
        "a" => match attrs.iter().find(|(k, _)| k == "href") {
            Some((_, href)) => Kind::Link(href.clone()),
            None => Kind::Strip,
        },
        "hr" => Kind::Rule,
        "p" | "div" | "center" | "section" | "article" | "header" | "footer" | "main" | "aside"
        | "nav" | "figure" | "figcaption" => Kind::Block,
        "span" | "font" | "small" | "big" => Kind::Strip,
        _ => return None,
    })
}

impl Renderer<'_> {
    /// 认得的标签照 Markdown 的样子画；交回假表示不认得，由外面原样写出来。
    pub(in crate::markdown) fn common_tag(
        &mut self,
        name: &str,
        attrs: &[(String, String)],
        closing: bool,
    ) -> bool {
        let Some(kind) = kind(name, attrs) else {
            return false;
        };
        match (kind, closing) {
            (Kind::Rule, false) => {
                self.ensure_title();
                self.rule();
            }
            (Kind::Block, _) => {
                self.ensure_title();
                self.flush();
                // 居不居中照这一层记：`<center>`，或者写了 `align="center"` 的（2026-09-30 项目主人要做）。
                if closing {
                    self.centered.pop();
                } else {
                    let align = attrs
                        .iter()
                        .any(|(k, v)| k == "align" && v.eq_ignore_ascii_case("center"));
                    self.centered.push(name == "center" || align);
                    self.gap();
                }
            }
            (Kind::Rule | Kind::Strip, _) => {}
            (_, true) => self.close_tag(name),
            (Kind::Style(style), false) => {
                self.ensure_title();
                self.styles.push(style);
                let at = self.styles.len() - 1;
                self.html_open.push((name.to_string(), Opened::Style(at)));
            }
            (Kind::Link(href), false) => {
                self.ensure_title();
                self.styles.push(theme::md_link());
                self.link_starts.push((href, self.pieces.len()));
                self.html_open.push((name.to_string(), Opened::Link));
            }
            (Kind::Script(up), false) => {
                self.html_open.push((name.to_string(), Opened::Script(up)));
            }
        }
        true
    }

    /// 一块结束：没收尾的行内标签到这里为止。
    pub(in crate::markdown) fn close_open_tags(&mut self) {
        while let Some((_, opened)) = self.html_open.pop() {
            self.undo(opened);
        }
    }

    /// 在 `<sub>`、`<sup>` 里的字转成下标、上标（和公式同一张表）；转不动的字照原样。
    pub(in crate::markdown) fn scripted<'t>(&self, text: &'t str) -> Cow<'t, str> {
        let up = self.html_open.iter().rev().find_map(|(_, o)| match o {
            Opened::Script(up) => Some(*up),
            _ => None,
        });
        let Some(up) = up else {
            return Cow::Borrowed(text);
        };
        let table = if up {
            &self.math.superscripts
        } else {
            &self.math.subscripts
        };
        Cow::Owned(text.chars().map(|c| *table.get(&c).unwrap_or(&c)).collect())
    }

    /// 收尾标签：收掉最近一个开着的同名标签；没开过的不管。
    fn close_tag(&mut self, name: &str) {
        if let Some(at) = self.html_open.iter().rposition(|(n, _)| n == name) {
            let (_, opened) = self.html_open.remove(at);
            self.undo(opened);
        }
    }

    fn undo(&mut self, opened: Opened) {
        match opened {
            Opened::Style(at) => {
                if at < self.styles.len() {
                    self.styles.remove(at);
                }
                for (_, other) in &mut self.html_open {
                    if let Opened::Style(later) = other
                        && *later > at
                    {
                        *later -= 1;
                    }
                }
            }
            Opened::Link => self.end_link(false),
            Opened::Script(_) => {}
        }
    }
}
