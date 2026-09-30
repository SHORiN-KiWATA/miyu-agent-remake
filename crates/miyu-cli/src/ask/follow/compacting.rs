//! 压缩那几行（施工 6-3 下，照项目主人 2026-09-29 定的样子）：灰色旁白，终端里原地刷新进度，压好了换成结果，失败的
//! 红。标准错误不是终端的，压缩中不印，只印结果那一行。`--format json` 不印。
//!
//! - 压缩中：`· 正在压缩上下文… 已写 3,120 字`；
//! - 压好了：`· 上下文已压缩：812.3k → 31k token`；
//! - 失败：`· 压缩失败：<原因>`；
//! - 暂停了自动压缩（施工 6-6 上）：红，照原因一行。

use serde_json::Value;

use super::{Follow, Format, REDRAW, Screen};
use crate::ask::usage::thousands;
use crate::language::Language;
use crate::shown::{Ink, Line, write};

/// 内核在摘要请求调了工具、接着改走隔离式时，原话末尾写的（`docs/blueprint/compaction.md` 第三条第 7 条）。
const ISOLATING: &str = "trying again without tools";

/// 正在压缩的那一次。
#[derive(Debug, Default)]
pub(super) struct Compacting {
    /// 摘要请求的名字（它替代到的那一条）；没在压的没有。
    seen: Option<u64>,
    /// 终端里那一行进度画着，还没换成结果。
    drawn: bool,
}

impl Follow<'_> {
    /// 进度：记下是哪一次；终端里原地刷新那一行。
    pub(super) fn compaction_progress(&mut self, body: &Value, screen: &mut Screen<'_>) {
        self.compacting.seen = body["seen"].as_u64();
        if self.plan.format != Format::Text || !screen.live {
            return;
        }
        let written = body["written"].as_u64().unwrap_or(0);
        let line = Line::gray(progress(&self.plan.language, written));
        if !self.compacting.drawn {
            self.close_answer(screen);
            self.thought();
            self.settle(screen);
            if !self.err_ends_line {
                write(screen.err, "\n");
            }
        }
        let painted = line.paint(screen.gray);
        write(
            screen.err,
            &format!("{REDRAW}{}", painted.trim_end_matches('\n')),
        );
        self.compacting.drawn = true;
        self.err_ends_line = false;
        self.aside = true;
        self.blank = false;
    }

    /// 压好了：换成结果那一行。
    pub(super) fn compaction_done(&mut self, body: &Value, screen: &mut Screen<'_>) {
        self.compacting.seen = None;
        let (before, after) = (
            body["before"].as_u64().unwrap_or(0),
            body["after"].as_u64().unwrap_or(0),
        );
        let line = Line::gray(done(&self.plan.language, before, after));
        self.finish(&line, screen);
    }

    /// 摘要请求的记录：出错的说压缩失败，照分类说原因。被打断的不说：这一轮的收尾会说。
    pub(super) fn compaction_called(&mut self, body: &Value, screen: &mut Screen<'_>) {
        if self.compacting.seen.is_none() || body["seen"].as_u64() != self.compacting.seen {
            return;
        }
        if body["result"].as_str() != Some("error") {
            return;
        }
        let class = body["error"]["class"].as_str().unwrap_or("other");
        let message = body["error"]["message"].as_str().unwrap_or_default();
        // 调了工具、接着改走隔离式的（施工 6-6 下）：不是失败，灰色说一句，这次压缩接着来进度。
        if class == "bad_summary" && message.ends_with(ISOLATING) {
            let line = Line::gray(isolating(&self.plan.language));
            self.finish(&line, screen);
            return;
        }
        self.compacting.seen = None;
        let reason = reason(&self.plan.language, class, message);
        let line = Line::inked(Ink::Red, failed(&self.plan.language, &reason));
        self.finish(&line, screen);
    }

    /// 暂停了自动压缩（施工 6-6 上）：红，照原因印一行。连续失败的，前面刚印过「压缩失败」那一行。
    pub(super) fn compaction_paused(&mut self, body: &Value, screen: &mut Screen<'_>) {
        let line = Line::inked(Ink::Red, paused(&self.plan.language, body));
        self.finish(&line, screen);
    }

    /// 结果那一行：终端里画着进度的，擦掉换成它；没画的，照旁白印。
    fn finish(&mut self, line: &Line, screen: &mut Screen<'_>) {
        if self.plan.format != Format::Text {
            self.compacting.drawn = false;
            return;
        }
        if std::mem::take(&mut self.compacting.drawn) {
            write(screen.err, REDRAW);
            write(screen.err, &line.paint(screen.gray));
            self.err_ends_line = true;
            self.aside = true;
            self.blank = false;
        } else {
            self.aside(line, screen);
        }
    }
}

/// 压缩中那一行。摘要请求刚发出去、还没收到字的不写字数：「已写 0 字」看着像卡住了，和终端界面一样（施工 6-3
/// 三补，2026-09-30 项目主人定）。
fn progress(language: &Language, written: u64) -> String {
    if written == 0 {
        return match language {
            Language::Chinese => "· 正在压缩上下文…".to_string(),
            Language::English => "· Compacting the context…".to_string(),
        };
    }
    let written = thousands(written);
    match language {
        Language::Chinese => format!("· 正在压缩上下文… 已写 {written} 字"),
        Language::English => format!("· Compacting the context… {written} characters written"),
    }
}

/// 压好了那一行。
fn done(language: &Language, before: u64, after: u64) -> String {
    let (before, after) = (tokens(before), tokens(after));
    match language {
        Language::Chinese => format!("· 上下文已压缩：{before} → {after} token"),
        Language::English => format!("· Context compacted: {before} → {after} tokens"),
    }
}

/// 失败那一行。
fn failed(language: &Language, reason: &str) -> String {
    match language {
        Language::Chinese => format!("· 压缩失败：{reason}"),
        Language::English => format!("· Compaction failed: {reason}"),
    }
}

/// 改走隔离式那一行（施工 6-6 下）。
fn isolating(language: &Language) -> String {
    match language {
        Language::Chinese => "· 摘要请求里调了工具，改用不带工具的再压".to_string(),
        Language::English => {
            "· The summary called a tool; compacting again without tools".to_string()
        }
    }
}

/// 暂停那一行：连续失败的带次数，内容太大的带是第几条；不认识的原因、缺了数的，只说暂停了、可以怎么办。
fn paused(language: &Language, body: &Value) -> String {
    let (reason, failures, entry) = (
        body["reason"].as_str(),
        body["failures"].as_u64(),
        body["entry"].as_u64(),
    );
    match (language, reason, failures, entry) {
        (Language::Chinese, Some("failures"), Some(n), _) => {
            format!("· 自动压缩连续失败 {n} 次，已暂停：可以手动压缩、换一个模型，或者开新会话")
        }
        (Language::English, Some("failures"), Some(n), _) => format!(
            "· Automatic compaction failed {n} times and is paused: compact manually, switch models, or start a new session"
        ),
        (Language::Chinese, Some("too_large"), _, Some(seq)) => {
            format!("· 第 {seq} 条内容太大，压完很快又满了，自动压缩已暂停")
        }
        (Language::English, Some("too_large"), _, Some(seq)) => format!(
            "· Entry {seq} is too large and keeps filling the context; automatic compaction is paused"
        ),
        (Language::Chinese, ..) => {
            "· 自动压缩已暂停：可以手动压缩、换一个模型，或者开新会话".to_string()
        }
        (Language::English, ..) => "· Automatic compaction is paused: compact manually, switch models, or start a new session".to_string(),
    }
}

/// 为什么失败：取不出摘要的分两种，调了工具的单说；别的照出错的分类说。
fn reason(language: &Language, class: &str, message: &str) -> String {
    if class == "bad_summary" && message.contains("called a tool") {
        return match language {
            Language::Chinese => "摘要请求里调了工具".to_string(),
            Language::English => "the summary called a tool".to_string(),
        };
    }
    language.class_name(class).to_string()
}

/// token 数写成给人看的：不到一千照写；一千以上写 `k`，一百万以上写 `M`，一位小数，整的不写小数。
fn tokens(n: u64) -> String {
    let (value, unit) = match n {
        0..1_000 => return n.to_string(),
        1_000..1_000_000 => (n as f64 / 1_000.0, "k"),
        _ => (n as f64 / 1_000_000.0, "M"),
    };
    let tenths = (value * 10.0).round() / 10.0;
    match tenths.fract() == 0.0 {
        true => format!("{tenths:.0}{unit}"),
        false => format!("{tenths:.1}{unit}"),
    }
}

#[cfg(test)]
mod tests;
