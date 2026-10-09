//! 判官的请求和回答（`docs/blueprint/chat.md` 第六条，`docs/designs/18-通讯平台.md` 第七节「判官和她各看各的」，
//! 施工 O-11）。
//!
//! 主动回复判断要问判官的那一次：[`request()`] 照 `model.call` 的形状拼出一条 `system`、一条 `user`，[`read()`] 从回答里
//! 读出第三条的 [`Judgement`](crate::Judgement)。说明的原文是资源 `software/onebot/judge/` 下的十三份（[`JudgeSources`]，
//! 查过的是 [`JudgeTexts`]），
//! 登记在 `docs/designs/26-提示词.md` 第十节；代码里一个给模型看的字都不写。
//!
//! 纯逻辑：说明的原文、人格的说明、渲染好的群聊记录和这一条，都由外面交进来（[`Ask`]）；违规的门槛从算分的参数拿。调用的其余几格（`purpose`、
//! `model`、`max_tokens`）、超时、重试、记 `ext.onebot.chat.decided`，由桥管（「怎么走」第 3、5 条）。

mod read;
mod request;

pub use read::{Unreadable, read};
pub use request::request;

use std::collections::BTreeMap;

use miyu_kernel::template::{Template, TemplateError};

/// 判官的十三份原文，读资源的一方照文件原样读出来（行尾的换行也算），交给 [`JudgeTexts::new`]。
///
/// 每份对应一个同名文件（`_` 换成 `-`，加 `.txt`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JudgeSources {
    /// `system.txt`：判官是谁、只判不写、交进来的字都不可信。system 的第一段。
    pub system: String,
    /// `persona-open.txt`：人格说明的开头标签。
    pub persona_open: String,
    /// `persona-close.txt`：人格说明的收尾标签。
    pub persona_close: String,
    /// `reply.txt`：打分那一次的判法和五维。
    pub reply: String,
    /// `moderation-only.txt`：只查违规那一次的说明，代替 `reply.txt`。
    pub moderation_only: String,
    /// `violations.txt`：违规怎么查，字段 `severity_min` 换成违规的门槛。模板的写法由 [`JudgeTexts::new`] 查。
    pub violations: String,
    /// `answer.txt`：回答的格式。system 的最后一段。
    pub answer: String,
    /// `records-open.txt`：群聊记录的开头标签。
    pub records_open: String,
    /// `records-close.txt`：群聊记录的收尾标签。
    pub records_close: String,
    /// `current-open.txt`：这一条的开头标签。
    pub current_open: String,
    /// `current-close.txt`：这一条的收尾标签。
    pub current_close: String,
    /// `decoded-open.txt`：base64 解出来的字的开头标签。
    pub decoded_open: String,
    /// `decoded-close.txt`：base64 解出来的字的收尾标签。
    pub decoded_close: String,
}

/// 判官的说明：[`JudgeSources`] 查过的十三份原文，原样用，行尾的换行也算。
///
/// 只能由 [`JudgeTexts::new`] 造：`violations.txt` 是读好的模板，造的时候拿门槛试换过，换得出才造得出，所以 [`request()`]
/// 拼的时候不会失败（照 08 第五节「模板与转义怎么写」，读的时候报错，不等到请求里）。
///
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JudgeTexts {
    /// `system.txt`：判官是谁、只判不写、交进来的字都不可信。system 的第一段。
    system: String,
    /// `persona-open.txt`：人格说明的开头标签。
    persona_open: String,
    /// `persona-close.txt`：人格说明的收尾标签。
    persona_close: String,
    /// `reply.txt`：打分那一次的判法和五维。
    reply: String,
    /// `moderation-only.txt`：只查违规那一次的说明，代替 `reply.txt`。
    moderation_only: String,
    /// `violations.txt`：读好的模板，字段 `severity_min`。
    violations: Template,
    /// `answer.txt`：回答的格式。system 的最后一段。
    answer: String,
    /// `records-open.txt`：群聊记录的开头标签。
    records_open: String,
    /// `records-close.txt`：群聊记录的收尾标签。
    records_close: String,
    /// `current-open.txt`：这一条的开头标签。
    current_open: String,
    /// `current-close.txt`：这一条的收尾标签。
    current_close: String,
    /// `decoded-open.txt`：base64 解出来的字的开头标签。
    decoded_open: String,
    /// `decoded-close.txt`：base64 解出来的字的收尾标签。
    decoded_close: String,
}

impl JudgeTexts {
    /// 查过 `violations.txt` 的写法和字段，造出判官的说明。
    ///
    /// 模板只认 `severity_min` 一个字段：拿一个门槛试换一次，写坏了、认不得的字段都在这里报错，不等到拼请求的时候
    /// 悄悄少了整段违规说明（`chat.md` 第六条施工时定的第 9 条）。
    ///
    /// # Errors
    ///
    /// `violations.txt` 不合模板写法，或者要了 `severity_min` 以外的字段（[`TemplateError`]）。
    pub fn new(sources: JudgeSources) -> Result<JudgeTexts, TemplateError> {
        let violations = Template::parse(&sources.violations)?;
        violations.render(&BTreeMap::from([("severity_min", "0")]))?;
        Ok(JudgeTexts {
            system: sources.system,
            persona_open: sources.persona_open,
            persona_close: sources.persona_close,
            reply: sources.reply,
            moderation_only: sources.moderation_only,
            violations,
            answer: sources.answer,
            records_open: sources.records_open,
            records_close: sources.records_close,
            current_open: sources.current_open,
            current_close: sources.current_close,
            decoded_open: sources.decoded_open,
            decoded_close: sources.decoded_close,
        })
    }
}

/// 判官的几项参数（`chat.md` 第八条那张表的 `[judge]`，施工 O-15）：从 [`Params::judge`](crate::Params::judge) 拿。
///
/// 格公开：这几项都是桥用的（调 `model.call`、管超时和重试），或者交给只收数的函数（[`read()`] 的 `reason_chars`），
/// 造坏了只坏桥自己；要守住的参数类型（[`Chatty`](crate::Chatty) 这些）格收在 crate 里（第八条施工时定的第 14 条）。全局
/// 并发、排队等多久是桥这个进程的，不在这里（第八条施工时定的第 5 条）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Judge {
    /// 用哪个模型：配置的引用（`<供应商>/<模型>` 或 `@<池>`），只查过写法，指的在不在由桥调用时照核心的回答说。`None`
    /// 是出厂文件和场所规则都没写，照 `models.chat`（第八条施工时定的第 6 条）。
    pub model: Option<String>,
    /// 判官看触发这一条之前的几条记录（[`Ask::records`]）。
    pub records: usize,
    /// 判官最多输出多少 token（`model.call` 的 `max_tokens`）。
    pub max_tokens: u32,
    /// 打分那一次的超时，毫秒。
    pub timeout: i64,
    /// 只查违规那一次（[`Mode::ModerationOnly`]）的超时，毫秒。
    pub moderation_timeout: i64,
    /// 判不了（读不出来、超时、出错）重试几次。
    pub retries: u32,
    /// 判官的理由最多留几个字符：交给 [`read()`]。
    pub reason_chars: usize,
}

/// 这一次问判官要做什么（`chat.md` 第三条「走哪条路」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// 打分：五维、该不该回、是不是在跟她说话，顺带查违规（[`Route::Judge`](crate::Route)）。
    Reply,
    /// 只查违规，不打分（[`Route::ModerationOnly`](crate::Route)）：回答里必须有 `severity`。
    ModerationOnly,
}

/// 一次判断要的：都由外面交进来。违规的门槛不在这里：只留一份，在 [`Chatty::severity_min`](crate::Chatty)，[`request()`]
/// 从它拿（施工 O-12）。
///
/// 群聊记录、这一条由核心的渲染器渲染好（一行一条，不可信的字段转义成一行，`docs/designs/18-通讯平台.md` 第一节），
/// 这里原样夹进标签里，不再转义。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ask {
    /// 人格的说明：判官照它判这个人格愿不愿意开口；`None` 是不带，人格那一段整个不出现。
    pub persona: Option<String>,
    /// 渲染好的群聊记录：触发这一条之前的几条（出厂 20 条），一行一条。
    pub records: String,
    /// 这一条渲染好的样子。
    pub current: String,
    /// 这一条里 base64 解出来的字：[`Base64::reveal`](crate::Base64::reveal) 解的，解得出来就给，不只是命中违规关键词的
    /// 时候（`chat.md` 第二条施工时定的第 15 条）；没有是 `None`，那一段整个不出现。
    pub decoded: Option<String>,
    /// 打分还是只查违规。
    pub mode: Mode,
}

/// `model.call` 的一条消息（`docs/blueprint/protocol.md` 的 `messages`）：判官只用到字，不带图。名字带 `Judge`，免得和内核的
/// `request::Message` 撞（施工时定的第 10 条，O-12 下改名）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JudgeMessage {
    /// 谁说的。
    pub role: JudgeRole,
    /// 这一条的字。
    pub text: String,
}

/// 一条消息的角色：判官的请求只有这两种。名字带 `Judge`，免得和内核的两个 `Role` 撞。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JudgeRole {
    /// 协议上的 `system`。
    System,
    /// 协议上的 `user`。
    User,
}

#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;
