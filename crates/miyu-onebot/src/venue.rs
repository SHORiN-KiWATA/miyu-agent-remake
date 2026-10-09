//! `miyu onebot venue show <场所>`（`onebot.md` 第一条「对外的样子」、「场所规则和出厂数据」第 7 条，施工 O-21）：一个场所
//! 每一项的值和来处，像 `udevadm info`（18 第四节「看和改」）。不连核心，照系统的语言说；读的文件和桥一样（[`crate::rules`]）。
//!
//! 1. 场所编号照内核的 `VenueId` 再 [`Venue::parse`] 解：解不出的在标准错误上说 [`Shown::BadVenue`]，退出码 2（用法不对）。
//! 2. 读出厂的：有问题的照起来时那样说（[`Failure::Factory`]），退出码 1。再读系统的，不记运行日志。
//! 3. 在标准输出上：规则设到的每一项一行（照键排，值照 TOML 写，后面是来处），一项都没有的说 [`Shown::None`]；接着一句
//!    [`Shown::Defaults`]；读系统的发现了问题的，一句 [`Shown::Problems`]，问题缩进两格一条一行。退出码 0：问题是印出来的一
//!    部分（「施工时定的」第 55 条）。

use std::io::Write;

use miyu_chat::Venue;
use miyu_kernel::id::VenueId;
use miyu_store::root::DataRoot;

use crate::rules::{Factory, load};
use crate::serve::Failure;
use crate::texts::Texts;

/// 场所编号认不出：用法不对。
const USAGE: u8 = 2;

/// 出厂的数据有问题、印不出去。
const FAILED: u8 = 1;

/// `venue show` 说的几句（「给人看的字」`venue/`）：怎么说照 `texts`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shown {
    /// 没有规则设到这个场所。
    None,
    /// 没列出的参数照出厂的 `defaults.toml`。
    Defaults,
    /// 问题那一段的标题。
    Problems,
    /// 场所编号认不出：照原样。
    BadVenue(String),
}

/// 印场所 `venue`（编号的原文）每一项的值和来处：读资源目录（`texts` 的）里出厂的、数据根 `root` 里系统的，说的照 `texts`。
/// 交回退出码。
pub fn show(
    root: &DataRoot,
    venue: &str,
    texts: &Texts,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let Some(parsed) = VenueId::parse(venue).ok().and_then(|id| Venue::parse(&id)) else {
        return said(
            err,
            &texts.shown(&Shown::BadVenue(venue.to_string())),
            USAGE,
        );
    };
    let factory = match Factory::load(texts.resources()) {
        Ok(factory) => factory,
        Err(problems) => return said(err, &texts.failure(&Failure::Factory(problems)), FAILED),
    };
    let loaded = load(&factory, root);
    let mut lines: Vec<String> = loaded
        .at(&parsed)
        .resolved
        .entries
        .iter()
        .map(|(key, entry)| texts.entry(key, entry))
        .collect();
    if lines.is_empty() {
        lines.push(texts.shown(&Shown::None));
    }
    lines.push(texts.shown(&Shown::Defaults));
    if !loaded.problems.is_empty() {
        lines.push(texts.shown(&Shown::Problems));
        lines.extend(
            loaded
                .problems
                .iter()
                .map(|problem| format!("  {}", texts.problem(problem))),
        );
    }
    said(out, &lines.join("\n"), 0)
}

/// `text` 印到 `to`，末尾换行，交回 `code`；印不出去的（标准输出关了）交回 1。
fn said(to: &mut dyn Write, text: &str, code: u8) -> u8 {
    match writeln!(to, "{text}").and_then(|()| to.flush()) {
        Ok(()) => code,
        Err(_) => FAILED,
    }
}
